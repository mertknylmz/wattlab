-- Baseline for the memberdb database; connections set search_path to the member schema.
CREATE TABLE training_session (
    id UUID PRIMARY KEY,
    request_id UUID NOT NULL UNIQUE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    finished_at TIMESTAMPTZ,
    -- FTP captured at session start keeps historical zones stable after the setting changes.
    ftp_w INTEGER CHECK (ftp_w BETWEEN 1 AND 2000),
    CHECK (finished_at IS NULL OR finished_at >= started_at)
);
CREATE INDEX session_latest ON training_session(started_at DESC, id DESC);

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
    received_at TIMESTAMPTZ DEFAULT clock_timestamp(),
    PRIMARY KEY (session_id, stream_id, sequence),
    CHECK (speed_kmh IS NOT NULL OR cadence_rpm IS NOT NULL OR power_w IS NOT NULL),
    CHECK (source <> 'cycling_power' OR (speed_kmh IS NULL AND cadence_rpm IS NULL))
);
CREATE INDEX telemetry_live_source ON telemetry_event(session_id, stream_id, connection_id, source, sequence DESC);

CREATE TABLE connection_event (
    session_id UUID NOT NULL REFERENCES training_session(id),
    stream_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    connection_id UUID NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('connecting','subscribing','streaming','retry_wait','stopped')),
    received_at TIMESTAMPTZ DEFAULT clock_timestamp(),
    PRIMARY KEY (session_id, stream_id, sequence)
);
CREATE INDEX connection_live_receipt ON connection_event(session_id, received_at DESC);

-- The 200 W default is a display reference, not a personal measurement.
CREATE TABLE athlete_settings (
    id SMALLINT PRIMARY KEY CHECK (id = 1),
    ftp_w INTEGER NOT NULL CHECK (ftp_w BETWEEN 1 AND 2000),
    ftp_is_default BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
INSERT INTO athlete_settings (id, ftp_w, ftp_is_default) VALUES (1, 200, true);
