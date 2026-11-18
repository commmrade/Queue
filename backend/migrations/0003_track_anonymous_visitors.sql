ALTER TABLE queue_entries
    ADD COLUMN visitor_id CHAR(36) CHARACTER SET ascii COLLATE ascii_general_ci NULL AFTER user_id,
    ADD UNIQUE KEY uq_queue_entries_visitor (queue_id, visitor_id);
