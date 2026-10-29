"""Send synthetic events to the local API. No trainer or Bluetooth access."""
import asyncio
import os
from pathlib import Path
import sys
import aiohttp
from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'experiments' / 'bluetooth'))
from ftms_parser import BikeData
from telemetry import TelemetryEvents
from telemetry_sender import TelemetrySender


async def main():
    load_dotenv(ROOT / '.env')
    sender = TelemetrySender(os.environ['WATTLAB_API_URL'], os.environ['WATTLAB_API_TOKEN'])
    events = TelemetryEvents()
    await sender.start()
    try:
        sender.enqueue('connection-events', events.state('CONNECTING'))
        sender.enqueue('connection-events', events.state('STREAMING'))
        sender.enqueue('telemetry', events.bike(BikeData(speed_kmh=25.5, cadence_rpm=90, power_w=150)))
        sender.enqueue('telemetry', events.power(150))
        sender.enqueue('connection-events', events.state('STOPPED'))
        await asyncio.wait_for(sender.queue.join(), 20)
    finally:
        await sender.close()
    if sender.delivered != 5 or sender.dropped or sender.failed:
        raise RuntimeError('Synthetic telemetry delivery failed')
    async with aiohttp.ClientSession(headers={'Authorization': 'Bearer ' + sender.token}, trust_env=False,
                                     timeout=aiohttp.ClientTimeout(total=3)) as http:
        async with http.get(f'{sender.base_url}/api/v1/sessions/{sender.session_id}') as response:
            response.raise_for_status()
            session = await response.json()
    if not session.get('finished_at'):
        raise RuntimeError('Session finish was not persisted')
    print('Smoke test passed: 2 measurements, 3 connection events acknowledged; session finished.')


if __name__ == '__main__':
    asyncio.run(main())
