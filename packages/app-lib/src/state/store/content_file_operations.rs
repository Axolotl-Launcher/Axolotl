//! Durable filesystem operation journals used during crash recovery.

use super::{Collection, Handle};
use sqlx::SqliteConnection;

pub(crate) struct ContentFileOperations;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileOperationRecord {
    pub payload: String,
    pub committed: bool,
}

impl Collection for ContentFileOperations {
    type Key = String;
    type Scope = String;
    type Record = FileOperationRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }
    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, bool)> = sqlx::query_as("SELECT payload, committed FROM content_file_operations WHERE instance_id = ?")
            .bind(key).fetch_optional(&mut *conn).await?;
        Ok(row.map(|(payload, committed)| FileOperationRecord {
            payload,
            committed,
        }))
    }
    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        match record {
            Some(record) => {
                sqlx::query("INSERT INTO content_file_operations(instance_id, payload, committed) VALUES (?, ?, ?) ON CONFLICT(instance_id) DO UPDATE SET payload = excluded.payload, committed = excluded.committed")
                .bind(key).bind(&record.payload).bind(record.committed).execute(&mut *conn).await?;
            }
            None => {
                sqlx::query(
                    "DELETE FROM content_file_operations WHERE instance_id = ?",
                )
                .bind(key)
                .execute(&mut *conn)
                .await?;
            }
        }
        Ok(())
    }
    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, bool)> = sqlx::query_as("SELECT instance_id, payload, committed FROM content_file_operations").fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .map(|(key, payload, committed)| {
                (key, FileOperationRecord { payload, committed })
            })
            .collect())
    }
    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM content_file_operations")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
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

pub(crate) async fn get(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Option<(String, bool)>> {
    Ok(ContentFileOperations::get(handle, &instance_id.to_owned())
        .await?
        .map(|record| (record.payload, record.committed)))
}

pub(crate) async fn set(
    handle: &Handle,
    instance_id: &str,
    payload: &str,
) -> crate::Result<()> {
    let mut unit = super::atom(handle).await?;
    let key = instance_id.to_owned();
    if ContentFileOperations::atom_get(&mut unit, &key)
        .await?
        .is_some()
    {
        return Err(crate::ErrorKind::InputError(
            "Content operation recovery must finish before another operation"
                .into(),
        )
        .into());
    }
    ContentFileOperations::atom_set(
        &mut unit,
        &key,
        Some(&FileOperationRecord {
            payload: payload.into(),
            committed: false,
        }),
    )
    .await?;
    unit.commit().await
}

pub(crate) async fn remove(
    handle: &Handle,
    instance_id: &str,
    payload: &str,
) -> crate::Result<()> {
    let mut unit = super::atom(handle).await?;
    let key = instance_id.to_owned();
    if ContentFileOperations::atom_get(&mut unit, &key)
        .await?
        .is_some_and(|record| record.payload == payload)
    {
        ContentFileOperations::atom_set(&mut unit, &key, None).await?;
    }
    unit.commit().await
}

pub(crate) async fn get_instance_ids(
    handle: &Handle,
) -> crate::Result<Vec<String>> {
    Ok(ContentFileOperations::get_all(handle)
        .await?
        .into_iter()
        .map(|(key, _)| key)
        .collect())
}

pub(crate) async fn mark_committed(
    instance_id: &str,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<()> {
    let result = sqlx::query("UPDATE content_file_operations SET committed = 1 WHERE instance_id = ?")
        .bind(instance_id).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(crate::ErrorKind::InputError(
            "Content operation journal disappeared before commit".into(),
        )
        .into());
    }
    Ok(())
}
