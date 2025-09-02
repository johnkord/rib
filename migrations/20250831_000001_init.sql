-- Schema initialization for rib
CREATE TABLE IF NOT EXISTS boards (
    id BIGSERIAL PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS threads (
    id BIGSERIAL PRIMARY KEY,
    board_id BIGINT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,                              -- NEW
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    bump_time TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_threads_board_bump ON threads(board_id, bump_time DESC);

CREATE TABLE IF NOT EXISTS replies (
    id BIGSERIAL PRIMARY KEY,
    thread_id BIGINT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    image_hash TEXT,      -- new
    mime TEXT,            -- new
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS images (
    id BIGSERIAL PRIMARY KEY,
    thread_id BIGINT REFERENCES threads(id) ON DELETE CASCADE,
    reply_id BIGINT REFERENCES replies(id) ON DELETE CASCADE,
    hash TEXT NOT NULL UNIQUE,
    mime TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS reports (
    id BIGSERIAL PRIMARY KEY,
    target_id BIGINT NOT NULL,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed data
INSERT INTO boards (slug, title)
VALUES ('general', 'General Discussion')
ON CONFLICT (slug) DO NOTHING;