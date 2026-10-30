"""Hardware-free process ownership and readiness tests for the local launcher."""
import importlib.util
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('run_local', Path(__file__).resolve().parents[1] / 'run_local.py')
launcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launcher)


class LauncherTests(unittest.TestCase):
    def child(self, code):
        child = subprocess.Popen([sys.executable, '-u', '-c', code], start_new_session=True,
                                 stdout=subprocess.PIPE, text=True)
        self.addCleanup(child.stdout.close)
        self.addCleanup(launcher.stop_child, child, 1)
        self.assertEqual(child.stdout.readline().strip(), 'ready')
        return child

    def test_shutdown_allows_owned_child_to_finish_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            marker = Path(tmp) / 'finished'
            code = ('import signal,time;from pathlib import Path\n'
                    f'def finish(*_):\n Path({str(marker)!r}).write_text("finished");raise SystemExit(0)\n'
                    'signal.signal(signal.SIGTERM,finish)\nprint("ready",flush=True)\n'
                    'while True: time.sleep(.05)')
            child = self.child(code)
            launcher.stop_child(child, 2)
            self.assertEqual(child.returncode, 0)
            self.assertEqual(marker.read_text(), 'finished')

    def test_stuck_child_is_killed_after_shutdown_budget(self):
        child = self.child('import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);print("ready",flush=True);time.sleep(60)')
        launcher.stop_child(child, .05)
        self.assertEqual(child.returncode, -signal.SIGKILL)

    def test_readiness_exited_process_never_opens_a_request(self):
        child = self.child('print("ready",flush=True)')
        child.wait(timeout=2)
        with patch.object(launcher.urllib.request, 'build_opener') as opener:
            with self.assertRaises(launcher.LaunchError):
                launcher.wait_ready(child, 'http://127.0.0.1:1234', 'unused-test-value', threading.Event())
            opener.return_value.open.assert_not_called()

    def test_plain_launch_does_not_enable_trainer(self):
        # Fail at the setup boundary: the default path must not reach BLE import/spawn.
        with tempfile.TemporaryDirectory() as tmp, patch.object(launcher, 'ROOT', Path(tmp)), patch.object(launcher.subprocess, 'Popen', side_effect=OSError('fixture')) as spawn:
            self.assertEqual(launcher.main(['--offline']), 1)
            self.assertEqual(spawn.call_count, 1)
            self.assertIn('local_db.py', spawn.call_args.args[0][1])


if __name__ == '__main__':
    unittest.main()
