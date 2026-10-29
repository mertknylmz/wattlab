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

Close Zwift and disconnect other trainer clients before running. `live_trainer.py` currently prints raw FTMS Indoor Bike Data packets; parsing power/cadence/speed is the next step.
