CREATE TABLE resource_operation_plans (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision > 0),
    payload TEXT NOT NULL,
    job_id TEXT UNIQUE REFERENCES install_jobs(id),
    queued INTEGER NOT NULL DEFAULT 0 CHECK (queued IN (0, 1)),
    completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE resource_installed_artifacts (
    entry_id TEXT PRIMARY KEY NOT NULL REFERENCES instance_content_entries(id) ON DELETE CASCADE,
    artifact TEXT NOT NULL,
    scope TEXT NOT NULL
);
