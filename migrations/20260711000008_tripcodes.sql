ALTER TABLE threads ADD COLUMN author_name TEXT;
ALTER TABLE threads ADD COLUMN tripcode TEXT;
ALTER TABLE replies ADD COLUMN author_name TEXT;
ALTER TABLE replies ADD COLUMN tripcode TEXT;

ALTER TABLE threads
    ADD CONSTRAINT threads_author_name_length
    CHECK (author_name IS NULL OR char_length(author_name) BETWEEN 1 AND 40);
ALTER TABLE replies
    ADD CONSTRAINT replies_author_name_length
    CHECK (author_name IS NULL OR char_length(author_name) BETWEEN 1 AND 40);

ALTER TABLE threads
    ADD CONSTRAINT threads_tripcode_format
    CHECK (tripcode IS NULL OR tripcode ~ '^![0-9a-f]{12}$');
ALTER TABLE replies
    ADD CONSTRAINT replies_tripcode_format
    CHECK (tripcode IS NULL OR tripcode ~ '^![0-9a-f]{12}$');