"""Bounded, asynchronous local API outbox. Never waits inside a BLE callback."""
import asyncio
import logging
import math
import re
from urllib.parse import urlsplit
from uuid import UUID, uuid4

import aiohttp

log = logging.getLogger(__name__)


class PermanentDeliveryError(RuntimeError):
    pass


def validate_local_api(base_url, token):
    """Only a credential-free loopback origin may receive the local token."""
    parsed = urlsplit(base_url)
    if (parsed.scheme != "http" or parsed.hostname not in ("127.0.0.1", "localhost", "::1")
            or parsed.username or parsed.password or parsed.query or parsed.fragment
            or parsed.path not in ("", "/")):
        raise ValueError("API URL must be an HTTP loopback origin without credentials")
    if not re.fullmatch(r"[A-Za-z0-9_-]{32,64}", token):
        raise ValueError("API token must contain 32 to 64 URL-safe characters")
    return base_url.rstrip('/')


class TelemetrySender:
    def __init__(self, base_url, token, *, command_id=None, capacity=512, max_retries=4,
                 retry_delay=1.0, timeout=2.0, shutdown_timeout=5.0):
        validate_local_api(base_url, token)
        if (type(capacity) is not int or capacity < 1 or type(max_retries) is not int or max_retries < 0
                or any(not math.isfinite(value) or value <= 0
                       for value in (retry_delay, timeout, shutdown_timeout))):
            raise ValueError("Invalid sender limits")
        if command_id is not None:
            UUID(str(command_id))
        self.base_url = base_url.rstrip('/')
        self.token = token
        self.command_id = str(command_id) if command_id is not None else None
        self.queue = asyncio.Queue(maxsize=capacity)
        self.max_retries = max_retries
        self.retry_delay = retry_delay
        self.timeout = timeout
        self.shutdown_timeout = shutdown_timeout
        self.request_id = str(uuid4())
        self.session_id = None
        self.delivered = 0
        self.dropped = 0
        self.failed = False
        self._accepting = False
        self._task = None
        self._http = None

    async def start(self):
        if self._task is not None:
            raise RuntimeError("Sender already started")
        self._http = aiohttp.ClientSession(
            headers={"Authorization": "Bearer " + self.token}, trust_env=False,
            timeout=aiohttp.ClientTimeout(total=self.timeout),
        )
        self._accepting = True
        self._task = asyncio.create_task(self._worker())

    async def ensure_session(self):
        """Create the session now instead of on the first event, so a session
        started before the trainer streams is still visible."""
        if self.session_id is None:
            body = {"request_id": self.request_id}
            if self.command_id:
                body["command_id"] = self.command_id
            result = await self._request('/api/v1/sessions', body)
            try:
                self.session_id = str(UUID(result['id']))
            except (KeyError, TypeError, ValueError, AttributeError) as exc:
                raise PermanentDeliveryError("API returned an invalid session") from exc
        return self.session_id

    def enqueue(self, kind, event):
        if event is None:
            return
        if kind not in ("telemetry", "connection-events"):
            raise ValueError("Unknown event kind")
        if not self._accepting or self.failed:
            self.dropped += 1
            return
        try:
            self.queue.put_nowait((kind, event))
        except asyncio.QueueFull:
            self.dropped += 1
            # Drop the newest event; preserve the ordering of queued observations.
            if self.dropped == 1 or self.dropped % 100 == 0:
                log.warning("Telemetry outbox full; dropped events: %d", self.dropped)

    async def _post(self, path, body):
        async with self._http.post(self.base_url + path, json=body, allow_redirects=False) as response:
            if response.status == 429 or response.status >= 500:
                raise OSError("Temporary API failure")
            if response.status != 200:
                # Never log response bodies, tokens, or full URLs.
                raise PermanentDeliveryError(f"API rejected request with HTTP {response.status}")
            try:
                return await response.json()
            except (ValueError, aiohttp.ContentTypeError) as exc:
                raise PermanentDeliveryError("API returned an invalid acknowledgement") from exc

    async def _request(self, path, body):
        for attempt in range(self.max_retries + 1):
            try:
                return await self._post(path, body)
            except (aiohttp.ClientError, OSError, TimeoutError):
                if attempt == self.max_retries:
                    raise
                await asyncio.sleep(min(self.retry_delay * 2 ** attempt, 8))

    async def _worker(self):
        while True:
            kind, event = await self.queue.get()
            try:
                if self.failed:
                    self.dropped += 1
                    continue
                # The server value is validated before it is used in a path.
                await self.ensure_session()
                result = await self._request(f'/api/v1/sessions/{self.session_id}/{kind}', {"events": [event]})
                if not isinstance(result, dict) or result.get('accepted') != 1:
                    raise PermanentDeliveryError("API did not acknowledge the event")
                self.delivered += 1
            except PermanentDeliveryError as exc:
                self.failed = True
                self.dropped += 1
                log.error("%s; API delivery disabled, BLE collection continues", exc)
            except (aiohttp.ClientError, OSError, TimeoutError):
                self.dropped += 1
                log.warning("API retry budget exhausted; event dropped, BLE collection continues")
            except asyncio.CancelledError:
                self.dropped += 1
                raise
            finally:
                self.queue.task_done()

    async def close(self):
        if self._http is None:
            return
        self._accepting = False
        try:
            try:
                await asyncio.wait_for(self.queue.join(), self.shutdown_timeout)
            except TimeoutError:
                log.warning("API outbox shutdown budget exhausted")
            finally:
                self._task.cancel()
                await asyncio.gather(self._task, return_exceptions=True)
                while not self.queue.empty():
                    self.queue.get_nowait()
                    self.queue.task_done()
                    self.dropped += 1
            if self.session_id is not None and not self.failed:
                try:
                    await asyncio.wait_for(
                        self._request(f'/api/v1/sessions/{self.session_id}/finish', {}),
                        self.shutdown_timeout,
                    )
                except (aiohttp.ClientError, OSError, TimeoutError, PermanentDeliveryError):
                    log.warning("Session finish could not be confirmed")
        finally:
            await self._http.close()
            self._http = None
            log.info("API delivery complete: delivered=%d dropped=%d", self.delivered, self.dropped)
