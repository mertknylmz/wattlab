import asyncio
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from trainer_connection import (BIKE, POWER, ConnectionSettings, TrainerConnection,
                                UnsupportedTrainer, ReconnectExhausted, CleanupFailed)
from ftms_parser import parse_indoor_bike_data, parse_cycling_power


class FakeClient:
    def __init__(self, chars=(BIKE, POWER), *, connect_error=False, fail_second=False,
                 stop_error=False, hang_connect=False, hang_stop=False, hang_subscribe=False):
        self.chars = {uuid: SimpleNamespace(uuid=uuid, properties=['notify']) for uuid in chars}
        self.services = SimpleNamespace(get_characteristic=self.chars.get)
        self.connect_error = connect_error
        self.fail_second = fail_second
        self.stop_error = stop_error
        self.hang_connect = hang_connect
        self.hang_stop = hang_stop
        self.hang_subscribe = hang_subscribe
        self.subscribing = asyncio.Event()
        self.is_connected = False
        self.started = []
        self.stopped = []
        self.callbacks = []
        self.disconnects = 0
        self.disconnect_error = False
        self.entered = asyncio.Event()

    async def connect(self):
        self.entered.set()
        if self.connect_error:
            raise OSError('unavailable')
        if self.hang_connect:
            await asyncio.Event().wait()
        self.is_connected = True

    async def start_notify(self, char, callback):
        self.subscribing.set()
        if self.hang_subscribe:
            await asyncio.Event().wait()
        if self.fail_second and self.started:
            raise OSError('subscription failed')
        self.started.append(char.uuid)
        self.callbacks.append(callback)

    async def stop_notify(self, char):
        self.stopped.append(char.uuid)
        if self.hang_stop:
            await asyncio.Event().wait()
        if self.stop_error:
            raise OSError('cleanup failed')

    async def disconnect(self):
        self.disconnects += 1
        if self.disconnect_error:
            raise OSError("adapter unavailable")
        self.is_connected = False
        self.lost(self)

    def drop(self):
        self.is_connected = False
        self.lost(self)


class ConnectionTests(unittest.IsolatedAsyncioTestCase):
    async def asyncTearDown(self):
        pending = [task for task in asyncio.all_tasks()
                   if task is not asyncio.current_task() and not task.done()]
        self.assertEqual(pending, [], "Connection tasks leaked")

    def manager(self, clients, **settings):
        self.created = []
        self.states = []
        self.values = []
        self.streaming = asyncio.Queue()

        def factory(*args, **kwargs):
            client = clients[len(self.created)]
            client.lost = kwargs['disconnected_callback']
            self.created.append(client)
            return client

        def state(value):
            self.states.append(value)
            if value == 'STREAMING':
                self.streaming.put_nowait(value)

        options = dict(retry_delay=.001, cleanup_timeout=.02, timeout=.1)
        options.update(settings)
        return TrainerConnection('test', factory, {BIKE: self.receive, POWER: self.receive},
                                 settings=ConnectionSettings(**options), on_state=state)

    def receive(self, sender, data):
        self.values.append(data)

    async def ready(self):
        await asyncio.wait_for(self.streaming.get(), .5)

    async def test_disconnect_reconnect_and_stale_callbacks(self):
        first, second = FakeClient(), FakeClient(chars=(POWER,))
        manager = self.manager([first, second])
        task = asyncio.create_task(manager.run())
        await self.ready()
        first.callbacks[0](None, b'first')
        first.drop()
        await self.ready()
        first.callbacks[0](None, b'stale')
        second.callbacks[0](None, b'new')
        manager.stop()
        await asyncio.wait_for(task, .5)
        self.assertEqual(self.values, [b'first', b'new'])
        self.assertEqual(second.started, [POWER])
        self.assertEqual([c.disconnects for c in self.created], [1, 1])

    async def test_ready_hook_subscriptions_are_cleaned_and_failures_do_not_block_telemetry(self):
        extra = SimpleNamespace(uuid='control', properties=['indicate'])
        lost = []

        async def ready(client):
            return [extra]

        async def broken(client):
            raise OSError('optional capability failed')

        for hook, expected_stop in ((ready, ['control', POWER, BIKE]), (broken, [POWER, BIKE])):
            client = FakeClient()
            manager = self.manager([client])
            manager.on_ready, manager.on_lost = hook, lambda: lost.append(True)
            task = asyncio.create_task(manager.run())
            await self.ready()
            manager.stop()
            await asyncio.wait_for(task, .5)
            self.assertEqual(client.stopped, expected_stop)
        self.assertEqual(lost, [True, True])

    async def test_missing_characteristics_is_not_retried(self):
        client = FakeClient(chars=())
        manager = self.manager([client])
        with self.assertRaises(UnsupportedTrainer):
            await manager.run()
        self.assertEqual(client.disconnects, 1)
        self.assertEqual(len(self.created), 1)

    async def test_non_notifying_characteristics_rejected(self):
        client = FakeClient(chars=(BIKE,))
        client.chars[BIKE].properties = ['read']
        with self.assertRaises(UnsupportedTrainer):
            await self.manager([client]).run()

    async def test_partial_subscription_failure_cleans_first(self):
        client = FakeClient(fail_second=True)
        with self.assertRaises(ReconnectExhausted):
            await self.manager([client], max_retries=0).run()
        self.assertEqual(client.stopped, [BIKE])
        self.assertEqual(client.disconnects, 1)

    async def test_retry_budget(self):
        clients = [FakeClient(connect_error=True) for _ in range(3)]
        with self.assertRaises(ReconnectExhausted):
            await self.manager(clients, max_retries=2).run()
        self.assertEqual(len(self.created), 3)
        self.assertTrue(all(c.disconnects == 1 for c in clients))

    async def test_stop_during_connect(self):
        client = FakeClient(hang_connect=True)
        manager = self.manager([client])
        task = asyncio.create_task(manager.run())
        await client.entered.wait()
        manager.stop()
        manager.stop()
        await asyncio.wait_for(task, .5)
        self.assertEqual(client.disconnects, 1)

    async def test_stop_during_retry_wait(self):
        client = FakeClient(connect_error=True)
        manager = self.manager([client], retry_delay=30)
        original = manager.on_state
        manager.on_state = lambda state: (original(state), manager.stop() if state == 'RETRY_WAIT' else None)
        await asyncio.wait_for(manager.run(), .5)
        self.assertEqual(len(self.created), 1)

    async def test_cleanup_failure_does_not_skip_other_resources(self):
        client = FakeClient(stop_error=True)
        manager = self.manager([client])
        task = asyncio.create_task(manager.run())
        await self.ready()
        manager.stop()
        with self.assertLogs('trainer_connection', level='WARNING'):
            await asyncio.wait_for(task, .5)
        self.assertEqual(client.stopped, [POWER, BIKE])
        self.assertEqual(client.disconnects, 1)

    async def test_cleanup_timeout_is_bounded(self):
        client = FakeClient(hang_stop=True)
        manager = self.manager([client])
        task = asyncio.create_task(manager.run())
        await self.ready()
        manager.stop()
        with self.assertLogs('trainer_connection', level='WARNING'):
            await asyncio.wait_for(task, .5)
        self.assertEqual(client.disconnects, 1)

    async def test_external_cancellation_cleans_up(self):
        client = FakeClient()
        manager = self.manager([client])
        task = asyncio.create_task(manager.run())
        await self.ready()
        task.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await task
        self.assertEqual(client.disconnects, 1)
        self.assertEqual(self.states[-1], 'STOPPED')

    async def test_stop_during_subscription(self):
        client = FakeClient(hang_subscribe=True)
        manager = self.manager([client])
        task = asyncio.create_task(manager.run())
        await client.subscribing.wait()
        manager.stop()
        await asyncio.wait_for(task, .5)
        self.assertEqual(client.disconnects, 1)

    async def test_backoff_and_cap_without_real_sleep(self):
        manager = self.manager([FakeClient(connect_error=True) for _ in range(5)],
                               max_retries=4, retry_delay=1, max_delay=3)
        delays = []

        async def sleep(delay):
            delays.append(delay)

        manager.sleep = sleep
        with self.assertRaises(ReconnectExhausted):
            await manager.run()
        self.assertEqual(delays, [1, 2, 3, 3])

    async def test_stable_connection_resets_budget(self):
        first, stable, last = FakeClient(connect_error=True), FakeClient(), FakeClient()
        manager = self.manager([first, stable, last], max_retries=1)
        now = [0]
        manager.clock = lambda: now[0]
        task = asyncio.create_task(manager.run())
        await self.ready()
        now[0] = 31
        stable.drop()
        await self.ready()
        manager.stop()
        await asyncio.wait_for(task, .5)
        self.assertEqual(len(self.created), 3)

    async def test_rapid_disconnect_does_not_reset_budget(self):
        first, second = FakeClient(), FakeClient()
        manager = self.manager([first, second], max_retries=1)
        manager.clock = lambda: 0
        task = asyncio.create_task(manager.run())
        await self.ready()
        first.drop()
        await self.ready()
        second.drop()
        with self.assertRaises(ReconnectExhausted):
            await asyncio.wait_for(task, .5)

    async def test_failed_disconnect_prevents_overlapping_reconnect(self):
        client = FakeClient()
        client.disconnect_error = True
        manager = self.manager([client])
        task = asyncio.create_task(manager.run())
        await self.ready()
        with self.assertLogs('trainer_connection', level='WARNING'):
            client.drop()
            with self.assertRaises(CleanupFailed):
                await asyncio.wait_for(task, .5)
        self.assertEqual(len(self.created), 1)

    async def test_connect_timeout(self):
        client = FakeClient(hang_connect=True)
        with self.assertRaises(ReconnectExhausted):
            await self.manager([client], timeout=.01, max_retries=0).run()
        self.assertEqual(client.disconnects, 1)


class ParserTests(unittest.TestCase):
    def test_speed_cadence_signed_power(self):
        value = parse_indoor_bike_data(bytes.fromhex('4400 c409 b400 f6ff'))
        self.assertEqual((value.speed_kmh, value.cadence_rpm, value.power_w), (25, 90, -10))

    def test_absent_values_and_inverted_speed_flag(self):
        value = parse_indoor_bike_data(b'\x01\x00')
        self.assertIsNone(value.speed_kmh)
        self.assertIsNone(value.power_w)

    def test_truncated_packets(self):
        for packet in (b'', b'\x00', b'\x00\x00', b'\x11\x00\x01\x02'):
            with self.subTest(packet=packet), self.assertRaises(ValueError):
                parse_indoor_bike_data(packet)
        with self.assertRaises(ValueError):
            parse_cycling_power(b'\x00\x00\x01')

    def test_cycling_power_signed(self):
        self.assertEqual(parse_cycling_power(b'\x00\x00\xf6\xff'), -10)

    def test_settings_validation(self):
        for kwargs in ({'timeout': 0}, {'max_retries': -1}, {'max_retries': 1.2}, {'retry_delay': float('nan')}):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                ConnectionSettings(**kwargs)


if __name__ == '__main__':
    unittest.main()
