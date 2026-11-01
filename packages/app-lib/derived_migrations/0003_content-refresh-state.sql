CREATE TABLE content_refresh_packs (
	instance_id TEXT NOT NULL,
	content_set_id TEXT NOT NULL,
	payload TEXT NOT NULL,
	PRIMARY KEY(instance_id, content_set_id)
);

CREATE TABLE content_refresh_scan_failures (
	instance_id TEXT NOT NULL,
	location_key TEXT NOT NULL,
	relative_path TEXT NOT NULL,
	payload TEXT NOT NULL,
	PRIMARY KEY(instance_id, location_key, relative_path)
);
