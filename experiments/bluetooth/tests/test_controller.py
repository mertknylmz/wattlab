import asyncio
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

from controller_scan import (
    ZWIFT_COMPANY_ID,
    ZWIFT_CONTROLLER_SERVICE,
    choose_controllers,
    classify_controller,
    rank_controllers,
)
from trainer_agent import TrainerAgent
from zwift_buttons import report_kind
from zwift_controller import (
    SYNC_TX,
    ControllerLink,
    _notify_kwargs,
    _radio_error,
    handshake_request,
    parse_device_public_key,
)


def public_key():
    private = ec.generate_private_key(ec.SECP256R1())
    return private.public_key().public_bytes(Encoding.X962, PublicFormat.UncompressedPoint)


def reply(tag, key=None):
    return b"RideOn" + tag + (public_key()[1:] if key is None else key)


class ScanTests(unittest.TestCase):
    def test_manufacturer_side_byte_identifies_play_and_click(self):
        left = classify_controller(
            "AA:BB:CC:DD:EE:01", "Zwift Play", -40, [], {ZWIFT_COMPANY_ID: bytes([3, 0xEE, 0x01])}
        )
        right = classify_controller("AA:BB:CC:DD:EE:02", None, -48, [], {ZWIFT_COMPANY_ID: bytes([2])})
        click = classify_controller("AA:BB:CC:DD:EE:03", "Zwift Click", -55, [], {ZWIFT_COMPANY_ID: bytes([9, 1, 2])})
        raw = classify_controller("AA:BB:CC:DD:EE:04", "Zwift Play", -60, [], {0: bytes([0x4A, 0x09, 3])})
        self.assertEqual((left["kind"], left["side"]), ("play", "left"))
        self.assertEqual((right["kind"], right["side"]), ("play", "right"))
        self.assertEqual((click["kind"], click["side"]), ("click", None))
        self.assertEqual(raw["side"], "left")

    def test_service_uuid_alone_is_not_a_controller_and_names_still_count(self):
        ride = classify_controller("AA:BB:CC:DD:EE:05", "Zwift Ride", -30, [ZWIFT_CONTROLLER_SERVICE])
        phone = classify_controller("11:22:33:44:55:66", "Phone", -20)
        named = classify_controller("AA:BB:CC:DD:EE:06", "Zwift Play Left", -70)
        self.assertIsNone(ride)
        self.assertIsNone(phone)
        self.assertEqual((named["kind"], named["side"]), ("play", "left"))

    def test_a_play_pair_is_chosen_over_a_louder_third_remote(self):
        click = {"address": "AA:BB:CC:DD:EE:01", "kind": "click", "side": None, "rssi": -30}
        left = {"address": "AA:BB:CC:DD:EE:02", "kind": "play", "side": "left", "rssi": -60}
        right = {"address": "AA:BB:CC:DD:EE:03", "kind": "play", "side": "right", "rssi": -70}
        extra = {"address": "AA:BB:CC:DD:EE:04", "kind": "play", "side": "left", "rssi": -80}
        chosen = choose_controllers([click, extra, right, left])
        self.assertEqual([item["address"] for item in chosen], [left["address"], right["address"]])
        ranked = rank_controllers([click, left, right])
        self.assertEqual([item["address"] for item in ranked if item["selected"]], [left["address"], right["address"]])
        clicks = [
            {"address": "AA:BB:CC:DD:EE:11", "kind": "click", "rssi": -40},
            {"address": "AA:BB:CC:DD:EE:12", "kind": "click", "rssi": -20},
            {"address": "AA:BB:CC:DD:EE:13", "kind": "click", "rssi": -30},
        ]
        self.assertEqual(
            [item["address"] for item in choose_controllers(clicks)],
            ["AA:BB:CC:DD:EE:12", "AA:BB:CC:DD:EE:13"],
        )


class RadioErrorTests(unittest.TestCase):
    def test_failure_text_keeps_a_phrase_and_drops_an_address(self):
        self.assertEqual(
            _radio_error(RuntimeError("failed to discover services, device disconnected")),
            "RuntimeError: failed to discover services, device disconnected",
        )
        hidden = _radio_error(RuntimeError("device 'dev_AA_BB_CC_DD_EE_FF' not found"))
        self.assertEqual(hidden, "RuntimeError: device 'dev_[redacted]' not found")
        self.assertNotIn("AA_BB", hidden)

class HandshakeTests(unittest.TestCase):
    def test_ride_on_request_and_the_three_device_replies(self):
        request = handshake_request(public_key())
        self.assertTrue(request.startswith(b"RideOn\x01\x02"))
        self.assertEqual(len(request), 8 + 64)
        for tag in (b"\x01\x01", b"\x01\x03", b"\x01\x04", b"\x00\x09", b"\x02\x03"):
            key = parse_device_public_key(reply(tag))
            self.assertEqual(len(key), 64)
        self.assertIsNone(parse_device_public_key(b"RideOn\x01\x02" + b"\x11" * 64))
        self.assertIsNone(parse_device_public_key(b"\x37\x00"))
        self.assertIsNone(parse_device_public_key(b"RideOn\x01\x03\x00"))
        self.assertEqual(_notify_kwargs(SYNC_TX), {"bluez": {"use_start_notify": True}})

    def test_button_reports_are_recognized_without_raising(self):
        self.assertEqual(report_kind(b"\x37\x08\x01"), "click")
        self.assertEqual(report_kind(b"\x07\x00"), "play")
        self.assertIsNone(report_kind(b""))
        self.assertIsNone(report_kind(b"\x19"))

    def test_invalid_device_key_stays_inside_the_notify_callback(self):
        link = ControllerLink(
            {"address": "AA:BB:CC:DD:EE:01", "name": "Zwift Play", "kind": "play", "side": "left"},
            lambda address: None,
            attempt_timeout=0.05,
        )
        link._ready = asyncio.Event()
        link._on_sync(None, reply(b"\x02\x03", b"\x00" * 64))
        link._on_async(None, b"\x37\x00")
        link._on_async(None, None)
        self.assertTrue(link._ready.is_set())
        self.assertIsNone(link._peer_key)
        self.assertIsNone(link._error)
        self.assertIsNotNone(link._answered)
        other = ControllerLink(
            {"address": "AA:BB:CC:DD:EE:09", "name": "Zwift Click", "kind": "click", "side": None},
            lambda address: None,
        )
        other._ready = asyncio.Event()
        raw = public_key()[1:]
        other._on_sync(None, b"RideOn\x01\x03" + raw)
        self.assertEqual(other._peer_key, raw)


class ScriptedClient:
    def __init__(self, address, mode):
        self.address = address
        self.mode = mode
        self.callbacks = {}
        self.writes = []
        self.disconnected = False

    async def connect(self):
        if self.mode == "down":
            raise OSError("adapter")

    async def disconnect(self):
        self.disconnected = True

    async def start_notify(self, uuid, callback):
        self.callbacks[uuid] = callback

    async def write(self, uuid, data):
        self.writes.append(bytes(data))
        callback = self.callbacks[SYNC_TX]
        if self.mode == "ok":
            callback(None, reply(b"\x01\x03"))
        elif self.mode == "status":
            callback(None, b"RideOn\x02\x03" + b"\x00" * 64)
        elif self.mode == "silent":
            return
        elif self.mode == "down":
            raise OSError("write")


class LinkTests(unittest.IsolatedAsyncioTestCase):
    async def test_connect_retries_once_then_reports_connected(self):
        modes = ["down", "ok"]
        clients = []

        def factory(address):
            clients.append(ScriptedClient(address, modes.pop(0)))
            return clients[-1]

        link = ControllerLink(
            {"address": "AA:BB:CC:DD:EE:01", "name": "Zwift Click", "kind": "click", "side": None},
            factory,
            attempt_timeout=1,
        )
        await link.connect()
        self.assertEqual(link.state, "connected")
        self.assertIsNone(link.message)
        self.assertTrue(clients[0].disconnected)
        self.assertTrue(clients[1].writes[0].startswith(b"RideOn\x01\x02"))
        await link.disconnect()
        self.assertEqual(link.state, "disconnected")

    async def test_two_timeouts_become_failed(self):
        def factory(address):
            return ScriptedClient(address, "silent")

        link = ControllerLink(
            {"address": "AA:BB:CC:DD:EE:02", "name": "Zwift Play", "kind": "play", "side": "right"},
            factory,
            attempt_timeout=0.05,
        )
        await link.connect()
        self.assertEqual(link.state, "failed")
        self.assertEqual(link.message, "controller_connect_timeout")

    async def test_reply_that_is_not_a_point_stays_connected(self):
        def factory(address):
            return ScriptedClient(address, "status")

        link = ControllerLink(
            {"address": "AA:BB:CC:DD:EE:05", "name": "Zwift Click", "kind": "click", "side": None},
            factory,
            attempt_timeout=1,
        )
        await link.connect()
        self.assertEqual(link.state, "connected")
        self.assertIsNone(link._peer_key)


class AgentControllerTests(unittest.IsolatedAsyncioTestCase):
    def agent(self, found, modes):
        self.dropped = []
        self.modes = list(modes)

        async def scanner():
            return found

        def factory(address):
            return ScriptedClient(address, self.modes.pop(0) if self.modes else "ok")

        async def drop(addresses):
            self.dropped.extend(addresses)

        def connection_factory(address, handlers, **hooks):
            stopped = asyncio.Event()
            connection = SimpleNamespace(handlers=handlers, address=address, stop=stopped.set)

            async def run():
                prepare = hooks.get("on_prepare")
                if prepare is not None:
                    await prepare()
                hooks["on_state"]("STREAMING")
                await stopped.wait()

            connection.run = run
            return connection

        return TrainerAgent(
            SimpleNamespace(),
            connection_factory=connection_factory,
            sender_factory=lambda: SimpleNamespace(),
            scanner=self.empty_scan,
            controller_scanner=scanner,
            controller_client_factory=factory,
            drop_leftovers=drop,
            controller_attempt_timeout=1,
        )

    async def empty_scan(self):
        return []

    async def finished(self, agent):
        pending = [task for task in agent.controller_tasks if not task.done()]
        if pending:
            await asyncio.gather(*pending)

    def play_pair(self):
        return [
            {"address": "AA:BB:CC:DD:EE:02", "name": "Zwift Play", "rssi": -60, "kind": "play", "side": "left"},
            {"address": "AA:BB:CC:DD:EE:03", "name": "Zwift Play", "rssi": -70, "kind": "play", "side": "right"},
            {"address": "AA:BB:CC:DD:EE:01", "name": "Zwift Click", "rssi": -20, "kind": "click", "side": None},
        ]

    async def test_scan_works_while_the_trainer_is_streaming_and_connects_the_pair(self):
        agent = self.agent(self.play_pair(), ["ok", "ok"])
        self.assertEqual(await agent.handle("connect", {"address": "AA:BB:CC:DD:EE:FF"}), (True, "connected"))
        self.assertEqual(await agent.handle("scan", {}), (False, "already_connected"))
        self.assertEqual(await agent.handle("scan_controllers", {}), (True, "found:3"))
        selected = [item["address"] for item in agent.status()["controller_devices"] if item["selected"]]
        self.assertEqual(selected, ["AA:BB:CC:DD:EE:02", "AA:BB:CC:DD:EE:03"])
        self.assertEqual(await agent.handle("connect_controller", {}), (True, "connecting"))
        await self.finished(agent)
        links = {item["address"]: item for item in agent.status()["controllers"]}
        self.assertEqual(links["AA:BB:CC:DD:EE:02"]["state"], "connected")
        self.assertEqual(links["AA:BB:CC:DD:EE:02"]["side"], "left")
        self.assertEqual(links["AA:BB:CC:DD:EE:03"]["side"], "right")
        self.assertNotIn("AA:BB:CC:DD:EE:01", links)
        self.assertEqual(await agent.handle("connect_controller", {}), (True, "already_connected"))
        self.assertEqual(await agent.handle("disconnect_controller", {}), (True, "disconnected"))
        self.assertTrue(all(item["state"] == "disconnected" for item in agent.status()["controllers"]))
        self.assertEqual(await agent.handle("disconnect_controller", {}), (True, "already_disconnected"))
        await agent.handle("disconnect", {})

    async def test_connected_trainer_is_not_offered_as_a_remote(self):
        devices = self.play_pair()
        devices.append({
            "address": "AA:BB:CC:DD:EE:FF", "name": "D500", "rssi": -20,
            "kind": "click", "side": None, "ble_device": object(),
        })
        agent = self.agent(devices, [])
        await agent.handle("connect", {"address": "AA:BB:CC:DD:EE:FF"})
        self.assertEqual(await agent.handle("scan_controllers", {}), (True, "found:3"))
        reported = agent.status()["controller_devices"]
        self.assertNotIn("AA:BB:CC:DD:EE:FF", [item["address"] for item in reported])
        self.assertTrue(all("ble_device" not in item for item in reported))

    async def test_missing_scan_and_failed_links_are_reported(self):
        agent = self.agent([], [])
        self.assertEqual(await agent.handle("connect_controller", {}), (False, "no_controllers"))

        async def broken():
            raise OSError("radio")

        agent.controller_scanner = broken
        self.assertEqual(await agent.handle("scan_controllers", {}), (False, "scan_failed"))

        async def one():
            return self.play_pair()[:1]

        agent.controller_scanner = one
        agent.controller_client_factory = lambda address: ScriptedClient(address, "silent")
        agent.controller_attempt_timeout = 0.05
        self.assertEqual(await agent.handle("scan_controllers", {}), (True, "found:1"))
        self.assertEqual(await agent.handle("connect_controller", {}), (True, "connecting"))
        await self.finished(agent)
        self.assertEqual(agent.status()["controllers"][0]["state"], "failed")
        self.assertEqual(agent.status()["controllers"][0]["message"], "controller_connect_timeout")

    async def test_trainer_connect_drops_leftover_controllers_without_forgetting_the_trainer(self):
        agent = self.agent(self.play_pair(), ["ok"])
        await agent.handle("scan_controllers", {})
        await agent.handle("connect_controller", {})
        self.assertEqual(await agent.handle("connect", {"address": "AA:BB:CC:DD:EE:10"}), (True, "connected"))
        self.assertIn("AA:BB:CC:DD:EE:02", self.dropped)
        self.assertIn("AA:BB:CC:DD:EE:10", self.dropped)
        self.assertTrue(all(item["state"] == "disconnected" for item in agent.status()["controllers"]))
        self.assertEqual(agent.status()["trainer_address"], "AA:BB:CC:DD:EE:10")
        self.assertEqual(agent.status()["connection_state"], "streaming")
        await agent.handle("disconnect", {})


if __name__ == "__main__":
    unittest.main()
