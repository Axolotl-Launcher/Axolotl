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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        Ok(Self::get_all(handle)
            .await?
            .into_iter()
            .filter(|(_, record)| record.project_type == project_type)
            .collect())
    }

    /// The pack stored under `sha1`, whichever id it was first catalogued as.
    pub(crate) async fn get_by_sha1(
        handle: &Handle,
        sha1: &str,
    ) -> crate::Result<Option<(SyncedPackKey, SyncedPackRecord)>> {
        Ok(Self::get_all(handle)
            .await?
            .into_iter()
            .find(|(_, record)| record.sha1 == sha1))
    }

    /// Catalogues a pack that was just copied into the cache, keeping the row
    /// already holding its hash and only refreshing when it was last seen.
    pub(crate) async fn record_upload(
        handle: &Handle,
        key: &str,
        record: &SyncedPackRecord,
    ) -> crate::Result<(SyncedPackKey, SyncedPackRecord)> {
        let mut unit = super::atom(handle).await?;
        let existing = Self::atom_get_all(&mut unit)
            .await?
            .into_iter()
            .find(|(_, saved)| saved.sha1 == record.sha1);
        let (id, saved) = if let Some((id, mut saved)) = existing {
            saved.modified_at = record.modified_at;
            (id, saved)
        } else {
            let mut saved = record.clone();
            saved.game_versions_json = "[]".into();
            saved.enabled = true;
            (key.to_owned(), saved)
        };
        Self::atom_set(&mut unit, &id, Some(&saved)).await?;
        unit.commit().await?;
        Ok((id, saved))
    }

    /// Catalogues an uploaded pack, replacing the metadata of a row that
    /// already holds its hash.
    pub(crate) async fn record_shared_upload(
        handle: &Handle,
        key: &str,
        record: &SyncedPackRecord,
    ) -> crate::Result<()> {
        let mut unit = super::atom(handle).await?;
        let existing = Self::atom_get_all(&mut unit)
            .await?
            .into_iter()
            .find(|(_, saved)| saved.sha1 == record.sha1);
        let (id, mut saved) =
            existing.unwrap_or_else(|| (key.to_owned(), record.clone()));
        saved.file_name = record.file_name.clone();
        saved.game_versions_json = record.game_versions_json.clone();
        saved.modified_at = record.modified_at;
        Self::atom_set(&mut unit, &id, Some(&saved)).await?;
        unit.commit().await
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
            Self::atom_set(atom, id, None).await?;
        }
        let existing = Self::atom_get_all(atom)
            .await?
            .into_iter()
            .find(|(_, saved)| saved.sha1 == record.sha1);
        let (id, mut saved) =
            existing.unwrap_or_else(|| (key.to_owned(), record.clone()));
        saved.file_name = record.file_name.clone();
        saved.game_versions_json = record.game_versions_json.clone();
        saved.enabled = record.enabled;
        saved.modified_at = record.modified_at;
        Self::atom_set(atom, &id, Some(&saved)).await
    }

    /// Turns a catalogued pack on or off, leaving its selection alone.
    pub(crate) async fn set_enabled(
        handle: &Handle,
        id: &str,
        enabled: bool,
        modified_at: i64,
    ) -> crate::Result<()> {
        let mut unit = super::atom(handle).await?;
        if let Some(mut record) =
            Self::atom_get(&mut unit, &id.to_owned()).await?
        {
            record.enabled = enabled;
            record.modified_at = modified_at;
            Self::atom_set(&mut unit, &id.to_owned(), Some(&record)).await?;
        }
        unit.commit().await
    }

    /// Records where a pack sits in the resource pack selection.
    pub(crate) async fn set_selection_in(
        atom: &mut Atom,
        id: &str,
        selected: bool,
        selection_order: Option<i64>,
    ) -> crate::Result<()> {
        if let Some(mut record) = Self::atom_get(atom, &id.to_owned()).await? {
            record.selected = selected;
            record.selection_order = selection_order;
            Self::atom_set(atom, &id.to_owned(), Some(&record)).await?;
        }
        Ok(())
    }

    /// Every catalogued pack of `project_type` that `instance_id` has already
    /// materialized, with the path it was put at.
    pub(crate) async fn materialized_for_instance(
        handle: &Handle,
        project_type: &str,
        instance_id: &str,
    ) -> crate::Result<Vec<(SyncedPackKey, SyncedPackRecord, String)>> {
        let catalog = Self::list_of_project_type(handle, project_type).await?;
        let links =
            super::synced_pack_instances::SyncedPackInstances::get_all(handle)
                .await?;
        Ok(catalog
            .into_iter()
            .filter_map(|(id, record)| {
                let path = links
                    .iter()
                    .find(|(key, _)| {
                        key.pack_id == id && key.instance_id == instance_id
                    })?
                    .1
                    .materialized_path
                    .clone()?;
                Some((id, record, path))
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

impl crate::state::sync_state::collection::Collection for SyncedPackCatalog {
    type Key = SyncedPackKey;
    type Scope = SyncedPackKey;
    type Record = SyncedPackRecord;
    const NAME: &'static str = "synced_pack_catalog";
    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }
    fn ordered(rows: &mut Vec<(Self::Key, Self::Record)>) {
        rows.sort_by(|a, b| {
            b.1.modified_at
                .cmp(&a.1.modified_at)
                .then_with(|| a.0.cmp(&b.0))
        });
    }
}
crate::state::sync_state::collection::verbs!(
    SyncedPackCatalog,
    SyncedPackKey,
    SyncedPackKey,
    SyncedPackRecord
);
