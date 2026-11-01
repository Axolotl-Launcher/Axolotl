//! The exact artifact each content entry installed.
//!
//! An entry whose installed file still matches the payload here is known to be
//! that artifact; one whose file changed has no exact artifact any more.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The installed artifacts, one per content entry.
pub(crate) struct ResourceInstalledArtifacts;

/// An artifact is identified by the content entry that installs it.
pub(crate) type InstalledArtifactKey = String;

/// What an installed entry carries.
///
/// The artifact and the install scope are kept as the text they are stored as:
/// what the text says is the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceInstalledArtifactRecord {
    pub artifact: String,
    pub scope: String,
}

/// One installed entry: its id, the file the artifact was verified against,
/// and the artifact payload. The local metadata and the payload are absent
/// when nothing recorded them.
pub(crate) type InstalledFileRow =
    (String, String, i64, Option<String>, Option<String>);

/// One installed entry of the applied content set, with its pack member and
/// the artifact payload when it has one.
pub(crate) type InstalledRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    i64,
    i64,
    String,
    i64,
    Option<String>,
    Option<String>,
);

impl Collection for ResourceInstalledArtifacts {
    type Key = InstalledArtifactKey;
    type Scope = InstalledArtifactKey;
    type Record = ResourceInstalledArtifactRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, String)> = sqlx::query_as(
            "
            SELECT artifact, scope
            FROM resource_installed_artifacts
            WHERE entry_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(artifact, scope)| Self::Record { artifact, scope }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM resource_installed_artifacts WHERE entry_id = ?",
            )
            .bind(key)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "
            SELECT entry_id, artifact, scope
            FROM resource_installed_artifacts
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(entry_id, artifact, scope)| {
                (entry_id, Self::Record { artifact, scope })
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_installed_artifacts")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(scope.clone(), record)])
            .unwrap_or_default())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}

impl ResourceInstalledArtifacts {
    /// Every entry of the instance that is not missing, with the file state
    /// its artifact has to agree with.
    pub(crate) async fn with_file_state(
        handle: &Handle,
        instance_id: &str,
    ) -> crate::Result<Vec<InstalledFileRow>> {
        let rows: Vec<InstalledFileRow> = sqlx::query_as(
            "
            SELECT entry.id, file.sha1, file.size, file.local_mod_data,
                exact.artifact
            FROM instance_content_entries entry
            JOIN instance_files file ON file.id = entry.file_id
            LEFT JOIN resource_installed_artifacts exact
                ON exact.entry_id = entry.id
            WHERE entry.instance_id = ? AND file.missing = 0
            ",
        )
        .bind(instance_id)
        .fetch_all(handle.pool())
        .await?;

        Ok(rows)
    }

    /// The artifact each entry of the instance installed, by entry id.
    pub(crate) async fn payloads_for_instance(
        handle: &Handle,
        instance_id: &str,
    ) -> crate::Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "
            SELECT exact.entry_id, exact.artifact
            FROM resource_installed_artifacts exact
            JOIN instance_content_entries entry ON entry.id = exact.entry_id
            WHERE entry.instance_id = ?
            ",
        )
        .bind(instance_id)
        .fetch_all(handle.pool())
        .await?;

        Ok(rows)
    }

    /// The entries of the instance's applied content set, with the file each
    /// one has and the artifact it installed.
    pub(crate) async fn installed_rows(
        handle: &Handle,
        instance_id: &str,
    ) -> crate::Result<Vec<InstalledRow>> {
        let rows: Vec<InstalledRow> = sqlx::query_as(
            "
            SELECT entry.id, entry.project_type, entry.ownership_kind,
                (SELECT member.id FROM instance_pack_members member
                 WHERE member.content_entry_id = entry.id
                 LIMIT 1) AS member_id,
                file.relative_path, file.enabled, file.missing,
                file.sha1, file.size, exact.artifact, exact.scope
            FROM instance_content_entries entry
            JOIN instance_content_sets content
                ON content.id = entry.content_set_id
            JOIN instance_files file ON file.id = entry.file_id
            LEFT JOIN resource_installed_artifacts exact
                ON exact.entry_id = entry.id
            WHERE entry.instance_id = ?
                AND content.id = (
                    SELECT applied_content_set_id
                    FROM instances WHERE id = entry.instance_id)
            ",
        )
        .bind(instance_id)
        .fetch_all(handle.pool())
        .await?;

        Ok(rows)
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &InstalledArtifactKey,
    record: &ResourceInstalledArtifactRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_installed_artifacts (entry_id, artifact, scope)
        VALUES (?, ?, ?)
        ON CONFLICT(entry_id) DO UPDATE SET
            artifact = excluded.artifact,
            scope = excluded.scope
        ",
    )
    .bind(key)
    .bind(&record.artifact)
    .bind(&record.scope)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
