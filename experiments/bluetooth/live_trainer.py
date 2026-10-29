import asyncio
from bleak import BleakClient
from config import trainer_address

INDOOR_BIKE_DATA = "00002ad2-0000-1000-8000-00805f9b34fb"

def on_data(_sender, data: bytearray):
    # Raw data first: flags determine which fields are present.
    print("Indoor Bike Data:", data.hex(" "))

async def main():
    async with BleakClient(trainer_address(), timeout=20.0) as client:
        print("Connected. Pedal to receive notifications; Ctrl+C to stop.")
        await client.start_notify(INDOOR_BIKE_DATA, on_data)
        try:
            await asyncio.Event().wait()
        finally:
            await client.stop_notify(INDOOR_BIKE_DATA)

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\\nStopped.")
