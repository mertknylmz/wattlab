"""Nearby BLE trainers for the picker. The agent owns the radio."""
import re

FTMS = "00001826-0000-1000-8000-00805f9b34fb"
CYCLING_POWER = "00001818-0000-1000-8000-00805f9b34fb"
SCAN_SECONDS = 5.0
TRAINER_NAME = re.compile(
    r"d500|trainer|kickr|tacx|wahoo|direto|suito|neo|flux|snap|saris|stages|"
    r"elite|van\s*rysel|indoor\s*bike|smart\s*bike|ftms|cycling",
    re.I,
)
NOISE_NAME = re.compile(
    r"buds|airpods|headphone|razer|mouse|keyboard|watch|phone|galaxy|iphone|pixel",
    re.I,
)


def device_entry(address, name=None, rssi=None, service_uuids=()):
    """Normalize one advertisement. Missing names stay None."""
    services = {str(uuid).lower() for uuid in service_uuids}
    return {
        "address": address,
        "name": name or None,
        "rssi": rssi,
        "likely_trainer": FTMS in services or CYCLING_POWER in services,
    }


def is_trainer(entry):
    """Keep FTMS/power bikes and named trainers; drop phones, buds and bare MACs."""
    if entry.get("likely_trainer"):
        return True
    name = entry.get("name") or ""
    if not name or NOISE_NAME.search(name):
        return False
    return bool(TRAINER_NAME.search(name))


def rank_devices(devices):
    trainers = [item for item in devices if is_trainer(item)]
    return sorted(trainers, key=lambda item: (not item.get("likely_trainer"), -(item.get("rssi") or -999)))


async def discover_devices(timeout=SCAN_SECONDS):
    from bleak import BleakScanner

    found = await BleakScanner.discover(timeout=timeout, return_adv=True)
    devices = []
    for device, advertisement in found.values():
        devices.append(
            device_entry(
                device.address,
                device.name or advertisement.local_name,
                advertisement.rssi,
                advertisement.service_uuids or (),
            )
        )
    return rank_devices(devices)
