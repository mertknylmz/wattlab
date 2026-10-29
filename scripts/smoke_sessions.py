"""Verify the running API's session lifecycle over HTTP. No Bluetooth access."""
import asyncio
import os
from pathlib import Path
from uuid import uuid4

import aiohttp
from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parents[1]


async def main():
    load_dotenv(ROOT / '.env')
    # Use the same validated local-origin/token policy as the telemetry sender.
    import sys
    sys.path.insert(0, str(ROOT / 'experiments' / 'bluetooth'))
    from telemetry_sender import TelemetrySender
    settings = TelemetrySender(os.environ['WATTLAB_API_URL'], os.environ['WATTLAB_API_TOKEN'])
    async with aiohttp.ClientSession(headers={'Authorization': 'Bearer ' + settings.token},
                                     trust_env=False, timeout=aiohttp.ClientTimeout(total=3)) as http:
        async def call(method, path, payload=None, expected=200):
            async with http.request(method, settings.base_url + path, json=payload, allow_redirects=False) as response:
                if response.status != expected:
                    raise RuntimeError(f'Session API check failed with HTTP {response.status}')
                return await response.json()

        request = {'request_id': str(uuid4())}
        first = await call('POST', '/api/v1/sessions', request)
        repeated = await call('POST', '/api/v1/sessions', request)
        if first != repeated or first.get('finished_at') is not None:
            raise RuntimeError('Session creation is not idempotent')
        path = '/api/v1/sessions/' + str(uuid4())
        await call('GET', path, expected=404)
        path = '/api/v1/sessions/' + first['id']
        if await call('GET', path) != first:
            raise RuntimeError('Session read does not match persisted creation')
        finished = await call('POST', path + '/finish')
        if not finished.get('finished_at') or await call('POST', path + '/finish') != finished:
            raise RuntimeError('Session finish is not idempotent')
        if await call('POST', '/api/v1/sessions', request) != finished:
            raise RuntimeError('Creation retry reopened a finished session')
    print('Session smoke test passed: create/read/finish persisted; repeated requests preserved lifecycle.')


if __name__ == '__main__':
    asyncio.run(main())
