-- The first-run (OOBE) screens this install has been through, by stable id.
-- A screen whose id is missing is shown at the next start on its own.
ALTER TABLE settings ADD COLUMN oobe_completed_steps TEXT NOT NULL DEFAULT '[]';
