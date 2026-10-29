"""Version-one wire events, independent of BLE and HTTP."""
from datetime import datetime, timezone
from uuid import uuid4


class TelemetryEvents:
    def __init__(self):
        self.stream_id = str(uuid4())
        self.connection_id = str(uuid4())
        self.sequence = 0

    def _envelope(self):
        event = {
            "schema_version": 1,
            "stream_id": self.stream_id,
            "connection_id": self.connection_id,
            "sequence": self.sequence,
            "observed_at": datetime.now(timezone.utc).isoformat(timespec="microseconds"),
        }
        self.sequence += 1
        return event

    def state(self, state):
        if state == "CONNECTING":
            self.connection_id = str(uuid4())
        return {**self._envelope(), "state": state.lower()}

    def bike(self, value):
        # An FTMS packet may contain none of the three metrics we currently export.
        if all(metric is None for metric in (value.speed_kmh, value.cadence_rpm, value.power_w)):
            return None
        return {**self._envelope(), "source": "ftms", "speed_kmh": value.speed_kmh,
                "cadence_rpm": value.cadence_rpm, "power_w": value.power_w}

    def power(self, watts):
        return {**self._envelope(), "source": "cycling_power", "speed_kmh": None,
                "cadence_rpm": None, "power_w": watts}
