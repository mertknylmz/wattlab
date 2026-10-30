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
    import sys
    sys.path.insert(0, str(ROOT / 'experiments' / 'bluetooth'))
    from telemetry_sender import TelemetrySender
    settings = TelemetrySender(os.environ['WATTLAB_API_URL'], os.environ['WATTLAB_AGENT_TOKEN'])
    timeout = aiohttp.ClientTimeout(total=3)
    async with aiohttp.ClientSession(trust_env=False, timeout=timeout) as http:
        async def call(method, path, payload=None, expected=200, token=None):
            headers = {'Authorization': 'Bearer ' + (token or settings.token)}
            async with http.request(method, settings.base_url + path, json=payload, headers=headers,
                                    allow_redirects=False) as response:
                if response.status != expected:
                    raise RuntimeError(f'Session API check failed with HTTP {response.status}')
                if response.status == 204:
                    return None
                return await response.json()

        username = f"s{uuid4().hex[:12]}"
        registered = await call('POST', '/api/v1/auth/register',
                                {'username': username, 'password': 'correct horse'}, 201, token='')
        member = registered['token']
        command = await call('POST', '/api/v1/agent/commands', {'kind': 'start_session'}, 202, token=member)
        await call('GET', '/api/v1/agent/commands/next?wait_seconds=0')
        request = {'request_id': str(uuid4()), 'command_id': command['id']}
        first = await call('POST', '/api/v1/sessions', request)
        repeated = await call('POST', '/api/v1/sessions', request)
        if first != repeated or first.get('finished_at') is not None:
            raise RuntimeError('Session creation is not idempotent')
        await call('GET', '/api/v1/sessions/' + str(uuid4()), expected=404)
        if await call('GET', '/api/v1/sessions/' + first['id']) != first:
            raise RuntimeError('Session read does not match persisted creation')
        finished = await call('POST', '/api/v1/sessions/' + first['id'] + '/finish')
        if not finished.get('finished_at') or await call('POST', '/api/v1/sessions/' + first['id'] + '/finish') != finished:
            raise RuntimeError('Session finish is not idempotent')
        if await call('POST', '/api/v1/sessions', request) != finished:
            raise RuntimeError('Creation retry reopened a finished session')
    print('Session smoke test passed: create/read/finish persisted; repeated requests preserved lifecycle.')


if __name__ == '__main__':
    asyncio.run(main())
