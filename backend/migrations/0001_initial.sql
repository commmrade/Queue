CREATE TABLE accounts (
    id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    email VARCHAR(254) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role ENUM('user', 'admin') NOT NULL DEFAULT 'user',
    created_at BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_accounts_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE queues (
    id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    owner_id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    title VARCHAR(160) NOT NULL,
    course VARCHAR(120) NOT NULL DEFAULT '',
    teacher VARCHAR(120) NOT NULL DEFAULT '',
    is_open BOOLEAN NOT NULL DEFAULT TRUE,
    created_at BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (id),
    KEY ix_queues_created_at (created_at),
    KEY ix_queues_owner_id (owner_id),
    CONSTRAINT fk_queues_owner FOREIGN KEY (owner_id) REFERENCES accounts(id)
        ON UPDATE CASCADE ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE queue_entries (
    position BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    queue_id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    user_id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NULL,
    name VARCHAR(100) NOT NULL,
    leave_token_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    created_at BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_queue_entries_position (position),
    UNIQUE KEY uq_queue_entries_name (queue_id, name),
    UNIQUE KEY uq_queue_entries_user (queue_id, user_id),
    KEY ix_queue_entries_queue_created (queue_id, created_at),
    CONSTRAINT fk_queue_entries_queue FOREIGN KEY (queue_id) REFERENCES queues(id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_queue_entries_user FOREIGN KEY (user_id) REFERENCES accounts(id)
        ON UPDATE CASCADE ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE revoked_tokens (
    jti CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    expires_at BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (jti),
    KEY ix_revoked_tokens_expires_at (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
