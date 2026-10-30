"""Read live telemetry with bounded reconnects and cooperative shutdown."""
import asyncio
import logging
import signal
import os
from bleak import BleakClient
from bleak.exc import BleakError
from config import trainer_address
from ftms_parser import parse_indoor_bike_data, parse_cycling_power
from trainer_connection import BIKE, POWER, TrainerConnection
from telemetry import TelemetryEvents


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
    # Read-only terminal use remains available without API configuration.
    address = trainer_address()
    api_url = os.getenv("WATTLAB_API_URL", "").strip()
    sender = None
    events = TelemetryEvents()
    if api_url:
        from telemetry_sender import TelemetrySender
        sender = TelemetrySender(api_url, os.getenv("WATTLAB_AGENT_TOKEN", ""))
        await sender.start()

    def bike(sender_char, data):
        bike_update(sender_char, data)
        if sender:
            try:
                sender.enqueue("telemetry", events.bike(parse_indoor_bike_data(data)))
            except ValueError:
                pass  # Already reported by terminal output.

    def power(sender_char, data):
        power_update(sender_char, data)
        if sender:
            try:
                sender.enqueue("telemetry", events.power(parse_cycling_power(data)))
            except ValueError:
                pass

    def state(value):
        print(f"Trainer: {value}")
        if sender:
            sender.enqueue("connection-events", events.state(value))

    connection = TrainerConnection(
        address, BleakClient, {BIKE: bike, POWER: power},
        retry_errors=(OSError, TimeoutError, BleakError),
        on_state=state,
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
        if sender:
            await sender.close()


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("Stopped.")
