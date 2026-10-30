-- Additive changes preserve existing usage tables, rows, and legacy readers.
-- Only a committed transaction may acknowledge a batch as accepted.
ALTER TABLE accepted_batches ADD COLUMN ingestion_token TEXT;
ALTER TABLE ingestion_global_daily ADD COLUMN reserved_writes INTEGER NOT NULL DEFAULT 0;

-- Reserve conservatively for batches accepted before write accounting existed.
UPDATE ingestion_global_daily SET reserved_writes = accepted_batches * 16;

DROP TRIGGER daily_active_rollups;
CREATE TRIGGER daily_active_rollups
AFTER INSERT ON daily_active
BEGIN
    INSERT OR IGNORE INTO wau_seen (installation_hash)
    SELECT NEW.installation_hash WHERE NEW.day BETWEEN date('now', '-6 days') AND date('now');
    INSERT OR IGNORE INTO mau_seen (installation_hash)
    SELECT NEW.installation_hash WHERE NEW.day BETWEEN date('now', '-29 days') AND date('now');
    INSERT INTO daily_active_dims (dimension, day, label, install_count)
    VALUES ('version', NEW.day, NEW.app_version, 1)
    ON CONFLICT (dimension, day, label) DO UPDATE SET install_count = install_count + 1;
    INSERT INTO daily_active_dims (dimension, day, label, install_count)
    VALUES ('platform', NEW.day, NEW.platform, 1)
    ON CONFLICT (dimension, day, label) DO UPDATE SET install_count = install_count + 1;
    INSERT INTO daily_active_dims (dimension, day, label, install_count)
    VALUES ('arch', NEW.day, NEW.arch, 1)
    ON CONFLICT (dimension, day, label) DO UPDATE SET install_count = install_count + 1;
END;
