CREATE TABLE queue_subqueues (
    id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
    queue_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
    title VARCHAR(120) NOT NULL,
    position BIGINT UNSIGNED NOT NULL,
    is_open BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (id),
    UNIQUE KEY uq_queue_subqueues_position (queue_id, position),
    UNIQUE KEY uq_queue_subqueues_title (queue_id, title),
    UNIQUE KEY uq_queue_subqueues_id_queue (id, queue_id),
    CONSTRAINT fk_queue_subqueues_queue FOREIGN KEY (queue_id) REFERENCES queues(id)
        ON UPDATE CASCADE ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO queue_subqueues (id, queue_id, title, position, is_open)
SELECT UUID(), id, 'Общая очередь', 0, is_open FROM queues;

ALTER TABLE queue_entries
    ADD COLUMN subqueue_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NULL AFTER queue_id;

UPDATE queue_entries e
JOIN queue_subqueues s ON s.queue_id = e.queue_id
SET e.subqueue_id = s.id;

ALTER TABLE queue_entries
    MODIFY subqueue_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
    DROP INDEX uq_queue_entries_name,
    DROP INDEX uq_queue_entries_user,
    DROP INDEX uq_queue_entries_visitor,
    ADD UNIQUE KEY uq_queue_entries_subqueue_name (subqueue_id, name),
    ADD UNIQUE KEY uq_queue_entries_subqueue_user (subqueue_id, user_id),
    ADD UNIQUE KEY uq_queue_entries_subqueue_visitor (subqueue_id, visitor_id),
    ADD KEY ix_queue_entries_subqueue_position (subqueue_id, position),
    ADD CONSTRAINT fk_queue_entries_subqueue FOREIGN KEY (subqueue_id, queue_id)
        REFERENCES queue_subqueues(id, queue_id)
        ON UPDATE CASCADE ON DELETE CASCADE;
