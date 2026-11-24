ALTER TABLE queues
    ADD COLUMN activity_at BIGINT UNSIGNED NOT NULL DEFAULT 0 AFTER created_at,
    ADD KEY ix_queues_activity_at (activity_at);

UPDATE queues SET activity_at = created_at;
