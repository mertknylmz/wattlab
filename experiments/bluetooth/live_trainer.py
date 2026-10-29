"""Read live FTMS data and cycling power."""
import asyncio
from bleak import BleakClient
from config import trainer_address
from ftms_parser import parse_indoor_bike_data, parse_cycling_power

BIKE = "00002ad2-0000-1000-8000-00805f9b34fb"
POWER = "00002a63-0000-1000-8000-00805f9b34fb"

def bike_update(_, data):
    try:
        v = parse_indoor_bike_data(data)
        print(f"FTMS | speed={v.speed_kmh} km/h cadence={v.cadence_rpm} RPM power={v.power_w} W")
    except ValueError as exc:
        print(f"FTMS parse error: {exc}")

def power_update(_, data):
    try:
        print(f"CYCLING POWER | {parse_cycling_power(data)} W")
    except ValueError as exc:
        print(f"Power parse error: {exc}")

async def main():
    async with BleakClient(trainer_address(), timeout=20.0) as client:
        print("Connected. Start pedaling; Ctrl+C to stop.")
        await client.start_notify(BIKE, bike_update)
        await client.start_notify(POWER, power_update)
        try:
            await asyncio.Event().wait()
        finally:
            await client.stop_notify(POWER)
            await client.stop_notify(BIKE)

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("Stopped.")
