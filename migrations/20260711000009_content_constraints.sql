ALTER TABLE boards
    ADD CONSTRAINT boards_slug_format
    CHECK (slug ~ '^[a-z0-9_-]{1,64}$') NOT VALID;
ALTER TABLE boards
    ADD CONSTRAINT boards_title_length
    CHECK (char_length(title) BETWEEN 1 AND 100) NOT VALID;

ALTER TABLE threads
    ADD CONSTRAINT threads_subject_length
    CHECK (char_length(subject) BETWEEN 1 AND 200) NOT VALID;
ALTER TABLE threads
    ADD CONSTRAINT threads_body_length
    CHECK (char_length(body) <= 2000) NOT VALID;

ALTER TABLE replies
    ADD CONSTRAINT replies_content_length
    CHECK (char_length(content) <= 2000) NOT VALID;