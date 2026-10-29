CREATE TABLE content_refresh_packs (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    content_set_id TEXT NOT NULL REFERENCES instance_content_sets(id) ON DELETE CASCADE,
    payload TEXT NOT NULL,
    PRIMARY KEY(instance_id, content_set_id)
);
