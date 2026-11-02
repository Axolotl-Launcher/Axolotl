-- Scheduling level of an install job. Install jobs hold the instance locks, so
-- every existing and new row starts at L3; the scheduler re-derives the level
-- on restart and never trusts this column as authority.
ALTER TABLE install_jobs ADD COLUMN level TEXT NOT NULL DEFAULT 'l3'
    CHECK (level IN ('l0', 'l1', 'l2', 'l3', 'l4'));
