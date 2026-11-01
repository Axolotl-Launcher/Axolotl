//! The packs the launcher keeps a single copy of and shares with instances.
//!
//! A pack is stored once per content hash, so a second upload of the same file
//! under another name reuses the row already holding that hash.

use sqlx::SqliteConnection;

use super::{Atom, Collection, Handle};

/// The catalog of shared packs.
pub(crate) struct SyncedPackCatalog;

/// A pack is identified by its own id.
pub(crate) type SyncedPackKey = String;

/// What a pack carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncedPackRecord {
    pub project_type: String,
    pub file_name: String,
    pub sha1: String,
    pub size: i64,
    pub game_versions_json: String,
    pub enabled: bool,
    pub selected: bool,
    pub selection_order: Option<i64>,
    pub created_at: i64,
    pub modified_at: i64,
}

impl Collection for SyncedPackCatalog {
    type Key = SyncedPackKey;
    type Scope = SyncedPackKey;
    type Record = SyncedPackRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<PackRow> = sqlx::query_as(
            "
            SELECT id, project_type, file_name, sha1, size, game_versions_json,
                enabled, selected, selection_order, created_at, modified_at
            FROM synced_pack_catalog
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(PackRow::into_record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM synced_pack_catalog WHERE id = ?")
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
        list(conn, None).await
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM synced_pack_catalog")
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

impl SyncedPackCatalog {
    /// Every catalogued pack of `project_type`, most recently changed first.
    pub(crate) async fn list_of_project_type(
        handle: &Handle,
        project_type: &str,
    ) -> crate::Result<Vec<(SyncedPackKey, SyncedPackRecord)>> {
        let mut connection = handle.connection().await?;
        list(&mut connection, Some(project_type)).await
    }

    /// The pack stored under `sha1`, whichever id it was first catalogued as.
    pub(crate) async fn get_by_sha1(
        handle: &Handle,
        sha1: &str,
    ) -> crate::Result<Option<(SyncedPackKey, SyncedPackRecord)>> {
        let mut connection = handle.connection().await?;
        let row: Option<PackRow> = sqlx::query_as(
            "
            SELECT id, project_type, file_name, sha1, size, game_versions_json,
                enabled, selected, selection_order, created_at, modified_at
            FROM synced_pack_catalog
            WHERE sha1 = ?
            ",
        )
        .bind(sha1)
        .fetch_optional(&mut *connection)
        .await?;

        Ok(row.map(|row| (row.id.clone(), row.into_record())))
    }

    /// Catalogues a pack that was just copied into the cache, keeping the row
    /// already holding its hash and only refreshing when it was last seen.
    pub(crate) async fn record_upload(
        handle: &Handle,
        key: &str,
        record: &SyncedPackRecord,
    ) -> crate::Result<(SyncedPackKey, SyncedPackRecord)> {
        let mut connection = handle.connection().await?;
        let row: PackRow = sqlx::query_as(
            "
            INSERT INTO synced_pack_catalog
                (id, project_type, file_name, sha1, size, game_versions_json,
                 enabled, created_at, modified_at)
            VALUES (?, ?, ?, ?, ?, '[]', 1, ?, ?)
            ON CONFLICT(sha1) DO UPDATE SET
                modified_at = excluded.modified_at
            RETURNING id, project_type, file_name, sha1, size,
                game_versions_json, enabled, selected, selection_order,
                created_at, modified_at
            ",
        )
        .bind(key)
        .bind(&record.project_type)
        .bind(&record.file_name)
        .bind(&record.sha1)
        .bind(record.size)
        .bind(record.created_at)
        .bind(record.modified_at)
        .fetch_one(&mut *connection)
        .await?;

        Ok((row.id.clone(), row.into_record()))
    }

    /// Catalogues an uploaded pack, replacing the metadata of a row that
    /// already holds its hash.
    pub(crate) async fn record_shared_upload(
        handle: &Handle,
        key: &str,
        record: &SyncedPackRecord,
    ) -> crate::Result<()> {
        let mut connection = handle.connection().await?;
        sqlx::query(
            "
            INSERT INTO synced_pack_catalog
                (id, project_type, file_name, sha1, size, game_versions_json,
                 enabled, created_at, modified_at)
            VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)
            ON CONFLICT(sha1) DO UPDATE SET
                file_name = excluded.file_name,
                game_versions_json = excluded.game_versions_json,
                modified_at = excluded.modified_at
            ",
        )
        .bind(key)
        .bind(&record.project_type)
        .bind(&record.file_name)
        .bind(&record.sha1)
        .bind(record.size)
        .bind(&record.game_versions_json)
        .bind(record.created_at)
        .bind(record.modified_at)
        .execute(&mut *connection)
        .await?;

        Ok(())
    }

    /// Catalogues a pack found in an instance directory, dropping the rows of
    /// the same name that hold older bytes first.
    pub(crate) async fn replace_seeded_in(
        atom: &mut Atom,
        rows_to_delete: &[SyncedPackKey],
        key: &str,
        record: &SyncedPackRecord,
    ) -> crate::Result<()> {
        for id in rows_to_delete {
            sqlx::query("DELETE FROM synced_pack_catalog WHERE id = ?")
                .bind(id)
                .execute(atom.connection())
                .await?;
        }
        sqlx::query(
            "
            INSERT INTO synced_pack_catalog
                (id, project_type, file_name, sha1, size, game_versions_json,
                 enabled, created_at, modified_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(sha1) DO UPDATE SET
                file_name = excluded.file_name,
                game_versions_json = excluded.game_versions_json,
                enabled = excluded.enabled,
                modified_at = excluded.modified_at
            ",
        )
        .bind(key)
        .bind(&record.project_type)
        .bind(&record.file_name)
        .bind(&record.sha1)
        .bind(record.size)
        .bind(&record.game_versions_json)
        .bind(record.enabled)
        .bind(record.created_at)
        .bind(record.modified_at)
        .execute(atom.connection())
        .await?;

        Ok(())
    }

    /// Turns a catalogued pack on or off, leaving its selection alone.
    pub(crate) async fn set_enabled(
        handle: &Handle,
        id: &str,
        enabled: bool,
        modified_at: i64,
    ) -> crate::Result<()> {
        let mut connection = handle.connection().await?;
        sqlx::query(
            "UPDATE synced_pack_catalog SET enabled = ?, modified_at = ? WHERE id = ?",
        )
        .bind(i64::from(enabled))
        .bind(modified_at)
        .bind(id)
        .execute(&mut *connection)
        .await?;

        Ok(())
    }

    /// Records where a pack sits in the resource pack selection.
    pub(crate) async fn set_selection_in(
        atom: &mut Atom,
        id: &str,
        selected: bool,
        selection_order: Option<i64>,
    ) -> crate::Result<()> {
        sqlx::query(
            "
            UPDATE synced_pack_catalog
            SET selected = ?, selection_order = ?
            WHERE id = ?
            ",
        )
        .bind(i64::from(selected))
        .bind(selection_order)
        .bind(id)
        .execute(atom.connection())
        .await?;

        Ok(())
    }

    /// Every catalogued pack of `project_type` that `instance_id` has already
    /// materialized, with the path it was put at.
    pub(crate) async fn materialized_for_instance(
        handle: &Handle,
        project_type: &str,
        instance_id: &str,
    ) -> crate::Result<Vec<(SyncedPackKey, SyncedPackRecord, String)>> {
        let mut connection = handle.connection().await?;
        let rows: Vec<MaterializedPackRow> = sqlx::query_as(
            "
            SELECT c.id, c.project_type, c.file_name, c.sha1, c.size,
                c.game_versions_json, c.enabled, c.selected, c.selection_order,
                c.created_at, c.modified_at, i.materialized_path
            FROM synced_pack_catalog c
            JOIN synced_pack_instances i ON i.pack_id = c.id
            WHERE c.project_type = ? AND i.instance_id = ?
              AND i.materialized_path IS NOT NULL
            ",
        )
        .bind(project_type)
        .bind(instance_id)
        .fetch_all(&mut *connection)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let MaterializedPackRow {
                    pack,
                    materialized_path,
                } = row;
                (pack.id.clone(), pack.into_record(), materialized_path)
            })
            .collect())
    }
}

/// One catalog row, as every statement above reads it.
#[derive(Debug, sqlx::FromRow)]
struct PackRow {
    id: String,
    project_type: String,
    file_name: String,
    sha1: String,
    size: i64,
    game_versions_json: String,
    enabled: bool,
    selected: bool,
    selection_order: Option<i64>,
    created_at: i64,
    modified_at: i64,
}

/// A catalog row with the path one instance materialized it at.
#[derive(Debug, sqlx::FromRow)]
struct MaterializedPackRow {
    #[sqlx(flatten)]
    pack: PackRow,
    materialized_path: String,
}

impl PackRow {
    fn into_record(self) -> SyncedPackRecord {
        SyncedPackRecord {
            project_type: self.project_type,
            file_name: self.file_name,
            sha1: self.sha1,
            size: self.size,
            game_versions_json: self.game_versions_json,
            enabled: self.enabled,
            selected: self.selected,
            selection_order: self.selection_order,
            created_at: self.created_at,
            modified_at: self.modified_at,
        }
    }
}

async fn list(
    conn: &mut SqliteConnection,
    project_type: Option<&str>,
) -> crate::Result<Vec<(SyncedPackKey, SyncedPackRecord)>> {
    let rows: Vec<PackRow> = match project_type {
        Some(project_type) => {
            sqlx::query_as(
                "
                SELECT id, project_type, file_name, sha1, size,
                    game_versions_json, enabled, selected, selection_order,
                    created_at, modified_at
                FROM synced_pack_catalog
                WHERE project_type = ?
                ORDER BY modified_at DESC
                ",
            )
            .bind(project_type)
            .fetch_all(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_as(
                "
                SELECT id, project_type, file_name, sha1, size,
                    game_versions_json, enabled, selected, selection_order,
                    created_at, modified_at
                FROM synced_pack_catalog
                ORDER BY modified_at DESC
                ",
            )
            .fetch_all(&mut *conn)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|row| (row.id.clone(), row.into_record()))
        .collect())
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &SyncedPackKey,
    record: &SyncedPackRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO synced_pack_catalog
            (id, project_type, file_name, sha1, size, game_versions_json,
             enabled, selected, selection_order, created_at, modified_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            project_type = excluded.project_type,
            file_name = excluded.file_name,
            sha1 = excluded.sha1,
            size = excluded.size,
            game_versions_json = excluded.game_versions_json,
            enabled = excluded.enabled,
            selected = excluded.selected,
            selection_order = excluded.selection_order,
            modified_at = excluded.modified_at
        ",
    )
    .bind(key)
    .bind(&record.project_type)
    .bind(&record.file_name)
    .bind(&record.sha1)
    .bind(record.size)
    .bind(&record.game_versions_json)
    .bind(record.enabled)
    .bind(record.selected)
    .bind(record.selection_order)
    .bind(record.created_at)
    .bind(record.modified_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
