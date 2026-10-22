CREATE TABLE resource_history (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    entry_id TEXT NOT NULL,
    recorded_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    backup_relative_path TEXT,
    backup_size INTEGER,
    backup_sha1 TEXT,
    backup_available INTEGER NOT NULL DEFAULT 0 CHECK (backup_available IN (0, 1))
);
CREATE INDEX resource_history_entry ON resource_history(instance_id, entry_id, recorded_at DESC);

CREATE TABLE resource_history_cleanup_plans (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    payload TEXT NOT NULL,
    consumed INTEGER NOT NULL DEFAULT 0 CHECK (consumed IN (0, 1))
);
