CREATE TABLE telemetry_event (
    session_id UUID NOT NULL REFERENCES training_session(id),
    stream_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    connection_id UUID NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('ftms', 'cycling_power')),
    speed_kmh DOUBLE PRECISION CHECK (speed_kmh >= 0 AND speed_kmh <= 655.35),
    cadence_rpm DOUBLE PRECISION CHECK (cadence_rpm >= 0 AND cadence_rpm <= 32767.5),
    power_w INTEGER CHECK (power_w BETWEEN -32768 AND 32767),
    PRIMARY KEY (session_id, stream_id, sequence),
    CHECK (speed_kmh IS NOT NULL OR cadence_rpm IS NOT NULL OR power_w IS NOT NULL),
    CHECK (source <> 'cycling_power' OR (speed_kmh IS NULL AND cadence_rpm IS NULL))
);
CREATE TABLE connection_event (
    session_id UUID NOT NULL REFERENCES training_session(id),
    stream_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    connection_id UUID NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('connecting','subscribing','streaming','retry_wait','stopped')),
    PRIMARY KEY (session_id, stream_id, sequence)
);
