"""Long-running trainer agent: executes commands from the local WattLab API.

The agent owns Bluetooth. It connects only on an explicit `connect` command,
records telemetry only inside a session the user started, and changes trainer
resistance only on an explicit `set_target_power` command. Nothing is replayed
after a reconnect: a new connection starts without control or target power.
"""
import asyncio
import logging
import os
import signal
from uuid import UUID, uuid4

import aiohttp

from ftms_control import ControlRejected, ControlUnavailable, FtmsControl
from ftms_parser import parse_cycling_power, parse_indoor_bike_data
from telemetry import TelemetryEvents
from telemetry_sender import PermanentDeliveryError, TelemetrySender
from trainer_connection import BIKE, POWER

log = logging.getLogger(__name__)
HEARTBEAT_SECONDS = 1.0
DISCONNECT_BUDGET = 15.0


class Recording:
    """One user-started session: its sender and its own event stream."""

    def __init__(self, sender):
        self.sender = sender
        self.events = TelemetryEvents()


class TrainerAgent:
    def __init__(self, api, *, connection_factory, sender_factory, control_factory=FtmsControl,
                 control_errors=(OSError, TimeoutError), agent_id=None):
        self.api = api
        self.connection_factory = connection_factory
        self.sender_factory = sender_factory
        self.control_factory = control_factory
        # Transport errors of the BLE library (e.g. BleakError) during control.
        self.control_errors = tuple(control_errors) + (ControlUnavailable,)
        self.agent_id = agent_id or str(uuid4())
        self.connection_state = "idle"
        self.connection = None
        self.connection_task = None
        self.recording = None
        self.control = None
        self._stop = asyncio.Event()

    # --- BLE callbacks: never await, never block -------------------------------
    def _bike(self, _sender, data):
        if self.recording is None:
            return
        try:
            event = self.recording.events.bike(parse_indoor_bike_data(data))
        except ValueError:
            return
        self.recording.sender.enqueue("telemetry", event)

    def _power(self, _sender, data):
        if self.recording is None:
            return
        try:
            event = self.recording.events.power(parse_cycling_power(data))
        except ValueError:
            return
        self.recording.sender.enqueue("telemetry", event)

    def _state(self, value):
        self.connection_state = value.lower()
        if self.recording is not None:
            self.recording.sender.enqueue("connection-events", self.recording.events.state(value))

    async def _ready(self, client):
        control = self.control_factory(client)
        try:
            extra = await control.setup()
        except self.control_errors + (ValueError,) as exc:
            log.warning("Trainer control unavailable (%s)", type(exc).__name__)
            return []
        self.control = control
        return extra

    def _lost(self):
        # The trainer may have dropped our control; never resend the old target.
        self.control = None

    # --- Status ----------------------------------------------------------------
    def status(self):
        control = self.control
        limits = control.power_range if control else None
        return {
            "agent_id": self.agent_id,
            "connection_state": self.connection_state,
            "session_id": self.recording.sender.session_id if self.recording else None,
            "erg_supported": bool(control and control.erg_supported),
            "power_range_w": [limits.min_w, limits.max_w] if limits else None,
            "target_power_w": control.target_w if control else None,
        }

    # --- Commands --------------------------------------------------------------
    async def handle(self, kind, command):
        """Return (succeeded, message). Messages are short codes, never secrets."""
        if kind == "connect":
            return self._connect()
        if kind == "disconnect":
            return await self._disconnect()
        if kind == "start_session":
            return await self._start_session()
        if kind == "finish_session":
            return await self._finish_session()
        if kind == "set_target_power":
            return await self._set_target(command.get("target_power_w"))
        if kind == "release_control":
            return await self._release()
        return False, "unsupported_command"

    def _connect(self):
        if self.connection_task is not None and not self.connection_task.done():
            return True, "already_connected"
        try:
            self.connection = self.connection_factory(
                {BIKE: self._bike, POWER: self._power},
                on_state=self._state, on_ready=self._ready, on_lost=self._lost)
        except (RuntimeError, ValueError):
            return False, "trainer_not_configured"
        self.connection_task = asyncio.create_task(self._run_connection(self.connection))
        return True, "connecting"

    async def _run_connection(self, connection):
        try:
            await connection.run()
            self.connection_state = "idle"
        except Exception as exc:  # The agent survives; the state shows the failure.
            log.error("Trainer connection ended (%s)", type(exc).__name__)
            self.connection_state = "failed"

    async def _disconnect(self):
        if self.connection_task is None or self.connection_task.done():
            self.connection_state = "idle"
            return True, "already_disconnected"
        await self._release_quietly()
        self.connection.stop()
        try:
            await asyncio.wait_for(asyncio.shield(self.connection_task), DISCONNECT_BUDGET)
        except TimeoutError:
            return False, "disconnect_timeout"
        return True, "disconnected"

    async def _start_session(self):
        if self.recording is not None:
            return False, "session_already_active"
        sender = self.sender_factory()
        await sender.start()
        try:
            await sender.ensure_session()
        except (aiohttp.ClientError, OSError, TimeoutError, PermanentDeliveryError):
            await sender.close()
            return False, "session_not_created"
        recording = Recording(sender)
        self.recording = recording
        if self.connection_state != "idle":
            # Mark the current link in the new session so its live view is valid.
            sender.enqueue("connection-events", recording.events.state(self.connection_state.upper()))
        return True, f"session:{sender.session_id}"

    async def _finish_session(self):
        recording, self.recording = self.recording, None
        if recording is None:
            return False, "no_active_session"
        await recording.sender.close()
        sender = recording.sender
        return True, f"delivered:{sender.delivered} dropped:{sender.dropped}"

    async def _set_target(self, watts):
        control = self.control
        if self.connection_state != "streaming" or control is None or not control.erg_supported:
            return False, "erg_unavailable"
        try:
            await control.set_target_power(watts)
        except ValueError:
            return False, "target_out_of_range"
        except ControlRejected as exc:
            return False, exc.reason
        except self.control_errors:
            return False, "control_failed"
        return True, f"target:{watts}"

    async def _release(self):
        if self.control is None:
            return True, "no_control"
        try:
            await self.control.release()
        except ControlRejected as exc:
            return False, exc.reason
        except self.control_errors:
            return False, "control_failed"
        return True, "released"

    async def _release_quietly(self):
        if self.control is not None and self.control.target_w is not None:
            succeeded, message = await self._release()
            if not succeeded:
                log.warning("Control release not confirmed (%s)", message)

    # --- Loops -----------------------------------------------------------------
    def stop(self):
        self._stop.set()

    async def _heartbeats(self):
        failures = 0
        while not self._stop.is_set():
            try:
                await self.api.heartbeat(self.status())
                failures = 0
            except (aiohttp.ClientError, OSError, TimeoutError, RuntimeError):
                failures += 1
                if failures == 1:
                    log.warning("Heartbeat failed; retrying")
            try:
                await asyncio.wait_for(self._stop.wait(), HEARTBEAT_SECONDS)
            except TimeoutError:
                pass

    async def _commands(self):
        delay = 1.0
        while not self._stop.is_set():
            try:
                command = await self.api.next_command()
                delay = 1.0
            except (aiohttp.ClientError, OSError, TimeoutError, RuntimeError):
                try:
                    await asyncio.wait_for(self._stop.wait(), delay)
                except TimeoutError:
                    pass
                delay = min(delay * 2, 5.0)
                continue
            if command is None:
                continue
            await self._execute(command)

    async def _execute(self, command):
        try:
            command_id = str(UUID(command["id"]))
            kind = command["kind"]
        except (KeyError, TypeError, ValueError):
            log.error("Ignoring malformed command")
            return
        succeeded, message = await self.handle(kind, command)
        log.info("Command %s: %s", kind, message if succeeded else f"rejected ({message})")
        for attempt in range(3):
            try:
                await self.api.report(command_id, succeeded, message)
                return
            except (aiohttp.ClientError, OSError, TimeoutError, RuntimeError):
                await asyncio.sleep(0.5 * (attempt + 1))
        log.warning("Command result could not be reported; it will expire on the API")

    async def run(self):
        loops = [asyncio.create_task(self._heartbeats()), asyncio.create_task(self._commands())]
        try:
            await self._stop.wait()
        finally:
            for task in loops:
                task.cancel()
            await asyncio.gather(*loops, return_exceptions=True)
            # Finish first so the session ends while data still flows, then
            # release resistance and disconnect.
            await self._finish_session()
            await self._disconnect()


async def main():
    from bleak import BleakClient
    from bleak.exc import BleakError
    from agent_client import AgentClient
    from config import trainer_address
    from trainer_connection import TrainerConnection

    api_url = os.getenv("WATTLAB_API_URL", "").strip()
    token = os.getenv("WATTLAB_API_TOKEN", "")

    def connection_factory(handlers, **hooks):
        return TrainerConnection(trainer_address(), BleakClient, handlers,
                                 retry_errors=(OSError, TimeoutError, BleakError), **hooks)

    async with AgentClient(api_url, token) as api:
        agent = TrainerAgent(api, connection_factory=connection_factory,
                             sender_factory=lambda: TelemetrySender(api_url, token),
                             control_errors=(OSError, TimeoutError, BleakError))
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, agent.stop)
        print("Trainer agent ready; waiting for commands from the WattLab screen.", flush=True)
        await agent.run()


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    asyncio.run(main())
