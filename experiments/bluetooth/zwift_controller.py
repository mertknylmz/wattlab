"""Zwift Click / Play connection. BLE stays here; Rust only sees the outcome.

Both remotes speak the RideOn handshake on the Zwift custom service: we write
our uncompressed secp256r1 public key, and the device answers with its own.
A bad public key is ignored inside the notify callback so Bleak keeps the
subscription. AES-CCM button decryption is not part of connecting.
"""
import asyncio
import logging
import shutil

from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

from zwift_buttons import report_kind

log = logging.getLogger(__name__)

RIDE_ON = b"RideOn"
# App hello is 0x01 0x02. Devices answer 0x01 0x03, 0x01 0x04, or 0x00 0x09.
REQUEST_TAG = b"\x01\x02"
RESPONSE_TAGS = (b"\x01\x03", b"\x01\x04", b"\x00\x09")
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
    """64-byte device key, None if this frame is not a handshake reply.

    A tagged reply with the wrong length raises ValueError. The notify
    callback catches that so one bad frame does not cancel the subscription.
    """
    if not payload.startswith(RIDE_ON) or len(payload) < 8:
        return None
    if payload[6:8] not in RESPONSE_TAGS:
        return None
    key = payload[8:]
    if len(key) != 64:
        raise ValueError("invalid device key")
    return key


class BleakControllerClient:
    """Thin Bleak wrapper. Tests substitute their own client."""

    def __init__(self, address):
        self.address = address
        self._client = None

    async def connect(self):
        from bleak import BleakClient

        self._client = BleakClient(self.address, timeout=ATTEMPT_TIMEOUT)
        await self._client.connect()

    async def disconnect(self):
        client, self._client = self._client, None
        if client is None:
            return
        try:
            await client.disconnect()
        except Exception as exc:
            log.warning("Controller link close failed (%s)", type(exc).__name__)

    async def start_notify(self, uuid, callback):
        await self._client.start_notify(uuid, callback)

    async def write(self, uuid, data):
        await self._client.write_gatt_char(uuid, data, response=True)


class ControllerLink:
    """One remote. `connect` tries twice, then stays failed."""

    def __init__(self, device, client_factory, attempt_timeout=ATTEMPT_TIMEOUT):
        self.address = device["address"]
        self.name = device.get("name")
        self.kind = device["kind"]
        self.side = device.get("side")
        self.state = "disconnected"
        self.message = None
        self.client_factory = client_factory
        self.attempt_timeout = attempt_timeout
        self._client = None
        self._ready = None
        self._peer_key = None
        self._error = None

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
        # Bleak drops the subscription if this raises. Invalid RideOn material
        # (including a point that is not on the curve) stays inside this call.
        ready = self._ready
        if ready is None or ready.is_set():
            return
        try:
            key = parse_device_public_key(bytes(data))
            if key is None:
                return
            ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), b"\x04" + key)
        except Exception:
            self._error = ValueError("invalid device key")
            ready.set()
            return
        self._peer_key = key
        ready.set()

    def _on_async(self, _sender, data):
        try:
            report_kind(bytes(data))
        except Exception:
            return

    async def _open(self):
        self._ready = asyncio.Event()
        self._peer_key = None
        self._error = None
        private = ec.generate_private_key(ec.SECP256R1())
        public = private.public_key().public_bytes(Encoding.X962, PublicFormat.UncompressedPoint)
        client = self.client_factory(self.address)
        self._client = client
        await client.connect()
        await client.start_notify(SYNC_TX, self._on_sync)
        await client.start_notify(ASYNC_CHAR, self._on_async)
        await client.write(SYNC_RX, handshake_request(public))
        await self._ready.wait()
        if self._error is not None or self._peer_key is None:
            raise self._error or TimeoutError("handshake")
        peer = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), b"\x04" + self._peer_key)
        # Confirms the point can complete ECDH. The shared secret is not kept:
        # button decryption is not required to report the link as connected.
        private.exchange(ec.ECDH(), peer)
        self._peer_key = None

    async def _cleanup(self):
        client, self._client = self._client, None
        if client is None:
            return
        try:
            await client.disconnect()
        except Exception as exc:
            log.warning("Controller cleanup failed (%s)", type(exc).__name__)

    async def connect(self):
        self.state = "connecting"
        self.message = None
        last = "controller_connect_failed"
        for _attempt in range(ATTEMPTS):
            try:
                await asyncio.wait_for(self._open(), self.attempt_timeout)
            except TimeoutError:
                last = "controller_connect_timeout"
                await self._cleanup()
                continue
            except Exception as exc:
                log.warning("Controller connect attempt failed (%s)", type(exc).__name__)
                last = "controller_connect_failed"
                await self._cleanup()
                continue
            self.state = "connected"
            self.message = None
            return
        await self._cleanup()
        self.state = "failed"
        self.message = last

    async def disconnect(self):
        await self._cleanup()
        self.state = "disconnected"
        self.message = None


async def drop_bluez_connections(addresses):
    """Ask BlueZ to drop leftover Click/Play links before a trainer connect.

    Pedals left connected by another process keep the adapter busy and the
    trainer connect times out. Missing bluetoothctl is not an error.
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
