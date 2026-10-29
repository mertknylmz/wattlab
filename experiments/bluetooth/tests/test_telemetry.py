import asyncio
import sys
import unittest
from pathlib import Path
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from ftms_parser import BikeData
from telemetry import TelemetryEvents
from telemetry_sender import TelemetrySender, PermanentDeliveryError


class EventTests(unittest.TestCase):
    def test_sources_missing_values_and_sequence(self):
        events = TelemetryEvents()
        connecting = events.state('CONNECTING')
        ftms = events.bike(BikeData(power_w=-10))
        power = events.power(100)
        self.assertEqual([connecting['sequence'], ftms['sequence'], power['sequence']], [0, 1, 2])
        self.assertIsNone(ftms['speed_kmh'])
        self.assertIsNone(power['cadence_rpm'])
        self.assertEqual(ftms['source'], 'ftms')
        self.assertEqual(power['source'], 'cycling_power')
        self.assertEqual(ftms['connection_id'], connecting['connection_id'])
        self.assertEqual(power['stream_id'], ftms['stream_id'])
        self.assertNotEqual(events.state('CONNECTING')['connection_id'], connecting['connection_id'])
        self.assertIsNone(events.bike(BikeData()))

    def test_only_loopback_origin_is_accepted(self):
        for url in ('https://example.com', 'http://127.0.0.1.evil.test',
                    'http://user:password@localhost', 'http://localhost/path'):
            with self.subTest(url=url), self.assertRaises(ValueError):
                TelemetrySender(url, 'x' * 32)


class SenderTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.senders = []

    async def asyncTearDown(self):
        for sender in self.senders:
            await sender.close()
        pending = [task for task in asyncio.all_tasks()
                   if task is not asyncio.current_task() and not task.done()]
        self.assertEqual(pending, [], 'Sender tasks leaked')

    async def sender(self, **kwargs):
        sender = TelemetrySender('http://127.0.0.1:8080', 'x' * 32, retry_delay=.001,
                                 shutdown_timeout=.02, **kwargs)
        await sender.start()
        self.senders.append(sender)
        return sender

    async def test_api_outage_does_not_block_collection_and_queue_is_bounded(self):
        sender = await self.sender(capacity=2, max_retries=0)

        async def unavailable(path, body):
            raise OSError('unavailable')

        sender._post = unavailable
        with self.assertLogs('telemetry_sender', level='WARNING'):
            for _ in range(20):
                sender.enqueue('telemetry', TelemetryEvents().power(100))
            await asyncio.wait_for(sender.queue.join(), .5)
        self.assertEqual(sender.delivered, 0)
        self.assertEqual(sender.dropped, 20)
        self.assertFalse(sender.failed)
        self.assertEqual(sender.queue.maxsize, 2)

    async def test_retry_uses_identical_payload_and_stable_start_request(self):
        sender = await self.sender()
        session_id = str(uuid4())
        calls = []
        attempts = 0

        async def post(path, body):
            nonlocal attempts
            calls.append((path, body))
            if path == '/api/v1/sessions':
                attempts += 1
                if attempts == 1:
                    raise OSError('lost acknowledgement')
                return {'id': session_id}
            return {'accepted': 1}

        sender._post = post
        event = TelemetryEvents().power(123)
        sender.enqueue('telemetry', event)
        await asyncio.wait_for(sender.queue.join(), .5)
        await sender.close()
        self.assertEqual(calls[0], calls[1])
        self.assertEqual(calls[2][1], {'events': [event]})
        self.assertTrue(calls[-1][0].endswith('/finish'))
        self.assertEqual(sender.delivered, 1)
        self.assertEqual(sender.dropped, 0)

    async def test_auth_or_validation_failure_disables_delivery_only(self):
        sender = await self.sender()

        async def rejected(path, body):
            raise PermanentDeliveryError('rejected')

        sender._post = rejected
        with self.assertLogs('telemetry_sender', level='ERROR'):
            sender.enqueue('telemetry', TelemetryEvents().power(100))
            await asyncio.wait_for(sender.queue.join(), .5)
        sender.enqueue('telemetry', TelemetryEvents().power(200))
        self.assertTrue(sender.failed)
        self.assertEqual(sender.dropped, 2)

    async def test_shutdown_cancels_inflight_and_counts_unsent_events(self):
        sender = await self.sender()

        async def blocked(path, body):
            await asyncio.Event().wait()

        sender._post = blocked
        sender.enqueue('telemetry', TelemetryEvents().power(1))
        sender.enqueue('telemetry', TelemetryEvents().power(2))
        with self.assertLogs('telemetry_sender', level='WARNING'):
            await asyncio.wait_for(sender.close(), .5)
        self.assertEqual(sender.dropped, 2)
        self.assertTrue(sender._task.done())

    async def test_cancelled_close_still_cleans_worker(self):
        sender = await self.sender()

        async def blocked(path, body):
            await asyncio.Event().wait()

        sender._post = blocked
        sender.enqueue('telemetry', TelemetryEvents().power(1))
        closing = asyncio.create_task(sender.close())
        await asyncio.sleep(0)
        closing.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await closing
        self.assertTrue(sender._task.done())
        self.assertIsNone(sender._http)

    async def test_bad_acknowledgement_is_not_counted_as_delivery(self):
        sender = await self.sender()

        async def wrong(path, body):
            return {'id': str(uuid4())} if path == '/api/v1/sessions' else {'accepted': 0}

        sender._post = wrong
        with self.assertLogs('telemetry_sender', level='ERROR'):
            sender.enqueue('telemetry', TelemetryEvents().power(1))
            await asyncio.wait_for(sender.queue.join(), .5)
        self.assertTrue(sender.failed)
        self.assertEqual(sender.delivered, 0)


if __name__ == '__main__':
    unittest.main()
