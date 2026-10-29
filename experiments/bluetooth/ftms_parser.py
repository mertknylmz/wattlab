"""Bluetooth FTMS Indoor Bike Data (0x2AD2) parser."""
from dataclasses import dataclass
import struct


@dataclass
class BikeData:
    speed_kmh: float | None = None
    cadence_rpm: float | None = None
    power_w: int | None = None
    average_speed_kmh: float | None = None
    average_cadence_rpm: float | None = None
    total_distance_m: int | None = None
    resistance_level: int | None = None
    average_power_w: int | None = None
    total_energy_kcal: int | None = None
    energy_per_hour_kcal: int | None = None
    energy_per_minute_kcal: int | None = None
    heart_rate_bpm: int | None = None
    metabolic_equivalent: float | None = None
    elapsed_time_s: int | None = None
    remaining_time_s: int | None = None


def parse_indoor_bike_data(data: bytes) -> BikeData:
    if len(data) < 2:
        raise ValueError("FTMS packet too short for flags")
    flags = int.from_bytes(data[:2], "little")
    offset = 2
    result = BikeData()

    def read(fmt: str):
        nonlocal offset
        size = struct.calcsize("<" + fmt)
        if offset + size > len(data):
            raise ValueError(f"Truncated FTMS packet at byte {offset}: {data.hex(' ')}")
        value = struct.unpack_from("<" + fmt, data, offset)[0]
        offset += size
        return value

    # Bit 0 is inverted: 0 means instantaneous speed IS present.
    if not flags & (1 << 0):
        result.speed_kmh = read("H") / 100
    if flags & (1 << 1):
        result.average_speed_kmh = read("H") / 100
    if flags & (1 << 2):
        result.cadence_rpm = read("H") / 2
    if flags & (1 << 3):
        result.average_cadence_rpm = read("H") / 2
    if flags & (1 << 4):
        result.total_distance_m = int.from_bytes(data[offset:offset+3], "little")
        if len(data[offset:offset+3]) != 3:
            raise ValueError("Truncated total distance")
        offset += 3
    if flags & (1 << 5):
        result.resistance_level = read("h")
    if flags & (1 << 6):
        result.power_w = read("h")
    if flags & (1 << 7):
        result.average_power_w = read("h")
    if flags & (1 << 8):
        result.total_energy_kcal = read("H")
        result.energy_per_hour_kcal = read("H")
        result.energy_per_minute_kcal = read("B")
    if flags & (1 << 9):
        result.heart_rate_bpm = read("B")
    if flags & (1 << 10):
        result.metabolic_equivalent = read("B") / 10
    if flags & (1 << 11):
        result.elapsed_time_s = read("H")
    if flags & (1 << 12):
        result.remaining_time_s = read("H")
    return result


def parse_cycling_power(data: bytes) -> int:
    """Cycling Power Measurement (0x2A63): signed instantaneous watts at bytes 2-3."""
    if len(data) < 4:
        raise ValueError("Cycling Power packet too short")
    return struct.unpack_from("<h", data, 2)[0]
