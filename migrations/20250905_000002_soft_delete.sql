-- Soft & Hard Deletion migration
-- Adds deleted_at timestamps and supporting partial indexes

ALTER TABLE boards  ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
ALTER TABLE threads ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
ALTER TABLE replies ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_boards_not_deleted ON boards(id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_threads_board_active ON threads(board_id, bump_time DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_replies_thread_active ON replies(thread_id, created_at ASC) WHERE deleted_at IS NULL;
