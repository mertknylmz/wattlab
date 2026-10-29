"""Read live telemetry with bounded reconnects and cooperative shutdown."""
import asyncio
import logging
import signal
from bleak import BleakClient
from bleak.exc import BleakError
from config import trainer_address
from ftms_parser import parse_indoor_bike_data, parse_cycling_power
from trainer_connection import BIKE, POWER, TrainerConnection


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
    connection = TrainerConnection(
        trainer_address(), BleakClient, {BIKE: bike_update, POWER: power_update},
        retry_errors=(OSError, TimeoutError, BleakError),
        on_state=lambda state: print(f"Trainer: {state}"),
    )
    loop = asyncio.get_running_loop()
    signals = (signal.SIGINT, signal.SIGTERM)
    for sig in signals:
        loop.add_signal_handler(sig, connection.stop)
    try:
        await connection.run()
    finally:
        for sig in signals:
            loop.remove_signal_handler(sig)


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("Stopped.")
