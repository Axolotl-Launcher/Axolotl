-- Scheduling tags that outlive a restart, and the restart budget of the work
-- they label. A row is data, never authority: the planned supervisor restart
-- path will re-mint a fresh tag for every owner that still has work, use the
-- stored id only as the reconciliation key, and derive the level again instead
-- of reading `level`. Until that path lands, startup clears tags whose owner is
-- gone or finished.
CREATE TABLE scheduler_tags (
    id INTEGER PRIMARY KEY,
    generation INTEGER NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('l0', 'l1', 'l2', 'l3', 'l4')),
    origin TEXT NOT NULL,
    owner TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX scheduler_tags_owner ON scheduler_tags (owner);

-- Failed restarts or rebuilds per scope since its last success. The counters
-- reset only when the work is live again.
CREATE TABLE scheduler_restart_budget (
    scope TEXT PRIMARY KEY NOT NULL,
    failures INTEGER NOT NULL DEFAULT 0 CHECK (failures >= 0),
    first_failure_at INTEGER,
    revoked INTEGER NOT NULL DEFAULT 0 CHECK (revoked IN (0, 1)),
    updated_at INTEGER NOT NULL
);
