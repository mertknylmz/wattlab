"""FTMS Control Point: capability discovery, target power (ERG) and release.

Pure encoders/decoders are separate from FtmsControl, which owns one connected
client's control point. After subscribe, setup requests control, RESET, then indoor-bike
simulation at 0% (or resistance level 0) so leftover ERG is not inherited.
Control is kept so the trainer stays in free-ride until the rider sets a target.
"""
import asyncio
import logging
import struct
from dataclasses import dataclass

FEATURE = "00002acc-0000-1000-8000-00805f9b34fb"
POWER_RANGE = "00002ad8-0000-1000-8000-00805f9b34fb"
CONTROL_POINT = "00002ad9-0000-1000-8000-00805f9b34fb"

REQUEST_CONTROL = 0x00
RESET = 0x01
SET_RESISTANCE = 0x04
SET_TARGET_POWER = 0x05
SET_SIMULATION = 0x11
RESPONSE_CODE = 0x80
POWER_TARGET_SUPPORTED = 1 << 3  # Target Setting Features, FTMS 4.3.1.2

RESULTS = {
    0x01: "success",
    0x02: "op_code_not_supported",
    0x03: "invalid_parameter",
    0x04: "operation_failed",
    0x05: "control_not_permitted",
}
log = logging.getLogger(__name__)


class ControlRejected(RuntimeError):
    """The trainer answered, but not with success. `reason` is a RESULTS value."""

    def __init__(self, reason):
        super().__init__(f"Trainer rejected control command: {reason}")
        self.reason = reason


class ControlUnavailable(RuntimeError):
    pass


@dataclass(frozen=True)
class PowerRange:
    min_w: int
    max_w: int
    increment_w: int


def parse_features(data):
    """Return whether target power is supported; requires both 32-bit fields."""
    if len(data) < 8:
        raise ValueError("FTMS feature value is truncated")
    _machine, target = struct.unpack_from("<II", data)
    return bool(target & POWER_TARGET_SUPPORTED)


def parse_power_range(data):
    if len(data) < 6:
        raise ValueError("Supported power range is truncated")
    minimum, maximum, increment = struct.unpack_from("<hhH", data)
    if minimum > maximum or increment == 0:
        raise ValueError("Supported power range is inconsistent")
    return PowerRange(minimum, maximum, increment)


def encode_target_power(watts):
    if isinstance(watts, bool) or not isinstance(watts, int) or not -32768 <= watts <= 32767:
        raise ValueError("Target power must be a signed 16-bit integer")
    return struct.pack("<Bh", SET_TARGET_POWER, watts)


def encode_simulation(grade_percent=0.0, wind_ms=0.0, crr=0.004, cw=0.51):
    """Indoor Bike Simulation Parameters: 0% grade is free ride, not ERG."""
    wind = int(round(wind_ms / 0.001))
    grade = int(round(grade_percent / 0.01))
    crr_u = int(round(crr / 0.0001))
    cw_u = int(round(cw / 0.01))
    if not -32768 <= wind <= 32767 or not -32768 <= grade <= 32767:
        raise ValueError("Simulation wind/grade is out of range")
    if not 0 <= crr_u <= 255 or not 0 <= cw_u <= 255:
        raise ValueError("Simulation crr/cw is out of range")
    return struct.pack("<BhhBB", SET_SIMULATION, wind, grade, crr_u, cw_u)


def encode_resistance_level(tenths):
    if isinstance(tenths, bool) or not isinstance(tenths, int) or not 0 <= tenths <= 255:
        raise ValueError("Resistance level must be 0–255 tenths")
    return struct.pack("<BB", SET_RESISTANCE, tenths)


def parse_response(data):
    """Return (request_opcode, result_name) from a control point indication."""
    if len(data) < 3 or data[0] != RESPONSE_CODE:
        raise ValueError("Not a control point response")
    return data[1], RESULTS.get(data[2], f"unknown_{data[2]:#04x}")


class FtmsControl:
    def __init__(self, client, *, timeout=5.0):
        self.client = client
        self.timeout = timeout
        self.erg_supported = False
        self.power_range = None
        self.target_w = None
        self._char = None
        self._has_control = False
        self._lock = asyncio.Lock()
        self._waiting = None

    async def setup(self):
        """Inspect capabilities; subscribe only when target power is usable.

        Returns characteristics the connection owner must unsubscribe.
        """
        services = self.client.services
        char = services.get_characteristic(CONTROL_POINT)
        feature = services.get_characteristic(FEATURE)
        if (char is None or feature is None or "indicate" not in char.properties
                or not {"write", "write-without-response"} & set(char.properties)):
            return []
        if not parse_features(await self.client.read_gatt_char(feature)):
            return []
        range_char = services.get_characteristic(POWER_RANGE)
        if range_char is not None:
            try:
                self.power_range = parse_power_range(await self.client.read_gatt_char(range_char))
            except ValueError as exc:
                log.warning("Ignoring invalid supported power range (%s)", exc)
        await self.client.start_notify(char, self._indication)
        self._char = char
        self.erg_supported = True
        await self._enter_free_ride()
        return [char]

    async def _request_control(self):
        await self._send(bytes([REQUEST_CONTROL]))
        self._has_control = True

    async def _enter_open_mode(self):
        """Leave ERG. Simulation 0% keeps cassette gears; resistance 0 is fallback."""
        try:
            await self._send(encode_simulation())
            log.info("Trainer in free-ride (simulation 0%)")
            return
        except ControlRejected as exc:
            if exc.reason != "op_code_not_supported":
                raise
        await self._send(encode_resistance_level(0))
        log.info("Trainer in free-ride (resistance level 0)")

    async def _enter_free_ride(self):
        """A new link must not inherit ERG. RESET alone is not enough on D500."""
        try:
            async with self._lock:
                await self._request_control()
                await self._send(bytes([RESET]))
                await self._request_control()
                await self._enter_open_mode()
        except (ControlRejected, TimeoutError, OSError) as exc:
            log.warning("Could not enter free-ride (%s)", type(exc).__name__)
            self._has_control = False
        self.target_w = None

    def _indication(self, _sender, data):
        try:
            response = parse_response(bytes(data))
        except ValueError:
            return
        if self._waiting is not None and not self._waiting.done():
            self._waiting.set_result(response)

    async def _send(self, payload):
        if self._char is None:
            raise ControlUnavailable("Target power is not supported by this trainer")
        loop = asyncio.get_running_loop()
        self._waiting = loop.create_future()
        try:
            await asyncio.wait_for(self.client.write_gatt_char(self._char, payload, response=True),
                                   self.timeout)
            opcode, result = await asyncio.wait_for(self._waiting, self.timeout)
        finally:
            self._waiting = None
        if opcode != payload[0]:
            raise ControlRejected("unexpected_response")
        if result != "success":
            raise ControlRejected(result)

    def validate_target(self, watts):
        if isinstance(watts, bool) or not isinstance(watts, int):
            raise ValueError("Target power must be whole watts")
        limits = self.power_range
        if limits is not None and not limits.min_w <= watts <= limits.max_w:
            raise ValueError(f"Target power must be within {limits.min_w}-{limits.max_w} W")

    async def set_target_power(self, watts):
        self.validate_target(watts)
        async with self._lock:
            if not self._has_control:
                await self._send(bytes([REQUEST_CONTROL]))
                self._has_control = True
            await self._send(encode_target_power(watts))
            self.target_w = watts

    async def release(self):
        """Leave ERG and put the trainer back in free-ride, keeping control."""
        async with self._lock:
            try:
                if not self._has_control:
                    await self._request_control()
                await self._send(bytes([RESET]))
                await self._request_control()
                await self._enter_open_mode()
            except (ControlRejected, TimeoutError, OSError, ControlUnavailable) as exc:
                log.warning("Could not return to free-ride (%s)", type(exc).__name__)
                self._has_control = False
            self.target_w = None
