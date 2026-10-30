import asyncio
import struct
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from ftms_control import (CONTROL_POINT, FEATURE, POWER_RANGE, ControlRejected, FtmsControl,
                          ControlUnavailable, encode_target_power, parse_features,
                          parse_power_range, parse_response)


def features(power_target=True):
    return struct.pack("<II", 0, (1 << 3) if power_target else 0)


class ControlClient:
    """Answers each control point write with a scripted indication."""

    def __init__(self, *, power_target=True, results=None, props=("write", "indicate"),
                 power_range=struct.pack("<hhH", 0, 1000, 1), silent=False):
        chars = {
            FEATURE: SimpleNamespace(uuid=FEATURE, properties=["read"]),
            CONTROL_POINT: SimpleNamespace(uuid=CONTROL_POINT, properties=list(props)),
        }
        if power_range is not None:
            chars[POWER_RANGE] = SimpleNamespace(uuid=POWER_RANGE, properties=["read"])
        self.services = SimpleNamespace(get_characteristic=chars.get)
        self.values = {FEATURE: features(power_target), POWER_RANGE: power_range}
        self.results = list(results or [])
        self.writes = []
        self.silent = silent
        self.indicate = None

    async def read_gatt_char(self, char):
        return self.values[char.uuid]

    async def start_notify(self, char, callback):
        self.indicate = callback

    async def write_gatt_char(self, char, payload, response):
        assert response is True
        self.writes.append(bytes(payload))
        if not self.silent:
            result = self.results.pop(0) if self.results else 0x01
            asyncio.get_running_loop().call_soon(self.indicate, char, bytes([0x80, payload[0], result]))


class ParserTests(unittest.TestCase):
    def test_feature_bits_and_truncation(self):
        self.assertTrue(parse_features(features(True)))
        self.assertFalse(parse_features(features(False)))
        with self.assertRaises(ValueError):
            parse_features(b"\x00" * 7)

    def test_power_range_is_signed_and_validated(self):
        self.assertEqual(parse_power_range(struct.pack("<hhH", -10, 800, 5)).min_w, -10)
        for raw in (b"\x00" * 5, struct.pack("<hhH", 900, 100, 1), struct.pack("<hhH", 0, 100, 0)):
            with self.assertRaises(ValueError):
                parse_power_range(raw)

    def test_target_encoding_and_response_parsing(self):
        self.assertEqual(encode_target_power(250), b"\x05\xfa\x00")
        self.assertEqual(encode_target_power(-1), b"\x05\xff\xff")
        for invalid in (True, 2.5, 40000):
            with self.assertRaises(ValueError):
                encode_target_power(invalid)
        self.assertEqual(parse_response(b"\x80\x05\x03"), (0x05, "invalid_parameter"))
        self.assertEqual(parse_response(b"\x80\x05\x09")[1], "unknown_0x09")
        with self.assertRaises(ValueError):
            parse_response(b"\x05\x01")


class ControlTests(unittest.IsolatedAsyncioTestCase):
    async def test_setup_without_power_target_never_subscribes(self):
        for client in (ControlClient(power_target=False), ControlClient(props=("write",))):
            control = FtmsControl(client)
            self.assertEqual(await control.setup(), [])
            self.assertFalse(control.erg_supported)
            with self.assertRaises(ControlUnavailable):
                await control.set_target_power(100)
            self.assertEqual(client.writes, [])

    async def test_first_target_requests_control_then_sets_power(self):
        client = ControlClient()
        control = FtmsControl(client)
        self.assertEqual(len(await control.setup()), 1)
        self.assertEqual(control.power_range.max_w, 1000)
        await control.set_target_power(200)
        await control.set_target_power(210)
        self.assertEqual(client.writes, [b"\x00", b"\x05\xc8\x00", b"\x05\xd2\x00"])
        self.assertEqual(control.target_w, 210)
        await control.release()
        self.assertEqual(client.writes[-1], b"\x01")
        self.assertIsNone(control.target_w)

    async def test_out_of_range_targets_are_rejected_before_writing(self):
        client = ControlClient()
        control = FtmsControl(client)
        await control.setup()
        with self.assertRaises(ValueError):
            await control.set_target_power(1001)
        self.assertEqual(client.writes, [])

    async def test_trainer_rejection_and_silence_are_reported(self):
        control = FtmsControl(ControlClient(results=[0x05]))
        await control.setup()
        with self.assertRaises(ControlRejected) as caught:
            await control.set_target_power(150)
        self.assertEqual(caught.exception.reason, "control_not_permitted")
        self.assertIsNone(control.target_w)
        silent = FtmsControl(ControlClient(silent=True), timeout=0.01)
        await silent.setup()
        with self.assertRaises(TimeoutError):
            await silent.set_target_power(150)
        self.assertIsNone(silent.target_w)


if __name__ == "__main__":
    unittest.main()
