import asyncio
from bleak import BleakClient
from config import trainer_address

async def main():
    print("Connecting to trainer...")
    async with BleakClient(trainer_address(), timeout=20.0) as client:
        print("Connected:", client.is_connected)
        for service in client.services:
            print(f"\\nSERVICE: {service.uuid} ({service.description})")
            for char in service.characteristics:
                print(f"  {char.uuid} | {char.properties}")

if __name__ == "__main__":
    asyncio.run(main())
