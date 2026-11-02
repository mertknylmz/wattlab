"""Zwift Click / Play connection. BLE stays here; Rust only sees the outcome.

Both remotes speak the RideOn handshake on the Zwift custom service: we write
our uncompressed secp256r1 public key, and the device answers with its own.
A bad public key is ignored inside the notify callback so Bleak keeps the
subscription. AES-CCM button decryption is not part of connecting.
"""
import asyncio
import logging
import re
import shutil

from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

from zwift_buttons import report_kind

log = logging.getLogger(__name__)

RIDE_ON = b"RideOn"
# App hello is 0x01 0x02. A 72-byte RideOn reply finishes the link. This
# firmware's tag is 02 03 and the body is not a secp256r1 point; older
# replies carry a big-endian point at byte 8.
REQUEST_TAG = b"\x01\x02"
ASYNC_CHAR = "00000002-19ca-4651-86e5-fa29dcdd09d1"
SYNC_RX = "00000003-19ca-4651-86e5-fa29dcdd09d1"
SYNC_TX = "00000004-19ca-4651-86e5-fa29dcdd09d1"
ATTEMPT_TIMEOUT = 20.0
ATTEMPTS = 2


def handshake_request(public_uncompressed):
    """RideOn hello. `public_uncompressed` includes the leading 0x04, which is not sent."""
    if len(public_uncompressed) != 65 or public_uncompressed[0] != 4:
        raise ValueError("public key must be an uncompressed secp256r1 point")
    return RIDE_ON + REQUEST_TAG + public_uncompressed[1:]


def parse_device_public_key(payload):
    """64 bytes after the tag, or None when this is not a complete reply."""
    if not payload.startswith(RIDE_ON) or len(payload) < 72:
        return None
    if payload[6:8] == REQUEST_TAG:
        return None
    return payload[8:72]


def _curve_point(key):
    if key is None:
        return None
    try:
        ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), b"\x04" + key)
    except Exception:
        return None
    return key


def _notify_kwargs(uuid):
    # 00000004 is indicate, not notify. AcquireNotify misses that indication,
    # so the handshake reply never reaches the callback.
    if uuid == SYNC_TX:
        return {"bluez": {"use_start_notify": True}}
    return {}


class BleakControllerClient:
    """Thin Bleak wrapper. Tests substitute their own client."""

    def __init__(self, address):
        self.address = address
        self._client = None
        self._discovered = None

    def set_discovered(self, device):
        # The scan already found the BlueZ device. Connecting by address would
        # start another discovery while the trainer is connected, and BlueZ
        # aborts that connection.
        self._discovered = device

    async def connect(self):
        from bleak import BleakClient
        from bleak.exc import BleakError

        target = self._discovered or self.address
        try:
            await self._open_client(BleakClient(target, timeout=ATTEMPT_TIMEOUT))
        except BleakError as exc:
            # The scan's BlueZ object is gone after a long first attempt.
            # One address lookup, while the trainer is already quiet.
            if self._discovered is None or "not found" not in str(exc).lower():
                raise
            log.info("Controller device cache missed; scanning once")
            self._discovered = None
            await self._open_client(BleakClient(self.address, timeout=ATTEMPT_TIMEOUT))

    async def _open_client(self, client):
        self._client = client
        await client.connect()

    async def disconnect(self):
        client, self._client = self._client, None
        if client is None:
            return
        try:
            await client.disconnect()
        except Exception as exc:
            log.warning("Controller link close failed (%s)", type(exc).__name__)

    async def start_notify(self, uuid, callback):
        await self._client.start_notify(uuid, callback, **_notify_kwargs(uuid))

    async def write(self, uuid, data):
        # The control point is write-without-response. A write request still
        # returns, and the firmware never answers it.
        try:
            await self._client.write_gatt_char(uuid, data, response=False)
        except Exception:
            await self._client.write_gatt_char(uuid, data, response=True)


class ControllerLink:
    """One remote. `connect` tries twice, then stays failed."""

    def __init__(self, device, client_factory, attempt_timeout=ATTEMPT_TIMEOUT):
        self.address = device["address"]
        self.name = device.get("name")
        self.kind = device["kind"]
        self.side = device.get("side")
        self.discovered = device.get("ble_device")
        self.state = "disconnected"
        self.message = None
        self.client_factory = client_factory
        self.attempt_timeout = attempt_timeout
        self._client = None
        self._ready = None
        self._peer_key = None
        self._error = None
        self._cancel = asyncio.Event()
        self._attempt = 0
        self._run = True
        self._step = "open"
        self._answered = None

    def arm(self):
        """Allow a background connect. Disconnect wins if it lands first."""
        self._run = True
        self._cancel = asyncio.Event()

    def snapshot(self):
        return {
            "address": self.address,
            "name": self.name,
            "kind": self.kind,
            "side": self.side,
            "state": self.state,
            "message": self.message,
        }

    def _on_sync(self, _sender, data):
        # Bleak drops the subscription if this raises.
        ready = self._ready
        if ready is None or ready.is_set():
            return
        try:
            payload = bytes(data)
            point = _curve_point(parse_device_public_key(payload))
        except Exception:
            return
        if not payload.startswith(RIDE_ON) or len(payload) < 72 or payload[6:8] == REQUEST_TAG:
            return
        if point is None:
            # This firmware answers RideOn 02 03. The body is not a curve
            # point; disconnecting here is what turned the blue light off.
            self._answered = payload
            log.info("Controller handshake answered")
        else:
            self._peer_key = point
            log.info("Controller handshake key accepted")
        ready.set()

    def _on_async(self, _sender, data):
        try:
            report_kind(bytes(data))
        except Exception:
            return

    async def _exchange(self, client, public):
        self._step = "write"
        await client.write(SYNC_RX, handshake_request(public))
        self._step = "handshake"
        if self._peer_key is not None or self._answered is not None or self._cancel.is_set():
            return
        ready = asyncio.create_task(self._ready.wait())
        cancelled = asyncio.create_task(self._cancel.wait())
        try:
            await asyncio.wait({ready, cancelled}, return_when=asyncio.FIRST_COMPLETED)
        finally:
            for task in (ready, cancelled):
                if not task.done():
                    task.cancel()
            await asyncio.gather(ready, cancelled, return_exceptions=True)

    async def _open(self):
        self._ready = asyncio.Event()
        self._peer_key = None
        self._error = None
        self._answered = None
        private = ec.generate_private_key(ec.SECP256R1())
        public = private.public_key().public_bytes(Encoding.X962, PublicFormat.UncompressedPoint)
        client = self.client_factory(self.address)
        self._client = client
        setter = getattr(client, "set_discovered", None)
        if setter is not None:
            setter(self.discovered)
        self._step = "connect"
        await client.connect()
        self._step = "notify"
        await client.start_notify(SYNC_TX, self._on_sync)
        await client.start_notify(ASYNC_CHAR, self._on_async)
        await self._exchange(client, public)
        if self._cancel.is_set():
            return False
        if self._peer_key is None and self._answered is not None:
            return True
        if self._error is not None or self._peer_key is None:
            raise self._error or TimeoutError("handshake")
        peer = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), b"\x04" + self._peer_key)
        # Confirms the point can complete ECDH. The shared secret is not kept:
        # button decryption is not required to report the link as connected.
        private.exchange(ec.ECDH(), peer)
        self._peer_key = None
        return True

    async def _cleanup(self):
        client, self._client = self._client, None
        if client is None:
            return
        try:
            await client.disconnect()
        except Exception as exc:
            log.warning("Controller cleanup failed (%s)", type(exc).__name__)

    def _current(self, attempt):
        return attempt == self._attempt and not self._cancel.is_set()

    async def connect(self):
        # Synchronous until the first await, so a disconnect cannot land halfway
        # through this setup and be cleared.
        if not self._run:
            return
        self._attempt += 1
        attempt = self._attempt
        self._cancel.clear()
        if not self._run:
            return
        self.state = "connecting"
        self.message = None
        last = "controller_connect_failed"
        for _ignored in range(ATTEMPTS):
            if not self._current(attempt):
                return
            try:
                opened = await asyncio.wait_for(self._open(), self.attempt_timeout)
            except TimeoutError:
                last = "controller_connect_timeout"
                log.warning("Controller connect attempt timed out at %s", self._step)
                await self._cleanup()
                continue
            except Exception as exc:
                if not self._current(attempt):
                    return
                log.warning(
                    "Controller connect attempt failed at %s (%s)",
                    self._step, _radio_error(exc),
                )
                last = "controller_connect_failed"
                await self._cleanup()
                if getattr(exc, "dbus_error", None) and self._current(attempt):
                    await asyncio.sleep(0.4)
                continue
            if not self._current(attempt) or opened is False:
                await self._cleanup()
                return
            self.state = "connected"
            self.message = None
            return
        if not self._current(attempt):
            return
        await self._cleanup()
        self.state = "failed"
        self.message = last

    async def disconnect(self):
        self._run = False
        self._attempt += 1
        self._cancel.set()
        await self._cleanup()
        self.state = "disconnected"
        self.message = None


_MAC = re.compile(r"[0-9A-Fa-f]{2}(?:[:_-][0-9A-Fa-f]{2}){5}")
_PATH = re.compile(r"/org/\S+")


def _redact(text):
    return _PATH.sub("[path]", _MAC.sub("[redacted]", text))


def _radio_error(exc):
    """A short BlueZ or Bleak phrase. Addresses and D-Bus paths are removed."""
    name = getattr(exc, "dbus_error", None)
    detail = getattr(exc, "dbus_error_details", None)
    if isinstance(name, str) and name:
        short = name.rsplit(".", 1)[-1]
        if isinstance(detail, str) and detail:
            return f"{short}: {_redact(detail)[:120]}"
        return short
    text = _redact(str(exc).split("\n", 1)[0].strip())[:120]
    if text and text != type(exc).__name__:
        return f"{type(exc).__name__}: {text}"
    return type(exc).__name__


async def stop_discovery():
    """Discovery must be off before a second device connects."""
    if shutil.which("bluetoothctl") is None:
        return
    try:
        process = await asyncio.create_subprocess_exec(
            "bluetoothctl", "scan", "off",
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await asyncio.wait_for(process.wait(), 3)
    except (OSError, TimeoutError) as exc:
        log.warning("Stopping discovery failed (%s)", type(exc).__name__)


async def drop_bluez_connections(addresses):
    """Ask BlueZ to drop leftover links before a trainer connect.

    A previous agent can leave the trainer, or a Click/Play, connected.
    Bleak then waits until the connect command times out. Missing
    bluetoothctl is not an error.
    """
    unique = []
    for address in addresses:
        if address and address not in unique:
            unique.append(address)
    if not unique or shutil.which("bluetoothctl") is None:
        return
    for address in unique:
        try:
            process = await asyncio.create_subprocess_exec(
                "bluetoothctl", "disconnect", address,
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
            )
            await asyncio.wait_for(process.wait(), 3)
        except (OSError, TimeoutError) as exc:
            log.warning("Leftover controller disconnect failed (%s)", type(exc).__name__)
