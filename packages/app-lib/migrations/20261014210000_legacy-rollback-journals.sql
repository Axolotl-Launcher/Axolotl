CREATE TABLE resource_legacy_rollback_journals (
    instance_id TEXT PRIMARY KEY REFERENCES instances(id) ON DELETE CASCADE,
    payload TEXT NOT NULL
);
