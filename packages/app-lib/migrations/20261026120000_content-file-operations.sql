CREATE TABLE content_file_operations (
    instance_id TEXT PRIMARY KEY REFERENCES instances(id) ON DELETE RESTRICT,
    payload TEXT NOT NULL,
    committed INTEGER NOT NULL DEFAULT 0 CHECK (committed IN (0, 1))
);
