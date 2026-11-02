DROP INDEX one_active_attempt;
CREATE UNIQUE INDEX one_active_attempt ON attempts(thread_id) WHERE state IN ('running', 'awaiting');
CREATE TABLE attempt_inputs (
    attempt_id TEXT PRIMARY KEY NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    payload TEXT NOT NULL
);
CREATE TABLE checkpoint_writes (
    thread_id TEXT NOT NULL,
    namespace TEXT NOT NULL,
    checkpoint_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    write_index INTEGER NOT NULL,
    channel TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY(thread_id, namespace, checkpoint_id, task_id, write_index),
    FOREIGN KEY(thread_id, namespace, checkpoint_id) REFERENCES checkpoints(thread_id, namespace, checkpoint_id) ON DELETE CASCADE
);
CREATE TABLE approval_batches (
    id TEXT PRIMARY KEY NOT NULL,
    attempt_id TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    actions TEXT NOT NULL,
    selected TEXT,
    state TEXT NOT NULL CHECK(state IN ('pending', 'decided', 'expired'))
);
CREATE TABLE workspace_state (
    id INTEGER PRIMARY KEY NOT NULL CHECK(id = 1),
    payload TEXT NOT NULL
);
