import os
from pathlib import Path
from dotenv import load_dotenv

load_dotenv(Path(__file__).resolve().parents[2] / ".env")

def trainer_address() -> str:
    address = os.getenv("TRAINER_ADDRESS", "").strip()
    if not address or address in {"YOUR_BLUETOOTH_ADDRESS", "YOUR_TRAINER_BLUETOOTH_ADDRESS"}:
        raise RuntimeError("Set TRAINER_ADDRESS in the repository root .env file")
    return address
