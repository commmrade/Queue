ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS username VARCHAR(32) NULL AFTER id;

UPDATE accounts
SET username = CONCAT('user_', REPLACE(id, '-', ''))
WHERE username IS NULL OR username = '';

ALTER TABLE accounts
    MODIFY username VARCHAR(32) NOT NULL,
    ADD UNIQUE INDEX IF NOT EXISTS uq_accounts_username (username),
    DROP COLUMN IF EXISTS email;
