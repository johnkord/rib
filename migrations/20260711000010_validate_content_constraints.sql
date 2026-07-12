SET LOCAL lock_timeout = '15s';
SET LOCAL statement_timeout = '5min';

ALTER TABLE boards VALIDATE CONSTRAINT boards_slug_format;
ALTER TABLE boards VALIDATE CONSTRAINT boards_title_length;
ALTER TABLE threads VALIDATE CONSTRAINT threads_subject_length;
ALTER TABLE threads VALIDATE CONSTRAINT threads_body_length;
ALTER TABLE replies VALIDATE CONSTRAINT replies_content_length;