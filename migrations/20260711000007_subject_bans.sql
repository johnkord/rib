CREATE TABLE subject_bans (
    subject TEXT PRIMARY KEY,
    reason TEXT NOT NULL,
    banned_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_subject_bans_active
    ON subject_bans(subject, expires_at);