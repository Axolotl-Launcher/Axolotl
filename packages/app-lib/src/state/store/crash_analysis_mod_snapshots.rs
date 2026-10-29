//! The mods an instance had when a crash analysis last succeeded.
//!
//! A snapshot is taken so a later crash can say which mods changed since. It is
//! replaced as a whole whenever a new analysis succeeds.

use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;

use super::Collection;

/// The snapshots of every instance.
pub(crate) struct CrashAnalysisModSnapshots;

/// A snapshot entry is identified by the instance it belongs to and the mod
/// file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrashAnalysisModSnapshotKey {
    pub instance_id: String,
    pub filename: String,
}

/// Every entry of one instance.
pub(crate) type CrashAnalysisModSnapshotScope = String;

/// What an entry carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrashAnalysisModSnapshotRecord {
    pub size: u64,
    pub sha256: String,
    pub project_id: Option<String>,
    pub project_title: Option<String>,
    pub icon_url: Option<String>,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
}

impl Collection for CrashAnalysisModSnapshots {
    type Key = CrashAnalysisModSnapshotKey;
    type Scope = CrashAnalysisModSnapshotScope;
    type Record = CrashAnalysisModSnapshotRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        key.instance_id == *scope
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row = sqlx::query(
            "
            SELECT size, sha256, project_id, project_title, icon_url,
                version_id, version_number
            FROM crash_analysis_mod_snapshots
            WHERE instance_id = ? AND filename = ?
            ",
        )
        .bind(&key.instance_id)
        .bind(&key.filename)
        .fetch_optional(&mut *conn)
        .await?;

        match row {
            Some(row) => Ok(Some(to_record(&row)?)),
            None => Ok(None),
        }
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "
                DELETE FROM crash_analysis_mod_snapshots
                WHERE instance_id = ? AND filename = ?
                ",
            )
            .bind(&key.instance_id)
            .bind(&key.filename)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows = sqlx::query(
            "
            SELECT instance_id, filename, size, sha256, project_id,
                project_title, icon_url, version_id, version_number
            FROM crash_analysis_mod_snapshots
            ORDER BY instance_id, filename
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let key = Self::Key {
                instance_id: row.try_get("instance_id")?,
                filename: row.try_get("filename")?,
            };
            entries.push((key, to_record(row)?));
        }

        Ok(entries)
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM crash_analysis_mod_snapshots")
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
        let rows = sqlx::query(
            "
            SELECT filename, size, sha256, project_id, project_title, icon_url,
                version_id, version_number
            FROM crash_analysis_mod_snapshots
            WHERE instance_id = ?
            ",
        )
        .bind(scope)
        .fetch_all(&mut *conn)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let key = Self::Key {
                instance_id: scope.clone(),
                filename: row.try_get("filename")?,
            };
            entries.push((key, to_record(row)?));
        }

        Ok(entries)
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query(
            "DELETE FROM crash_analysis_mod_snapshots WHERE instance_id = ?",
        )
        .bind(scope)
        .execute(&mut *conn)
        .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

fn to_record(row: &SqliteRow) -> crate::Result<CrashAnalysisModSnapshotRecord> {
    Ok(CrashAnalysisModSnapshotRecord {
        size: row.try_get("size")?,
        sha256: row.try_get("sha256")?,
        project_id: row.try_get("project_id")?,
        project_title: row.try_get("project_title")?,
        icon_url: row.try_get("icon_url")?,
        version_id: row.try_get("version_id")?,
        version_number: row.try_get("version_number")?,
    })
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &CrashAnalysisModSnapshotKey,
    record: &CrashAnalysisModSnapshotRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO crash_analysis_mod_snapshots (
            instance_id, filename, size, sha256, project_id, project_title,
            icon_url, version_id, version_number
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (instance_id, filename) DO UPDATE SET
            size = excluded.size,
            sha256 = excluded.sha256,
            project_id = excluded.project_id,
            project_title = excluded.project_title,
            icon_url = excluded.icon_url,
            version_id = excluded.version_id,
            version_number = excluded.version_number
        ",
    )
    .bind(&key.instance_id)
    .bind(&key.filename)
    .bind(record.size as i64)
    .bind(&record.sha256)
    .bind(&record.project_id)
    .bind(&record.project_title)
    .bind(&record.icon_url)
    .bind(&record.version_id)
    .bind(&record.version_number)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
