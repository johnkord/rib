-- Add created_by tracking (renamed to compliant version format)
ALTER TABLE threads ADD COLUMN IF NOT EXISTS created_by TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE replies ADD COLUMN IF NOT EXISTS created_by TEXT NOT NULL DEFAULT 'legacy';

UPDATE threads SET created_by = 'legacy' WHERE created_by = 'legacy';
UPDATE replies SET created_by = 'legacy' WHERE created_by = 'legacy';