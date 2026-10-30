"""Send synthetic events to the local API. No trainer or Bluetooth access."""
import asyncio
import os
from pathlib import Path
import sys
from uuid import uuid4
import aiohttp
from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'experiments' / 'bluetooth'))
from ftms_parser import BikeData
from telemetry import TelemetryEvents
from telemetry_sender import TelemetrySender


async def main():
    load_dotenv(ROOT / '.env')
    origin = os.environ['WATTLAB_API_URL'].rstrip('/')
    agent = os.environ['WATTLAB_AGENT_TOKEN']
    timeout = aiohttp.ClientTimeout(total=3)
    async with aiohttp.ClientSession(trust_env=False, timeout=timeout) as http:
        username = f"t{uuid4().hex[:12]}"
        async with http.post(origin + '/api/v1/auth/register',
                             json={'username': username, 'password': 'correct horse'},
                             allow_redirects=False) as response:
            response.raise_for_status()
            member = (await response.json())['token']
        async with http.post(origin + '/api/v1/agent/commands', json={'kind': 'start_session'},
                             headers={'Authorization': 'Bearer ' + member}, allow_redirects=False) as response:
            if response.status != 202:
                raise RuntimeError(f'Command submit failed with HTTP {response.status}')
            command_id = (await response.json())['id']
        async with http.get(origin + '/api/v1/agent/commands/next?wait_seconds=0',
                            headers={'Authorization': 'Bearer ' + agent}, allow_redirects=False) as response:
            response.raise_for_status()

    sender = TelemetrySender(origin, agent, command_id=command_id)
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
    async with aiohttp.ClientSession(headers={'Authorization': 'Bearer ' + agent}, trust_env=False,
                                     timeout=aiohttp.ClientTimeout(total=3)) as http:
        async with http.get(f'{origin}/api/v1/sessions/{sender.session_id}') as response:
            response.raise_for_status()
            session = await response.json()
    if not session.get('finished_at'):
        raise RuntimeError('Session finish was not persisted')
    print('Smoke test passed: 2 measurements, 3 connection events acknowledged; session finished.')


if __name__ == '__main__':
    asyncio.run(main())
