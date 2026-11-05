ALTER TABLE checkpoint_writes RENAME TO checkpoint_writes_with_parent;
CREATE TABLE checkpoint_writes (
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    namespace TEXT NOT NULL,
    checkpoint_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    write_index INTEGER NOT NULL,
    channel TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY(thread_id, namespace, checkpoint_id, task_id, write_index)
);
INSERT INTO checkpoint_writes SELECT * FROM checkpoint_writes_with_parent;
DROP TABLE checkpoint_writes_with_parent;
