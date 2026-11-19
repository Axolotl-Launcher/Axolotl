//! The schema of the resource cache database.
//!
//! The migrations are written here rather than read from a directory: these
//! versions are already applied in existing caches, and restating them as files
//! would change the checksums their applied records carry, so every one of them
//! would stop opening.

use sqlx::migrate::{Migration, MigrationType, Migrator};
use std::borrow::Cow;

pub(crate) const CACHE_SCHEMA_VERSION: u32 = 3;

pub(crate) fn migrator() -> Migrator {
    Migrator {
        migrations: Cow::Owned(vec![
            Migration::new(
                20261011090000,
                "independent resource cache".into(),
                MigrationType::Simple,
                sqlx::SqlStr::from_static(
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
            "#,
                ),
                false,
            ),
            Migration::new(
                20261011121000,
                "legacy resource import checkpoint".into(),
                MigrationType::Simple,
                sqlx::SqlStr::from_static(
                    r#"
            CREATE TABLE resource_legacy_import (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_rowid INTEGER NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1))
            );
            INSERT INTO resource_legacy_import(id) VALUES (1);
            "#,
                ),
                false,
            ),
            Migration::new(
                20261118090000,
                "resource lookup and instance content cache".into(),
                MigrationType::Simple,
                sqlx::SqlStr::from_static(
                    r#"
            CREATE TABLE cache (
                id TEXT NOT NULL,
                data_type TEXT NOT NULL,
                alias TEXT,
                data JSONB,
                expires INTEGER NOT NULL,
                UNIQUE(data_type, alias),
                PRIMARY KEY(id, data_type)
            );
            CREATE TABLE content_refresh_packs (
                instance_id TEXT NOT NULL,
                content_set_id TEXT NOT NULL,
                payload TEXT NOT NULL,
                PRIMARY KEY(instance_id, content_set_id)
            );
            CREATE TABLE content_refresh_scan_failures (
                instance_id TEXT NOT NULL,
                location_key TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                payload TEXT NOT NULL,
                PRIMARY KEY(instance_id, location_key, relative_path)
            );
            CREATE TABLE content_evidence (
                kind TEXT NOT NULL,
                instance_id TEXT NOT NULL,
                record_id TEXT NOT NULL,
                guard TEXT NOT NULL,
                payload TEXT NOT NULL,
                PRIMARY KEY(kind, instance_id, record_id)
            );
            CREATE INDEX content_evidence_instance ON content_evidence(instance_id);
            "#,
                ),
                false,
            ),
        ]),
        ..Migrator::DEFAULT
    }
}

pub(crate) async fn clear_projections(
    unit: &mut super::Atom,
) -> crate::Result<()> {
    sqlx::raw_sql(
        "DELETE FROM cache;
         DELETE FROM content_refresh_packs;
         DELETE FROM content_refresh_scan_failures;
         DELETE FROM content_evidence;",
    )
    .execute(unit.connection())
    .await?;
    Ok(())
}
