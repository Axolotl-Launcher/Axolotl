//! The reviewed plans that remove old backups.

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
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl ResourceHistoryCleanupPlans {
    /// Every plan saved for the instance, consumed or not.
    pub(crate) async fn payloads_for_instance(
        handle: &Handle,
        instance_id: &str,
    ) -> crate::Result<Vec<String>> {
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
