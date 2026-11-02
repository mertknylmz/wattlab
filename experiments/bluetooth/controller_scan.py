"""Nearby Zwift Click and Play controllers. The agent owns the radio."""
import re

# Zwift custom controller service. Zwift Ride uses it too, so the UUID alone
# is not enough to call a device a Click or a Play.
ZWIFT_CONTROLLER_SERVICE = "00000001-19ca-4651-86e5-fa29dcdd09d1"
# Bluetooth company id. Bleak hands us the payload after these two bytes.
ZWIFT_COMPANY_ID = 0x094A
# Manufacturer side byte: Play right, Play left, Click.
SIDE_BYTE = {2: ("play", "right"), 3: ("play", "left"), 9: ("click", None)}
SCAN_SECONDS = 5.0
MAX_CONTROLLERS = 2


def _side_from_name(name):
    if re.search(r"\bleft\b", name, re.I):
        return "left"
    if re.search(r"\bright\b", name, re.I):
        return "right"
    return None


def _kind_from_name(name):
    lowered = name.lower()
    if "zwift" not in lowered:
        return None
    if "click" in lowered:
        return "click"
    if "play" in lowered:
        return "play"
    return None


def _side_byte(manufacturer_data):
    """Return (kind, side) from a Zwift advertisement, or None."""
    if not manufacturer_data:
        return None
    payload = manufacturer_data.get(ZWIFT_COMPANY_ID)
    if payload is None:
        # A raw field still starts with the little-endian company id.
        for raw in manufacturer_data.values():
            if len(raw) >= 3 and int.from_bytes(raw[:2], "little") == ZWIFT_COMPANY_ID:
                payload = raw[2:]
                break
    if not payload:
        return None
    return SIDE_BYTE.get(payload[0])


def classify_controller(address, name=None, rssi=None, service_uuids=(), manufacturer_data=None):
    """One Click or Play advertisement, or None for everything else.

    The custom service is shared with Zwift Ride, so it never identifies a
    controller by itself. A Zwift Click/Play name or the company side byte does.
    """
    identified = _side_byte(manufacturer_data)
    label = name or ""
    if identified:
        kind, side = identified
    else:
        kind = _kind_from_name(label)
        side = _side_from_name(label) if kind == "play" else None
    if kind is None:
        # service_uuids may include ZWIFT_CONTROLLER_SERVICE. Ride uses that
        # UUID too, so it is not an identifier. Only the side byte or the name is.
        return None
    if kind == "click":
        side = None
    return {
        "address": address,
        "name": name or None,
        "rssi": rssi,
        "kind": kind,
        "side": side,
    }


def choose_controllers(devices):
    """Up to two controllers. A Play left/right pair wins over a louder third device."""
    plays = [item for item in devices if item.get("kind") == "play"]

    def strongest(side):
        group = [item for item in plays if item.get("side") == side]
        if not group:
            return None
        return max(group, key=lambda item: item.get("rssi") or -999)

    left, right = strongest("left"), strongest("right")
    if left and right:
        return [left, right]
    ranked = sorted(devices, key=lambda item: -(item.get("rssi") or -999))
    return ranked[:MAX_CONTROLLERS]


def rank_controllers(devices):
    ranked = sorted(devices, key=lambda item: -(item.get("rssi") or -999))
    chosen = {item["address"] for item in choose_controllers(ranked)}
    for item in ranked:
        item["selected"] = item["address"] in chosen
    return ranked


async def discover_controllers(timeout=SCAN_SECONDS):
    from bleak import BleakScanner

    found = await BleakScanner.discover(timeout=timeout, return_adv=True)
    devices = []
    for device, advertisement in found.values():
        entry = classify_controller(
            device.address,
            device.name or advertisement.local_name,
            advertisement.rssi,
            advertisement.service_uuids or (),
            advertisement.manufacturer_data or {},
        )
        if entry:
            # Kept for the connect call. The heartbeat copies named fields only.
            entry["ble_device"] = device
            devices.append(entry)
    return rank_controllers(devices)
