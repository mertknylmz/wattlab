-- Existing events deliberately remain NULL: historical observations must not
-- become fresh just because the live view was installed.
ALTER TABLE telemetry_event ADD COLUMN received_at TIMESTAMPTZ;
ALTER TABLE telemetry_event ALTER COLUMN received_at SET DEFAULT clock_timestamp();
ALTER TABLE connection_event ADD COLUMN received_at TIMESTAMPTZ;
ALTER TABLE connection_event ALTER COLUMN received_at SET DEFAULT clock_timestamp();
CREATE INDEX telemetry_live_source ON telemetry_event(session_id,stream_id,connection_id,source,sequence DESC);
CREATE INDEX connection_live_receipt ON connection_event(session_id,received_at DESC);
CREATE INDEX session_latest ON training_session(started_at DESC,id DESC);
