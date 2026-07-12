ALTER TABLE images DROP CONSTRAINT IF EXISTS images_hash_key;

ALTER TABLE images
    ADD CONSTRAINT images_exactly_one_owner
    CHECK (num_nonnulls(thread_id, reply_id) = 1) NOT VALID;

ALTER TABLE images
    ADD CONSTRAINT images_valid_hash
    CHECK (hash ~ '^[0-9a-f]{64}$') NOT VALID;

ALTER TABLE images VALIDATE CONSTRAINT images_exactly_one_owner;
ALTER TABLE images VALIDATE CONSTRAINT images_valid_hash;

CREATE UNIQUE INDEX IF NOT EXISTS idx_images_one_per_thread
    ON images(thread_id)
    WHERE thread_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_images_one_per_reply
    ON images(reply_id)
    WHERE reply_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_images_hash ON images(hash);