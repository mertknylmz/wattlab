import asyncio
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from ftms_control import ControlRejected, PowerRange
from trainer_agent import TrainerAgent
from trainer_connection import BIKE, POWER

BIKE_PACKET = bytes([0x44, 0x00, 0xC4, 0x09, 0xB4, 0x00, 0x96, 0x00])  # 25 km/h, 90 rpm, 150 W
POWER_PACKET = bytes([0x00, 0x00, 0xA0, 0x00])  # 160 W


class FakeSender:
    def __init__(self, fail=False):
        self.fail = fail
        self.events = []
        self.session_id = None
        self.closed = False
        self.delivered = 0
        self.dropped = 0

    async def start(self):
        pass

    async def ensure_session(self):
        if self.fail:
            raise OSError("api down")
        self.session_id = str(uuid4())
        return self.session_id

    def enqueue(self, kind, event):
        self.events.append((kind, event))
        self.delivered += 1

    async def close(self):
        self.closed = True


class FakeConnection:
    def __init__(self, handlers, on_state, on_ready, on_lost):
        self.handlers, self.on_state, self.on_ready, self.on_lost = handlers, on_state, on_ready, on_lost
        self._stop = asyncio.Event()

    def stop(self):
        self._stop.set()

    async def run(self):
        self.on_state("CONNECTING")
        await self.on_ready(SimpleNamespace())
        self.on_state("STREAMING")
        await self._stop.wait()
        self.on_lost()
        self.on_state("STOPPED")


class FakeControl:
    def __init__(self, client, *, supported=True, reject=None):
        self.erg_supported = supported
        self.power_range = PowerRange(0, 1000, 1)
        self.target_w = None
        self.reject = reject
        self.released = 0

    async def setup(self):
        return []

    async def set_target_power(self, watts):
        if not 0 <= watts <= 1000:
            raise ValueError("range")
        if self.reject:
            raise ControlRejected(self.reject)
        self.target_w = watts

    async def release(self):
        self.released += 1
        self.target_w = None


class AgentTests(unittest.IsolatedAsyncioTestCase):
    def agent(self, *, sender_fail=False, control=FakeControl):
        self.senders = []
        self.connections = []

        def sender_factory():
            self.senders.append(FakeSender(sender_fail))
            return self.senders[-1]

        def connection_factory(address, handlers, **hooks):
            connection = FakeConnection(handlers, **hooks)
            connection.address = address
            self.connections.append(connection)
            return connection

        return TrainerAgent(SimpleNamespace(), connection_factory=connection_factory,
                            sender_factory=sender_factory, control_factory=control,
                            scanner=self.scanner)

    async def scanner(self):
        return [{"address": "AA:BB:CC:DD:EE:FF", "name": "D500", "rssi": -40,
                 "likely_trainer": True}]

    async def connected(self, agent, address="AA:BB:CC:DD:EE:FF"):
        self.assertEqual(await agent.handle("connect", {"address": address}), (True, "connected"))
        self.assertEqual(agent.connection_state, "streaming")
        self.assertEqual(self.connections[-1].address, address)

    async def test_telemetry_is_recorded_only_inside_a_started_session(self):
        agent = self.agent()
        await self.connected(agent)
        handlers = self.connections[0].handlers
        handlers[BIKE](None, BIKE_PACKET)  # Before a session: shown nowhere, sent nowhere.
        ok, message = await agent.handle("start_session", {})
        self.assertTrue(ok)
        sender = self.senders[0]
        self.assertEqual(message, f"session:{sender.session_id}")
        self.assertEqual(sender.events[0][0], "connection-events")
        self.assertEqual(sender.events[0][1]["state"], "streaming")
        handlers[BIKE](None, BIKE_PACKET)
        handlers[POWER](None, POWER_PACKET)
        kinds = [(kind, event.get("source")) for kind, event in sender.events[1:]]
        self.assertEqual(kinds, [("telemetry", "ftms"), ("telemetry", "cycling_power")])
        self.assertEqual(agent.status()["session_id"], sender.session_id)
        self.assertEqual(await agent.handle("start_session", {}), (False, "session_already_active"))
        ok, _ = await agent.handle("finish_session", {})
        self.assertTrue(ok and sender.closed)
        self.assertEqual(await agent.handle("finish_session", {}), (False, "no_active_session"))
        await agent.handle("disconnect", {})

    async def test_session_requires_a_streaming_trainer(self):
        agent = self.agent()
        self.assertEqual(await agent.handle("start_session", {}), (False, "trainer_not_connected"))
        self.assertEqual(self.senders, [])
        await self.connected(agent)
        ok, message = await agent.handle("start_session", {})
        self.assertTrue(ok)
        self.assertTrue(message.startswith("session:"))

    async def test_session_creation_failure_is_reported_and_cleaned_up(self):
        agent = self.agent(sender_fail=True)
        await self.connected(agent)
        self.assertEqual(await agent.handle("start_session", {}), (False, "session_not_created"))
        self.assertTrue(self.senders[0].closed)
        self.assertIsNone(agent.recording)

    async def test_target_power_requires_a_streaming_capable_trainer(self):
        agent = self.agent()
        self.assertEqual(await agent.handle("set_target_power", {"target_power_w": 200}),
                         (False, "erg_unavailable"))
        await self.connected(agent)
        self.assertEqual(await agent.handle("set_target_power", {"target_power_w": 200}),
                         (True, "target:200"))
        self.assertEqual(agent.status()["target_power_w"], 200)
        self.assertEqual(agent.status()["power_range_w"], [0, 1000])
        self.assertEqual(await agent.handle("set_target_power", {"target_power_w": 5000}),
                         (False, "target_out_of_range"))
        control = agent.control
        self.assertEqual(await agent.handle("disconnect", {}), (True, "disconnected"))
        self.assertEqual(control.released, 1, "resistance is released before disconnecting")
        self.assertIsNone(agent.control)
        self.assertEqual(agent.connection_state, "idle")

    async def test_reconnect_starts_without_control_or_old_target(self):
        agent = self.agent()
        await self.connected(agent)
        await agent.handle("set_target_power", {"target_power_w": 180})
        self.connections[0].on_lost()
        self.assertIsNone(agent.status()["target_power_w"])
        self.assertFalse(agent.status()["erg_supported"])
        await agent.handle("disconnect", {})

    async def test_trainer_rejection_is_forwarded_as_a_code(self):
        agent = self.agent(control=lambda client: FakeControl(client, reject="control_not_permitted"))
        await self.connected(agent)
        self.assertEqual(await agent.handle("set_target_power", {"target_power_w": 150}),
                         (False, "control_not_permitted"))
        await agent.handle("disconnect", {})

    async def test_unknown_and_idempotent_commands(self):
        agent = self.agent()
        self.assertEqual(await agent.handle("explode", {}), (False, "unsupported_command"))
        self.assertEqual(await agent.handle("disconnect", {}), (True, "already_disconnected"))
        self.assertEqual(await agent.handle("release_control", {}), (True, "no_control"))
        await self.connected(agent)
        self.assertEqual(await agent.handle("connect", {"address": "AA:BB:CC:DD:EE:FF"}),
                         (True, "already_connected"))
        await agent.handle("disconnect", {})

    async def test_scan_lists_devices_and_connect_needs_the_chosen_address(self):
        agent = self.agent()
        self.assertEqual(await agent.handle("connect", {}), (False, "trainer_not_configured"))
        self.assertEqual(await agent.handle("scan", {}), (True, "found:1"))
        self.assertEqual(agent.status()["devices"][0]["name"], "D500")
        await self.connected(agent)
        self.assertEqual(agent.status()["trainer_address"], "AA:BB:CC:DD:EE:FF")
        self.assertEqual(agent.status()["devices"], [])
        self.assertEqual(await agent.handle("scan", {}), (False, "already_connected"))
        await agent.handle("disconnect", {})

    async def test_run_reports_results_and_shuts_down_cleanly(self):
        reports, beats = [], []
        command_id = str(uuid4())
        queue = [{"id": command_id, "kind": "start_session"}]

        async def next_command():
            if queue:
                return queue.pop()
            await asyncio.sleep(0.01)
            return None

        async def report(cid, ok, message):
            reports.append((cid, ok, message))

        async def heartbeat(status):
            beats.append(status)

        agent = self.agent()
        await self.connected(agent)
        agent.api = SimpleNamespace(next_command=next_command, report=report, heartbeat=heartbeat)
        runner = asyncio.create_task(agent.run())
        for _ in range(50):
            if reports:
                break
            await asyncio.sleep(0.01)
        agent.stop()
        await runner
        self.assertEqual(reports[0][:2], (command_id, True))
        self.assertTrue(beats and beats[0]["connection_state"] == "streaming")
        self.assertTrue(self.senders[0].closed, "shutdown finishes the active session")


if __name__ == "__main__":
    unittest.main()
