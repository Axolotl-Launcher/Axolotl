CREATE TABLE content_refresh_scan_failures (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    location_key TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY(instance_id, location_key, relative_path)
);
