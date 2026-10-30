-- Single local athlete until profiles are designed. The 200 W default is a
-- display reference, not a personal measurement.
CREATE TABLE athlete_settings (
    id SMALLINT PRIMARY KEY CHECK (id = 1),
    ftp_w INTEGER NOT NULL CHECK (ftp_w BETWEEN 1 AND 2000),
    ftp_is_default BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
INSERT INTO athlete_settings (id, ftp_w, ftp_is_default) VALUES (1, 200, true);

-- FTP captured at session start keeps historical zones stable after the
-- setting changes. Sessions recorded before this migration stay NULL.
ALTER TABLE training_session ADD COLUMN ftp_w INTEGER CHECK (ftp_w BETWEEN 1 AND 2000);
