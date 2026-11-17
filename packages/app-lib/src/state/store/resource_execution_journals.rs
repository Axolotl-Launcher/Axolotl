//! The files a resource plan was between, while it publishes them.
//!
//! A journal names every file an interrupted execution had already moved, so a
//! retry can put them back before it starts again.

use sqlx::Connection as _;
use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The journals, one per plan that is executing.
pub(crate) struct ResourceExecutionJournals;

/// A journal is identified by the plan it belongs to.
pub(crate) type ExecutionJournalKey = String;

/// What a journal carries.
///
/// The journal is kept as the text it is stored as: what the text says is the
/// caller's business.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ExecutionJournalRecord {
    pub payload: String,
}

impl Collection for ResourceExecutionJournals {
    type Key = ExecutionJournalKey;
    type Scope = ExecutionJournalKey;
    type Record = ExecutionJournalRecord;

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
                "resource_execution_journals",
                key,
            )
            .await;
        }

        let payload: Option<String> = sqlx::query_scalar(
            "
            SELECT payload
            FROM resource_execution_journals
            WHERE plan_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(payload.map(|payload| Self::Record { payload }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::write(
                conn,
                "resource_execution_journals",
                key,
                record,
            )
            .await;
        }

        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM resource_execution_journals WHERE plan_id = ?",
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
                "resource_execution_journals",
            )
            .await;
        }

        let rows: Vec<(String, String)> = sqlx::query_as(
            "
            SELECT plan_id, payload
            FROM resource_execution_journals
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(plan_id, payload)| (plan_id, Self::Record { payload }))
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            for (key, _) in super::recovery_documents::strict_all::<
                ExecutionJournalRecord,
            >(conn, "resource_execution_journals")
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

        sqlx::query("DELETE FROM resource_execution_journals")
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

impl ResourceExecutionJournals {
    /// The journal `key` saved, read before a retry replays it.
    pub(crate) async fn for_plan(
        handle: &Handle,
        key: &str,
    ) -> crate::Result<Option<String>> {
        if let Some(selected) = super::recovery_documents::snapshot_records(
            handle.pool(),
            "resource_execution_journals",
            Some(key),
            None,
        )
        .await?
        {
            return match selected.into_iter().next() {
                Some(row) => Ok(Some(
                    row.decode::<ExecutionJournalRecord>().await?.payload,
                )),
                None => Ok(None),
            };
        }
        let key = key.to_owned();
        Ok(Self::get(handle, &key).await?.map(|record| record.payload))
    }

    /// The journal of a plan that has not completed, the only one a recovery
    /// replays.
    pub(crate) async fn pending_payload(
        handle: &Handle,
        key: &str,
    ) -> crate::Result<Option<String>> {
        if let Some(selected) =
            super::recovery_documents::snapshot_resource_pairs(
                handle.pool(),
                Some(key),
                None,
            )
            .await?
        {
            let plan = selected
                .iter()
                .find(|row| row.kind() == "resource_operation_plans");
            let plan = match plan {
                Some(plan) => Some(plan.decode::<super::resource_operation_plans::ResourceOperationPlanRecord>().await?),
                None => None,
            };
            return if plan.is_some_and(|row| !row.completed) {
                match selected
                    .iter()
                    .find(|row| row.kind() == "resource_execution_journals")
                {
                    Some(journal) => Ok(Some(
                        journal
                            .decode::<ExecutionJournalRecord>()
                            .await?
                            .payload,
                    )),
                    None => Ok(None),
                }
            } else {
                Ok(None)
            };
        }

        let payload: Option<String> = sqlx::query_scalar(
            "
            SELECT journal.payload
            FROM resource_execution_journals journal
            JOIN resource_operation_plans plan ON plan.id = journal.plan_id
            WHERE journal.plan_id = ? AND plan.completed = 0
            ",
        )
        .bind(key)
        .fetch_optional(handle.pool())
        .await?;

        Ok(payload)
    }

    /// Whether the instance has a plan whose journal awaits recovery.
    pub(crate) async fn pending_for_instance(
        handle: &Handle,
        instance_id: &str,
    ) -> crate::Result<bool> {
        if let Some(selected) =
            super::recovery_documents::snapshot_resource_pairs(
                handle.pool(),
                None,
                Some(instance_id),
            )
            .await?
        {
            // Decode every selected journal strictly before answering. Missing
            // or opaque recovery evidence must never authorize cleanup.
            let mut journals = Vec::new();
            for row in selected
                .iter()
                .filter(|row| row.kind() == "resource_execution_journals")
            {
                row.decode::<ExecutionJournalRecord>().await?;
                journals.push(row.key.as_str());
            }
            for key in journals {
                if let Some(plan) = selected.iter().find(|row| {
                    row.kind() == "resource_operation_plans" && row.key == key
                }) {
                    if plan.decode::<super::resource_operation_plans::ResourceOperationPlanRecord>().await?.instance_id == instance_id {
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }

        let pending: i64 = sqlx::query_scalar(
            "
            SELECT EXISTS(
                SELECT 1
                FROM resource_execution_journals journal
                JOIN resource_operation_plans plan
                    ON plan.id = journal.plan_id
                WHERE plan.instance_id = ?)
            ",
        )
        .bind(instance_id)
        .fetch_one(handle.pool())
        .await?;

        Ok(pending != 0)
    }

    /// Saves the journal of a plan that is about to publish its files.
    pub(crate) async fn save(
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
                    return Self::write(
                        conn,
                        &key.to_owned(),
                        Some(&ExecutionJournalRecord {
                            payload: payload.into(),
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
        write_row(
            &mut connection,
            key,
            &ExecutionJournalRecord {
                payload: payload.to_owned(),
            },
        )
        .await
    }

    /// Drops the journal of a plan that no longer needs replaying.
    pub(crate) async fn delete(
        handle: &Handle,
        key: &str,
    ) -> crate::Result<()> {
        let key = key.to_owned();
        Self::set(handle, &key, None).await
    }

    /// [`Self::delete`] inside the unit that publishes the plan.
    pub(crate) async fn delete_in(
        conn: &mut SqliteConnection,
        key: &str,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            return Self::write(conn, &key.to_owned(), None).await;
        }

        let key = key.to_owned();
        Self::write(conn, &key, None).await
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &ExecutionJournalRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_execution_journals (plan_id, payload)
        VALUES (?, ?)
        ON CONFLICT(plan_id) DO UPDATE SET payload = excluded.payload
        ",
    )
    .bind(key)
    .bind(&record.payload)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
