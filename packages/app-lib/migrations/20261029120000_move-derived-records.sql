-- A downgrade removes this migration's record without restoring the tables,
-- so its replay must not fail on tables a previous run already dropped.
--
-- Four tables move to the derived database. telemetry_outbox stays here: this
-- database is migrated before that one is opened, so the queued events cannot
-- be carried over, and keeping the table where telemetry_identity is written
-- keeps both in one transaction.
DROP TABLE IF EXISTS crash_analysis_mod_snapshots;
DROP TABLE IF EXISTS discovered_javas;
DROP TABLE IF EXISTS google_translate_ip_cache;
DROP TABLE IF EXISTS translation_cache;
