# WattLab Bluetooth experiments

Ubuntu + Van Rysel D500 BLE exploration. Bluetooth addresses are local configuration, not committed.

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
cp .env.example .env
# Edit .env with your trainer's Bluetooth address
python experiments/bluetooth/scan_trainer.py
python experiments/bluetooth/live_trainer.py
```

Close Zwift and disconnect other trainer clients before running. `scan_trainer.py` connects to the configured address and lists GATT services; it does not scan nearby devices.

`live_trainer.py` prints parsed speed (km/h), cadence (RPM), and power (W). It uses whichever notifying FTMS Indoor Bike Data or Cycling Power characteristics are available; at least one is required. Missing measurements remain `None`. If a selected subscription fails, the whole connection attempt is cleaned up and retried rather than silently dropping that source.

The Python connection manager detects disconnects and permits up to eight retries, waiting 1, 2, 4, 8, 16, then at most 30 seconds between attempts. The budget resets after a connection has remained in streaming state for 30 seconds; this does not imply that fresh telemetry arrived. No automatic reconnect is triggered solely by missing packets. Connection/subscription operations have a 20-second timeout. These settings can be changed through `ConnectionSettings` in Python; no new environment variables are required.

Ctrl+C or SIGTERM requests shutdown during connection, streaming, or retry waits. Successfully opened subscriptions are cleaned up independently, then disconnect is attempted, with a three-second timeout per cleanup operation. Cleanup failures are logged without device addresses; an unsuccessful disconnect disables automatic reconnect to avoid overlapping connections. Cleanup is best effort if the OS or adapter fails. No trainer control commands are sent.

## Hardware-free tests

The lifecycle and parser tests use only Python's standard library and fake clients:

```bash
python3 -m unittest discover -s experiments/bluetooth/tests -v
```

These tests cover disconnect/reconnect, stale callbacks, capabilities, partial subscriptions, retry limits/backoff, timeouts, cancellation, and resource cleanup. Real Ubuntu/BlueZ and D500 behavior requires a separate device test; these tests do not establish hardware compatibility.
