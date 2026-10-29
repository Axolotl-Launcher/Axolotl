-- Saved pack metadata and failed scans move to the derived database, where a
-- rebuildable record belongs. A downgrade removes this migration's record
-- without restoring the tables, so its replay must not fail on tables a
-- previous run already dropped.
DROP TABLE IF EXISTS content_refresh_packs;
DROP TABLE IF EXISTS content_refresh_scan_failures;
