-- Threads: add temp JSONB, populate, swap
ALTER TABLE threads ADD COLUMN created_by_jsonb JSONB;
UPDATE threads SET created_by_jsonb = (
    CASE 
        WHEN left(trim(created_by),1) = '{' THEN created_by::jsonb
        ELSE jsonb_build_object(
            'v',1,
            'provider','discord',
            'discord_id', CASE WHEN position(':' in created_by) > 0 THEN split_part(created_by,':',1) ELSE created_by END,
            'username', CASE WHEN position(':' in created_by) > 0 AND split_part(created_by,':',2) <> '' THEN split_part(created_by,':',2) ELSE created_by END,
            'display',  CASE WHEN position(':' in created_by) > 0 AND split_part(created_by,':',2) <> '' THEN split_part(created_by,':',2) ELSE created_by END,
            'legacy_source', created_by
        )
    END
);
ALTER TABLE threads DROP COLUMN created_by;
ALTER TABLE threads RENAME COLUMN created_by_jsonb TO created_by;
ALTER TABLE threads ALTER COLUMN created_by SET DEFAULT jsonb_build_object('v',1,'provider','unknown','username','legacy','display','legacy');

-- Replies
ALTER TABLE replies ADD COLUMN created_by_jsonb JSONB;
UPDATE replies SET created_by_jsonb = (
    CASE 
        WHEN left(trim(created_by),1) = '{' THEN created_by::jsonb
        ELSE jsonb_build_object(
            'v',1,
            'provider','discord',
            'discord_id', CASE WHEN position(':' in created_by) > 0 THEN split_part(created_by,':',1) ELSE created_by END,
            'username', CASE WHEN position(':' in created_by) > 0 AND split_part(created_by,':',2) <> '' THEN split_part(created_by,':',2) ELSE created_by END,
            'display',  CASE WHEN position(':' in created_by) > 0 AND split_part(created_by,':',2) <> '' THEN split_part(created_by,':',2) ELSE created_by END,
            'legacy_source', created_by
        )
    END
);
ALTER TABLE replies DROP COLUMN created_by;
ALTER TABLE replies RENAME COLUMN created_by_jsonb TO created_by;
ALTER TABLE replies ALTER COLUMN created_by SET DEFAULT jsonb_build_object('v',1,'provider','unknown','username','legacy','display','legacy');

-- (Optional) future indexes:
-- CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_threads_created_by_discord_id ON threads ((created_by->>'discord_id'));
-- CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_replies_created_by_discord_id ON replies ((created_by->>'discord_id'));
