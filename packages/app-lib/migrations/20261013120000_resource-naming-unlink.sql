ALTER TABLE settings ADD COLUMN resource_chinese_filenames INTEGER NOT NULL DEFAULT 1 CHECK (resource_chinese_filenames IN (0, 1));
ALTER TABLE settings ADD COLUMN resource_preserve_filenames INTEGER NOT NULL DEFAULT 1 CHECK (resource_preserve_filenames IN (0, 1));

CREATE TABLE resource_pack_unlinks (
    instance_id TEXT PRIMARY KEY NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    content_set_id TEXT NOT NULL REFERENCES instance_content_sets(id) ON DELETE CASCADE,
    unlinked_at INTEGER NOT NULL
);
