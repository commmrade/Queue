ALTER TABLE queue_entries
    DROP FOREIGN KEY fk_queue_entries_user,
    DROP FOREIGN KEY fk_queue_entries_queue;

ALTER TABLE queues
    DROP FOREIGN KEY fk_queues_owner;

ALTER TABLE accounts
    MODIFY id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL;

ALTER TABLE queues
    MODIFY id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
    MODIFY owner_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL;

ALTER TABLE queue_entries
    MODIFY id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
    MODIFY queue_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
    MODIFY user_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NULL,
    MODIFY leave_token_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL;

ALTER TABLE revoked_tokens
    MODIFY jti CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL;

ALTER TABLE queues
    ADD CONSTRAINT fk_queues_owner FOREIGN KEY (owner_id) REFERENCES accounts(id)
        ON UPDATE CASCADE ON DELETE RESTRICT;

ALTER TABLE queue_entries
    ADD CONSTRAINT fk_queue_entries_queue FOREIGN KEY (queue_id) REFERENCES queues(id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    ADD CONSTRAINT fk_queue_entries_user FOREIGN KEY (user_id) REFERENCES accounts(id)
        ON UPDATE CASCADE ON DELETE SET NULL;
