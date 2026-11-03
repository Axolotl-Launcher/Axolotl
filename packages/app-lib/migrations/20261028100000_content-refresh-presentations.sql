CREATE TABLE content_refresh_presentations (
    instance_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (instance_id, file_id),
    FOREIGN KEY (instance_id, file_id)
        REFERENCES instance_files(instance_id, id) ON DELETE CASCADE
);
