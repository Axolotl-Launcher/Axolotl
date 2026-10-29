CREATE TABLE cache (
	id TEXT NOT NULL,
	data_type TEXT NOT NULL,
	alias TEXT NULL,

	data JSONB NULL,
	expires INTEGER NOT NULL,

	UNIQUE (data_type, alias),
	PRIMARY KEY (id, data_type)
);
