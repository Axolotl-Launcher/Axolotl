//! A prepared content operation, kept as the draft it was reviewed from.

use sqlx::SqliteConnection;

use super::Collection;

/// The plans, one per prepared operation.
pub(crate) struct ResourceOperationPlans;

/// A plan is identified by its own id.
pub(crate) type ResourceOperationPlanKey = String;

/// The plans of one instance.
pub(crate) type ResourceOperationPlanScope = String;

/// What a plan carries.
///
/// The operation itself is kept as the text it is stored as: what the text says
/// is the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceOperationPlanRecord {
    pub instance_id: String,
    pub revision: i64,
    pub payload: String,
    pub job_id: Option<String>,
    pub queued: bool,
    pub completed: bool,
}

impl Collection for ResourceOperationPlans {
    type Key = ResourceOperationPlanKey;
    type Scope = ResourceOperationPlanScope;
    type Record = ResourceOperationPlanRecord;

    /// The instance a plan belongs to is in the record, not in its id, so what
    /// decides whether a plan is in a scope is the record; the replacement
    /// checks each record against the scope before it writes anything.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<PlanRow> = sqlx::query_as(
            "
            SELECT instance_id, revision, payload, job_id, queued, completed
            FROM resource_operation_plans
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(to_record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM resource_operation_plans WHERE id = ?")
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
        let rows: Vec<PlanRowWithKey> = sqlx::query_as(
            "
            SELECT id, instance_id, revision, payload, job_id, queued,
                completed
            FROM resource_operation_plans
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(split_row).collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_operation_plans")
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
        let rows: Vec<PlanRowWithKey> = sqlx::query_as(
            "
            SELECT id, instance_id, revision, payload, job_id, queued,
                completed
            FROM resource_operation_plans
            WHERE instance_id = ?
            ",
        )
        .bind(scope)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(split_row).collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        delete_omitted(conn, scope, records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

type PlanRow = (String, i64, String, Option<String>, bool, bool);

type PlanRowWithKey = (String, String, i64, String, Option<String>, bool, bool);

fn split_row(
    (
        id,
        instance_id,
        revision,
        payload,
        job_id,
        queued,
        completed,
    ): PlanRowWithKey,
) -> (String, ResourceOperationPlanRecord) {
    (
        id,
        ResourceOperationPlanRecord {
            instance_id,
            revision,
            payload,
            job_id,
            queued,
            completed,
        },
    )
}

fn to_record(row: PlanRow) -> ResourceOperationPlanRecord {
    let (instance_id, revision, payload, job_id, queued, completed) = row;
    ResourceOperationPlanRecord {
        instance_id,
        revision,
        payload,
        job_id,
        queued,
        completed,
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &ResourceOperationPlanRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_operation_plans
            (id, instance_id, revision, payload, job_id, queued, completed)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            instance_id = excluded.instance_id,
            revision = excluded.revision,
            payload = excluded.payload,
            job_id = excluded.job_id,
            queued = excluded.queued,
            completed = excluded.completed
        ",
    )
    .bind(key)
    .bind(&record.instance_id)
    .bind(record.revision)
    .bind(&record.payload)
    .bind(&record.job_id)
    .bind(record.queued)
    .bind(record.completed)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the plans of the instance that the replacement leaves out, and only
/// those: a plan the replacement keeps has to survive, because the install job
/// of the instance may still be running it.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: &str,
    records: &[(ResourceOperationPlanKey, ResourceOperationPlanRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM resource_operation_plans WHERE instance_id = ?",
    )
    .bind(scope)
    .fetch_all(&mut *conn)
    .await?;

    for id in existing {
        if records.iter().any(|(key, _)| key == &id) {
            continue;
        }
        sqlx::query("DELETE FROM resource_operation_plans WHERE id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

/// A scoped replacement may only touch the plans of one instance, and the plans
/// it writes must belong to it.
fn ensure_records_are_in_scope(
    scope: &ResourceOperationPlanScope,
    records: &[(ResourceOperationPlanKey, ResourceOperationPlanRecord)],
) -> crate::Result<()> {
    if records
        .iter()
        .all(|(_, record)| &record.instance_id == scope)
    {
        return Ok(());
    }
    Err(crate::ErrorKind::InputError(
        "A plan is not among the plans of the scope it was written to"
            .to_string(),
    )
    .into())
}

/// A scoped replacement writes by id, so an id already recorded under another
/// instance would be taken from that instance instead of replaced here.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &ResourceOperationPlanScope,
    records: &[(ResourceOperationPlanKey, ResourceOperationPlanRecord)],
) -> crate::Result<()> {
    for (key, _) in records {
        let Some(existing) = ResourceOperationPlans::read(conn, key).await?
        else {
            continue;
        };
        if &existing.instance_id != scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Plan {key} belongs to another instance"
            ))
            .into());
        }
    }

    Ok(())
}
