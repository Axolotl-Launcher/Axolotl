CREATE TABLE crash_analysis_mod_snapshots (
	instance_id TEXT NOT NULL,
	filename TEXT NOT NULL,
	size INTEGER NOT NULL,
	sha256 TEXT NOT NULL,
	project_id TEXT,
	project_title TEXT,
	icon_url TEXT,
	version_id TEXT,
	version_number TEXT,
	PRIMARY KEY (instance_id, filename)
);

CREATE TABLE discovered_javas (
	path TEXT NOT NULL PRIMARY KEY,
	major_version INTEGER NOT NULL,
	full_version TEXT NOT NULL,
	architecture TEXT NOT NULL,
	file_size INTEGER NOT NULL,
	file_mtime_ms INTEGER NOT NULL,
	distribution TEXT
);

CREATE INDEX discovered_javas_major_version
	ON discovered_javas (major_version);

CREATE TABLE google_translate_ip_cache (
	ip TEXT NOT NULL PRIMARY KEY,
	latency_ms INTEGER NOT NULL,
	created_at INTEGER NOT NULL
);

CREATE TABLE translation_cache (
	key TEXT NOT NULL PRIMARY KEY,
	translation TEXT NOT NULL,
	created_at INTEGER NOT NULL
);

CREATE INDEX translation_cache_created_at
	ON translation_cache (created_at);
