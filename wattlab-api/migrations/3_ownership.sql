-- Every session and setting belongs to a member. Data recorded before accounts
-- existed has no owner and is discarded by user decision (30 October 2026).
DELETE FROM connection_event;
DELETE FROM telemetry_event;
DELETE FROM training_session;
ALTER TABLE training_session ADD COLUMN member_id UUID NOT NULL REFERENCES member(id);
CREATE INDEX session_member_latest ON training_session(member_id, started_at DESC, id DESC);
CREATE INDEX session_active ON training_session(started_at DESC) WHERE finished_at IS NULL;

-- A missing row means the 200 W default display reference.
DROP TABLE athlete_settings;
CREATE TABLE athlete_settings (
    member_id UUID PRIMARY KEY REFERENCES member(id) ON DELETE CASCADE,
    ftp_w INTEGER NOT NULL CHECK (ftp_w BETWEEN 1 AND 2000),
    ftp_is_default BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
