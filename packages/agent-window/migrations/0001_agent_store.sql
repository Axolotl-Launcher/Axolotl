CREATE TABLE threads (
    id TEXT PRIMARY KEY NOT NULL,
    payload TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE attempts (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    turn_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'awaiting', 'completed', 'failed', 'cancelled', 'interrupted')),
    computer_use INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE UNIQUE INDEX one_active_attempt ON attempts(thread_id) WHERE state IN ('queued', 'running', 'awaiting');
CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    attempt_id TEXT REFERENCES attempts(id) ON DELETE CASCADE,
    payload TEXT NOT NULL
);
CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    UNIQUE(attempt_id, event_id)
);
CREATE INDEX events_attempt ON events(attempt_id, sequence);
CREATE TABLE checkpoints (
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    namespace TEXT NOT NULL,
    checkpoint_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    UNIQUE(thread_id, namespace, checkpoint_id)
);
CREATE TABLE approvals (
    id TEXT PRIMARY KEY NOT NULL,
    attempt_id TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    call_id TEXT NOT NULL,
    tool TEXT NOT NULL,
    arguments TEXT NOT NULL,
    revision INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'rejected', 'consumed', 'expired')),
    UNIQUE(attempt_id, call_id)
);
CREATE TABLE executions (
    attempt_id TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    call_id TEXT NOT NULL,
    tool TEXT NOT NULL,
    arguments TEXT NOT NULL,
    is_write INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('started', 'completed', 'failed', 'unknown')),
    result TEXT,
    PRIMARY KEY(attempt_id, call_id)
);
CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    payload TEXT NOT NULL
);
CREATE TABLE imports (id TEXT PRIMARY KEY NOT NULL);
