"""HTTP side of the trainer agent: command long poll, results and heartbeat."""
import aiohttp

from telemetry_sender import validate_local_api

LONG_POLL_SECONDS = 20


class AgentApiError(RuntimeError):
    pass


class AgentClient:
    def __init__(self, base_url, token, *, timeout=3.0):
        self.base_url = validate_local_api(base_url, token)
        self.token = token
        self.timeout = timeout
        self._http = None

    async def __aenter__(self):
        self._http = aiohttp.ClientSession(
            headers={"Authorization": "Bearer " + self.token}, trust_env=False)
        return self

    async def __aexit__(self, *_):
        await self._http.close()
        self._http = None

    async def _call(self, method, path, *, json=None, timeout=None):
        budget = aiohttp.ClientTimeout(total=timeout or self.timeout)
        async with self._http.request(method, self.base_url + path, json=json, timeout=budget,
                                      allow_redirects=False) as response:
            if response.status == 204:
                return None
            if response.status != 200:
                # Never log bodies, tokens or full URLs.
                raise AgentApiError(f"API answered HTTP {response.status}")
            try:
                return await response.json()
            except (ValueError, aiohttp.ContentTypeError) as exc:
                raise AgentApiError("API returned invalid JSON") from exc

    async def next_command(self):
        return await self._call("GET", f"/api/v1/agent/commands/next?wait_seconds={LONG_POLL_SECONDS}",
                                timeout=LONG_POLL_SECONDS + 5)

    async def report(self, command_id, succeeded, message=None):
        body = {"status": "succeeded" if succeeded else "rejected", "message": message}
        await self._call("PUT", f"/api/v1/agent/commands/{command_id}/result", json=body)

    async def heartbeat(self, status):
        await self._call("PUT", "/api/v1/agent/status", json=status)
