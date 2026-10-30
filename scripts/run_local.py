"""Headless developer launcher: local DB + API, optionally the trainer agent (--trainer).

The screens run only in the desktop app (wattlab-desktop); this is for scripts and tests.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request

from dotenv import dotenv_values

ROOT = Path(__file__).resolve().parents[1]


class LaunchError(RuntimeError):
    pass


def stop_child(child, timeout=30):
    """Signal only our process group; never find/kill unrelated API processes."""
    if child.poll() is not None:
        return
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        child.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(timeout=5)


def wait_ready(child, origin, token, stopping, timeout=15):
    """Require both public liveness and the configured protected API to be ready."""
    deadline = time.monotonic() + timeout
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    while time.monotonic() < deadline and not stopping.is_set():
        if child.poll() is not None:
            raise LaunchError('API exited before it was ready; inspect its startup output.')
        try:
            request = urllib.request.Request(origin + '/api/v1/status',
                                             headers={'Authorization': 'Bearer ' + token})
            with opener.open(request, timeout=1) as response:
                value = json.load(response)
            if isinstance(value, dict) and value.get('service') == 'wattlab-api' and value.get('status') == 'ok':
                return
        except urllib.error.HTTPError as error:
            if error.code == 401:
                raise LaunchError('Local API authentication failed; check local configuration.') from None
        except (OSError, ValueError):
            pass
        stopping.wait(.2)
    raise LaunchError('API readiness timed out or startup was interrupted.')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--trainer', action='store_true',
                        help='Start the trainer agent; it connects and controls only on screen commands. Close other trainer apps first')
    parser.add_argument('--offline', action='store_true', help='Build using cached Cargo dependencies')
    options = parser.parse_args(argv)
    stopping = threading.Event()
    children = []
    previous = {}
    for sig in (signal.SIGINT, signal.SIGTERM):
        previous[sig] = signal.signal(sig, lambda *_: stopping.set())
    lock = None
    try:
        (ROOT / '.wattlab').mkdir(exist_ok=True)
        lock = (ROOT / '.wattlab' / 'run-local.lock').open('w')
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise LaunchError('Another WattLab local launcher is running.') from None

        def spawn(command, env=None):
            if stopping.is_set():
                raise LaunchError('Startup interrupted.')
            child = subprocess.Popen(command, cwd=ROOT, env=env, start_new_session=True)
            children.append(child)
            return child

        def run_step(command):
            child = spawn(command)
            while child.poll() is None and not stopping.wait(.1):
                pass
            if stopping.is_set():
                raise LaunchError('Startup interrupted.')
            if child.returncode:
                raise LaunchError('A setup step failed; inspect the preceding output.')

        # The existing helper owns database creation and preserves .env secrets.
        run_step([sys.executable, str(ROOT / 'scripts/local_db.py'), 'start'])
        env = dict(os.environ)
        for key, value in dotenv_values(ROOT / '.env').items():
            if value is not None:
                env.setdefault(key, value)
        try:
            port = int(env.get('WATTLAB_API_PORT', '8080'))
            if not 1 <= port <= 65535:
                raise ValueError
        except ValueError:
            raise LaunchError('WATTLAB_API_PORT must be an integer between 1 and 65535.') from None
        origin = f'http://127.0.0.1:{port}'
        # Refuse to take over an IDE/user-owned API. No unrelated process is stopped.
        try:
            with socket.socket() as probe:
                probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                probe.bind(('127.0.0.1', port))
        except OSError:
            raise LaunchError('The API port is unavailable; stop your existing API or choose another port.') from None
        sys.path.insert(0, str(ROOT / 'experiments/bluetooth'))
        from telemetry_sender import TelemetrySender
        TelemetrySender(origin, env.get('WATTLAB_AGENT_TOKEN', ''))  # Validation only; no network/task.
        cargo = shutil.which('cargo') or str(Path.home() / '.cargo/bin/cargo')
        if not Path(cargo).is_file():
            raise LaunchError('Cargo is missing; install the Rust toolchain first.')
        command = [cargo, 'build', '--locked', '--manifest-path', str(ROOT / 'wattlab-api/Cargo.toml')]
        if options.offline:
            command.append('--offline')
        run_step(command)
        env['WATTLAB_API_URL'] = origin
        env['WATTLAB_ENV_FILE'] = str(ROOT / '.env')
        target = Path(env.get('CARGO_TARGET_DIR', str(ROOT / 'wattlab-api/target')))
        if not target.is_absolute():
            target = ROOT / target
        api = spawn([str(target / 'debug/wattlab-api')], env)
        wait_ready(api, origin, env['WATTLAB_AGENT_TOKEN'], stopping)
        if options.trainer:
            spawn([sys.executable, '-u', str(ROOT / 'experiments/bluetooth/trainer_agent.py')], env)
        print('API: ' + origin + '/api/v1 (Bearer token from local .env; not printed here).', flush=True)
        print('Trainer agent started; it connects only on a command.' if options.trainer else 'Trainer agent disabled; use --trainer to enable it.', flush=True)
        print('Screens run in the desktop app (wattlab-desktop). Ctrl+C stops owned processes; PostgreSQL stays available.', flush=True)
        while not stopping.wait(.3):
            if any(child.poll() is not None for child in children[-(2 if options.trainer else 1):]):
                raise LaunchError('A running component exited; shutting down the other owned processes.')
        return 0
    except (LaunchError, OSError, ValueError) as error:
        # Unexpected OS messages may include local credentials/addresses: classify only.
        message = str(error) if isinstance(error, LaunchError) else 'Local startup failed; check installed tools and configuration.'
        print(message, file=sys.stderr)
        return 0 if stopping.is_set() else 1
    finally:
        # Reverse order lets the sender drain/finish while the API is still alive.
        for child in reversed(children):
            stop_child(child)
        if lock is not None:
            lock.close()
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == '__main__':
    raise SystemExit(main())
