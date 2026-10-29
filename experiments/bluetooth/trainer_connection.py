"""Single-owner BLE lifecycle; no hardware access until run() is called."""
import asyncio
import logging
import time
from dataclasses import dataclass
from math import isfinite

BIKE = "00002ad2-0000-1000-8000-00805f9b34fb"
POWER = "00002a63-0000-1000-8000-00805f9b34fb"
log = logging.getLogger(__name__)


class UnsupportedTrainer(RuntimeError):
    pass


class ReconnectExhausted(RuntimeError):
    pass


class CleanupFailed(RuntimeError):
    pass


@dataclass(frozen=True)
class ConnectionSettings:
    timeout: float = 20.0
    cleanup_timeout: float = 3.0
    max_retries: int = 8
    retry_delay: float = 1.0
    max_delay: float = 30.0
    stable_after: float = 30.0

    def __post_init__(self):
        for name in ("timeout", "cleanup_timeout", "retry_delay", "max_delay", "stable_after"):
            value = getattr(self, name)
            if not isfinite(value) or value <= 0:
                raise ValueError(f"{name} must be finite and positive")
        if isinstance(self.max_retries, bool) or not isinstance(self.max_retries, int) or self.max_retries < 0:
            raise ValueError("max_retries must be a non-negative integer")


def telemetry_characteristics(services):
    """Use either supported source; never assume both exist."""
    selected = []
    for uuid in (BIKE, POWER):
        char = services.get_characteristic(uuid)
        if char is not None and "notify" in char.properties:
            selected.append(char)
    if not selected:
        raise UnsupportedTrainer("No supported notifying telemetry characteristic")
    return selected


class TrainerConnection:
    def __init__(self, address, client_factory, handlers, *, settings=None,
                 retry_errors=(OSError, TimeoutError), on_state=None,
                 clock=time.monotonic, sleep=asyncio.sleep):
        self.clock = clock
        self.sleep = sleep
        self._stable = False
        self._cleanup_failed = False
        self.address = address
        self.client_factory = client_factory
        self.handlers = handlers
        self.settings = settings or ConnectionSettings()
        self.retry_errors = retry_errors
        self.on_state = on_state or (lambda state: None)
        self._stop = asyncio.Event()
        self._running = False
        self._generation = 0
        self._stream_started = None

    def stop(self):
        self._stop.set()

    async def _cleanup(self, client, subscribed):
        # Each operation has its own budget; one failure cannot skip disconnect.
        for char in reversed(subscribed):
            try:
                await asyncio.wait_for(client.stop_notify(char), self.settings.cleanup_timeout)
            except Exception as exc:
                log.warning("Notification cleanup failed (%s)", type(exc).__name__)
        try:
            await asyncio.wait_for(client.disconnect(), self.settings.cleanup_timeout)
        except Exception as exc:
            log.warning("Disconnect cleanup failed (%s)", type(exc).__name__)
            self._cleanup_failed = True

    async def _session(self):
        loop = asyncio.get_running_loop()
        disconnected = asyncio.Event()
        self._generation += 1
        generation = self._generation

        def lost(_):
            loop.call_soon_threadsafe(disconnected.set)

        client = self.client_factory(self.address, disconnected_callback=lost,
                                     timeout=self.settings.timeout)
        subscribed = []
        try:
            self.on_state("CONNECTING")
            await asyncio.wait_for(client.connect(), self.settings.timeout)
            self.on_state("SUBSCRIBING")
            for char in telemetry_characteristics(client.services):
                if disconnected.is_set() or not client.is_connected:
                    raise ConnectionError("Disconnected during setup")
                handler = self.handlers[char.uuid.lower()]

                def receive(sender, data, handler=handler):
                    if generation == self._generation and not self._stop.is_set() and not disconnected.is_set():
                        handler(sender, data)

                await asyncio.wait_for(client.start_notify(char, receive), self.settings.timeout)
                subscribed.append(char)
            if disconnected.is_set() or not client.is_connected:
                raise ConnectionError("Disconnected during setup")
            self._stream_started = self.clock()
            self.on_state("STREAMING")
            await disconnected.wait()
            raise ConnectionError("Trainer disconnected")
        finally:
            self._stable = (self._stream_started is not None and
                            self.clock() - self._stream_started >= self.settings.stable_after)
            self._generation += 1  # Ignore callbacks from the previous connection.
            cleanup = asyncio.create_task(self._cleanup(client, subscribed))
            try:
                await asyncio.shield(cleanup)
            except asyncio.CancelledError:
                await cleanup
                raise

    async def _until_stop(self, operation):
        task = asyncio.create_task(operation)
        stopper = asyncio.create_task(self._stop.wait())
        try:
            done, _ = await asyncio.wait((task, stopper), return_when=asyncio.FIRST_COMPLETED)
            if stopper in done:
                return
            await task
        finally:
            for pending in (task, stopper):
                if not pending.done():
                    pending.cancel()
            await asyncio.gather(task, stopper, return_exceptions=True)

    async def run(self):
        if self._running:
            raise RuntimeError("Connection manager is already running")
        self._running = True
        failures = 0
        try:
            while not self._stop.is_set():
                self._stream_started = None
                self._stable = False
                try:
                    await self._until_stop(self._session())
                except self.retry_errors:
                    if self._cleanup_failed:
                        raise CleanupFailed("Disconnect could not be confirmed; reconnect disabled") from None
                    if self._stop.is_set():
                        break
                    if self._stable:
                        failures = 0
                    if failures >= self.settings.max_retries:
                        raise ReconnectExhausted("Trainer retry budget exhausted") from None
                    delay = min(self.settings.retry_delay * 2 ** min(failures, 30), self.settings.max_delay)
                    failures += 1
                    self.on_state("RETRY_WAIT")
                    await self._until_stop(self.sleep(delay))
        finally:
            self._running = False
            self.on_state("STOPPED")
