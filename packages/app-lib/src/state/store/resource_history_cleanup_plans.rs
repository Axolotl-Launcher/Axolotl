//! The reviewed plans that remove old backups.

use sqlx::Connection as _;
use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The cleanup plans, one per reviewed removal.
pub(crate) struct ResourceHistoryCleanupPlans;

/// A cleanup plan is identified by its own id.
pub(crate) type CleanupPlanKey = String;

/// What a cleanup plan carries.
///
/// The plan and the progress made on it are kept as the text they are stored
/// as: what the text says is the caller's business.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CleanupPlanRecord {
    pub instance_id: String,
    pub revision: u32,
    pub payload: String,
    pub consumed: bool,
}

impl Collection for ResourceHistoryCleanupPlans {
    type Key = CleanupPlanKey;
    type Scope = CleanupPlanKey;
    type Record = CleanupPlanRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::read(
                conn,
                "resource_history_cleanup_plans",
                key,
            )
            .await;
        }

        let row: Option<(String, u32, String, bool)> = sqlx::query_as(
            "
            SELECT instance_id, revision, payload, consumed
            FROM resource_history_cleanup_plans
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(
            row.map(|(instance_id, revision, payload, consumed)| {
                Self::Record {
                    instance_id,
                    revision,
                    payload,
                    consumed,
                }
            }),
        )
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::write(
                conn,
                "resource_history_cleanup_plans",
                key,
                record,
            )
            .await;
        }

        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM resource_history_cleanup_plans WHERE id = ?",
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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::all(
                conn,
                "resource_history_cleanup_plans",
            )
            .await;
        }

        let rows: Vec<(String, String, u32, String, bool)> = sqlx::query_as(
            "
            SELECT id, instance_id, revision, payload, consumed
            FROM resource_history_cleanup_plans
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, instance_id, revision, payload, consumed)| {
                (
                    id,
                    Self::Record {
                        instance_id,
                        revision,
                        payload,
                        consumed,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            for (key, _) in
                super::recovery_documents::strict_all::<Self::Record>(
                    conn,
                    "resource_history_cleanup_plans",
                )
                .await?
            {
                if !records.iter().any(|(candidate, _)| candidate == &key) {
                    Self::write(conn, &key, None).await?;
                }
            }
            for (key, record) in records {
                Self::write(conn, key, Some(record)).await?;
            }
            return Ok(());
        }

        sqlx::query("DELETE FROM resource_history_cleanup_plans")
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
        if super::recovery_documents::active(conn).await? {
            return Ok(Self::read_all(conn)
                .await?
                .into_iter()
                .filter(|(key, record)| key == scope)
                .collect());
        }

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
        if super::recovery_documents::active(conn).await? {
            for (key, record) in records {
                if !(key == scope) {
                    return Err(crate::ErrorKind::InputError(
                        "Operation record outside replacement scope".into(),
                    )
                    .into());
                }
            }
            for (key, _) in Self::read(conn, scope)
                .await?
                .map(|record| vec![(scope.clone(), record)])
                .unwrap_or_default()
                .into_iter()
                .filter(|(key, record)| key == scope)
            {
                if !records.iter().any(|(candidate, _)| candidate == &key) {
                    Self::write(conn, &key, None).await?;
                }
            }
            for (key, record) in records {
                Self::write(conn, key, Some(record)).await?;
            }
            return Ok(());
        }

        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}

impl ResourceHistoryCleanupPlans {
    /// Every plan saved for the instance, consumed or not.
    pub(crate) async fn payloads_for_instance(
        handle: &Handle,
        instance_id: &str,
    ) -> crate::Result<Vec<String>> {
        if let Some(selected) = super::recovery_documents::snapshot_records(
            handle.pool(),
            "resource_history_cleanup_plans",
            None,
            Some(instance_id),
        )
        .await?
        {
            let mut payloads = Vec::new();
            for row in selected {
                match row.decode::<CleanupPlanRecord>().await {
                    Ok(record) if record.instance_id == instance_id => {
                        payloads.push(record.payload)
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(key=%row.key,%error,"Recovery record unavailable; original retained and other records continue")
                    }
                }
            }
            return Ok(payloads);
        }

        let rows: Vec<String> = sqlx::query_scalar(
            "
            SELECT payload
            FROM resource_history_cleanup_plans
            WHERE instance_id = ?
            ",
        )
        .bind(instance_id)
        .fetch_all(handle.pool())
        .await?;

        Ok(rows)
    }

    /// Saves a plan under its own id.
    pub(crate) async fn insert(
        handle: &Handle,
        key: &str,
        instance_id: &str,
        revision: u32,
        payload: &str,
    ) -> crate::Result<()> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    if Self::read(conn, &key.to_owned()).await?.is_some() {
                        return Err(crate::ErrorKind::InputError(
                            "Cleanup plan exists".into(),
                        )
                        .into());
                    }
                    return Self::write(
                        conn,
                        &key.to_owned(),
                        Some(&CleanupPlanRecord {
                            instance_id: instance_id.into(),
                            revision,
                            payload: payload.into(),
                            consumed: false,
                        }),
                    )
                    .await;
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        let mut connection = handle.connection().await?;
        sqlx::query(
            "
            INSERT INTO resource_history_cleanup_plans
                (id, instance_id, revision, payload)
            VALUES (?, ?, ?, ?)
            ",
        )
        .bind(key)
        .bind(instance_id)
        .bind(revision)
        .bind(payload)
        .execute(&mut *connection)
        .await?;

        Ok(())
    }

    /// The payload of the plan `key` at `revision`, while it is unused.
    pub(crate) async fn open_payload(
        handle: &Handle,
        key: &str,
        revision: u32,
    ) -> crate::Result<Option<String>> {
        if let Some(selected) = super::recovery_documents::snapshot_records(
            handle.pool(),
            "resource_history_cleanup_plans",
            Some(key),
            None,
        )
        .await?
        {
            return match selected.into_iter().next() {
                Some(row) => Ok(Some(row.decode::<CleanupPlanRecord>().await?)
                    .filter(|row| row.revision == revision && !row.consumed)
                    .map(|row| row.payload)),
                None => Ok(None),
            };
        }

        let payload: Option<String> = sqlx::query_scalar(
            "
            SELECT payload
            FROM resource_history_cleanup_plans
            WHERE id = ? AND revision = ? AND consumed = 0
            ",
        )
        .bind(key)
        .bind(revision)
        .fetch_optional(handle.pool())
        .await?;

        Ok(payload)
    }

    /// Whether the plan `key` has been executed.
    pub(crate) async fn consumed(
        handle: &Handle,
        key: &str,
    ) -> crate::Result<bool> {
        if let Some(selected) = super::recovery_documents::snapshot_records(
            handle.pool(),
            "resource_history_cleanup_plans",
            Some(key),
            None,
        )
        .await?
        {
            let row = selected.into_iter().next().ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Operation record is missing or changed".into(),
                )
            })?;
            return Ok(row.decode::<CleanupPlanRecord>().await?.consumed);
        }

        let consumed: bool = sqlx::query_scalar(
            "
            SELECT consumed
            FROM resource_history_cleanup_plans
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_one(handle.pool())
        .await?;

        Ok(consumed)
    }

    /// Records the progress made on a plan that is still unused.
    pub(crate) async fn save_progress(
        handle: &Handle,
        key: &str,
        payload: &str,
    ) -> crate::Result<()> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    if let Some(mut row) =
                        Self::read(conn, &key.to_owned()).await?
                    {
                        if row.consumed {
                            return Ok(());
                        }
                        row.payload = payload.into();
                        Self::write(conn, &key.to_owned(), Some(&row)).await?;
                    }
                    return Ok(());
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        sqlx::query(
            "
            UPDATE resource_history_cleanup_plans
            SET payload = ?
            WHERE id = ? AND consumed = 0
            ",
        )
        .bind(payload)
        .bind(key)
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    /// Records the progress made on a plan that was already executed.
    pub(crate) async fn save_payload(
        handle: &Handle,
        key: &str,
        payload: &str,
    ) -> crate::Result<()> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    if let Some(mut row) =
                        Self::read(conn, &key.to_owned()).await?
                    {
                        row.payload = payload.into();
                        Self::write(conn, &key.to_owned(), Some(&row)).await?;
                    }
                    return Ok(());
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        sqlx::query(
            "
            UPDATE resource_history_cleanup_plans
            SET payload = ?
            WHERE id = ?
            ",
        )
        .bind(payload)
        .bind(key)
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    /// Records the final progress and marks the plan used.
    pub(crate) async fn consume(
        handle: &Handle,
        key: &str,
        payload: &str,
    ) -> crate::Result<()> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    if let Some(mut row) =
                        Self::read(conn, &key.to_owned()).await?
                    {
                        row.payload = payload.into();
                        row.consumed = true;
                        Self::write(conn, &key.to_owned(), Some(&row)).await?;
                    }
                    return Ok(());
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        sqlx::query(
            "
            UPDATE resource_history_cleanup_plans
            SET payload = ?, consumed = 1
            WHERE id = ?
            ",
        )
        .bind(payload)
        .bind(key)
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    /// [`Self::consume`] inside the unit that removes the backups.
    pub(crate) async fn consume_in(
        conn: &mut SqliteConnection,
        key: &str,
        payload: &str,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            if let Some(mut row) = Self::read(conn, &key.to_owned()).await? {
                row.payload = payload.into();
                row.consumed = true;
                Self::write(conn, &key.to_owned(), Some(&row)).await?;
            }
            return Ok(());
        }

        sqlx::query(
            "
            UPDATE resource_history_cleanup_plans
            SET payload = ?, consumed = 1
            WHERE id = ?
            ",
        )
        .bind(payload)
        .bind(key)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &CleanupPlanRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_history_cleanup_plans
            (id, instance_id, revision, payload, consumed)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            instance_id = excluded.instance_id,
            revision = excluded.revision,
            payload = excluded.payload,
            consumed = excluded.consumed
        ",
    )
    .bind(key)
    .bind(&record.instance_id)
    .bind(record.revision)
    .bind(&record.payload)
    .bind(record.consumed)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
