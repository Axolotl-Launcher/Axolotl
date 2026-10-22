use sqlx::migrate::{Migration, MigrationType, Migrator};
use std::borrow::Cow;

pub const CACHE_SCHEMA_VERSION: u32 = 2;

pub fn migrator() -> Migrator {
    Migrator {
        migrations: Cow::Owned(vec![Migration::new(
            20261011090000,
            "independent resource cache".into(),
            MigrationType::Simple,
            r#"
            CREATE TABLE resource_cache_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                generation INTEGER NOT NULL
            );
            INSERT INTO resource_cache_state (id, generation) VALUES (1, 0);
            CREATE TABLE resource_cache_entries (
                namespace TEXT NOT NULL,
                source TEXT NOT NULL,
                query_key TEXT NOT NULL,
                value_version INTEGER NOT NULL,
                payload TEXT NOT NULL,
                retrieved_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                PRIMARY KEY (namespace, source, query_key)
            );
            CREATE INDEX resource_cache_expiry ON resource_cache_entries(expires_at);
            CREATE TABLE resource_artifact_evidence (
                evidence_key TEXT PRIMARY KEY,
                value_version INTEGER NOT NULL,
                payload TEXT NOT NULL,
                verified_at INTEGER NOT NULL
            );
            "#.into(),
            false,
        ), Migration::new(
            20261011121000,
            "legacy resource import checkpoint".into(),
            MigrationType::Simple,
            r#"
            CREATE TABLE resource_legacy_import (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_rowid INTEGER NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1))
            );
            INSERT INTO resource_legacy_import(id) VALUES (1);
            "#.into(),
            false,
        )]),
        ..Migrator::DEFAULT
    }
}
