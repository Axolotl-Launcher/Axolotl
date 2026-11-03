CREATE UNIQUE INDEX instance_files_instance_identity
    ON instance_files(instance_id, id);
CREATE UNIQUE INDEX instance_content_entries_instance_identity
    ON instance_content_entries(instance_id, id);

CREATE TABLE content_file_fingerprints (
    instance_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (instance_id, file_id),
    FOREIGN KEY (instance_id, file_id)
        REFERENCES instance_files(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE content_refresh_checks (
    instance_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (instance_id, entry_id),
    FOREIGN KEY (instance_id, entry_id)
        REFERENCES instance_content_entries(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, file_id)
        REFERENCES instance_files(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX content_refresh_checks_file
    ON content_refresh_checks(instance_id, file_id);
