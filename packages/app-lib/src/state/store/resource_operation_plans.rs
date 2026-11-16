//! A prepared content operation, kept as the draft it was reviewed from.

use sqlx::Connection as _;
use sqlx::SqliteConnection;

use super::{Collection, Handle};

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::read(
                conn,
                "resource_operation_plans",
                key,
            )
            .await;
        }

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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::write(
                conn,
                "resource_operation_plans",
                key,
                record,
            )
            .await;
        }

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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::all(
                conn,
                "resource_operation_plans",
            )
            .await;
        }

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
        if super::recovery_documents::active(conn).await? {
            for (key, _) in
                super::recovery_documents::strict_all::<Self::Record>(
                    conn,
                    "resource_operation_plans",
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
        if super::recovery_documents::active(conn).await? {
            return Ok(Self::read_all(conn)
                .await?
                .into_iter()
                .filter(|(key, record)| &record.instance_id == scope)
                .collect());
        }

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
        if super::recovery_documents::active(conn).await? {
            for (key, _) in records {
                if let Some(existing) = Self::read(conn, key).await? {
                    let record = &existing;
                    if !(&record.instance_id == scope) {
                        return Err(crate::ErrorKind::InputError(
                            "Operation key belongs to another scope".into(),
                        )
                        .into());
                    }
                }
            }
            for (key, record) in records {
                if !(&record.instance_id == scope) {
                    return Err(crate::ErrorKind::InputError(
                        "Operation record outside replacement scope".into(),
                    )
                    .into());
                }
            }
            for (key, _) in super::recovery_documents::strict_owner::<
                Self::Record,
            >(
                conn, "resource_operation_plans", scope
            )
            .await?
            .into_iter()
            .filter(|(key, record)| &record.instance_id == scope)
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

        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        delete_omitted(conn, scope, records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

impl ResourceOperationPlans {
    /// Whether `key` completed at `revision`, which only a plan that exists
    /// answers at all.
    pub(crate) async fn completed_at(
        handle: &Handle,
        key: &str,
        revision: u32,
    ) -> crate::Result<Option<i64>> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    return Ok(Self::read(conn, &key.to_owned())
                        .await?
                        .filter(|row| row.revision == i64::from(revision))
                        .map(|row| i64::from(row.completed)));
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        let completed: Option<i64> = sqlx::query_scalar(
            "
            SELECT completed
            FROM resource_operation_plans
            WHERE id = ? AND revision = ?
            ",
        )
        .bind(key)
        .bind(revision)
        .fetch_optional(handle.pool())
        .await?;

        Ok(completed)
    }

    /// The payload `key` stored at `revision`.
    pub(crate) async fn payload_at(
        handle: &Handle,
        key: &str,
        revision: u32,
    ) -> crate::Result<String> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    return Ok(Self::read(conn, &key.to_owned())
                        .await?
                        .filter(|row| row.revision == i64::from(revision))
                        .ok_or_else(|| {
                            crate::ErrorKind::InputError(
                                "Operation record is missing or changed".into(),
                            )
                        })?
                        .payload);
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        let payload: String = sqlx::query_scalar(
            "
            SELECT payload
            FROM resource_operation_plans
            WHERE id = ? AND revision = ?
            ",
        )
        .bind(key)
        .bind(revision)
        .fetch_one(handle.pool())
        .await?;

        Ok(payload)
    }

    /// The completion flag of `key`, whatever revision it is on.
    pub(crate) async fn completed(
        handle: &Handle,
        key: &str,
    ) -> crate::Result<i64> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    return Ok(i64::from(
                        Self::read(conn, &key.to_owned())
                            .await?
                            .ok_or_else(|| {
                                crate::ErrorKind::InputError(
                                    "Operation record is missing or changed"
                                        .into(),
                                )
                            })?
                            .completed,
                    ));
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        let completed: i64 = sqlx::query_scalar(
            "
            SELECT completed
            FROM resource_operation_plans
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_one(handle.pool())
        .await?;

        Ok(completed)
    }

    /// Marks `key` completed once its files and rows are published.
    pub(crate) async fn mark_completed(
        handle: &Handle,
        key: &str,
        revision: u32,
    ) -> crate::Result<()> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    return Self::mark_completed_in(conn, key, revision).await;
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
            UPDATE resource_operation_plans
            SET completed = 1
            WHERE id = ? AND revision = ?
            ",
        )
        .bind(key)
        .bind(revision)
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    /// [`Self::mark_completed`] inside the unit that publishes the plan.
    pub(crate) async fn mark_completed_in(
        conn: &mut SqliteConnection,
        key: &str,
        revision: u32,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            if let Some(mut row) = Self::read(conn, &key.to_owned()).await? {
                if row.revision == i64::from(revision) {
                    row.completed = true;
                    Self::write(conn, &key.to_owned(), Some(&row)).await?;
                }
            }
            return Ok(());
        }

        sqlx::query(
            "
            UPDATE resource_operation_plans
            SET completed = 1
            WHERE id = ? AND revision = ?
            ",
        )
        .bind(key)
        .bind(revision)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    /// Saves a reviewed plan before it is queued.
    pub(crate) async fn insert_prepared(
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
                            "Plan already exists".into(),
                        )
                        .into());
                    }
                    return Self::write(
                        conn,
                        &key.to_owned(),
                        Some(&ResourceOperationPlanRecord {
                            instance_id: instance_id.into(),
                            revision: i64::from(revision),
                            payload: payload.into(),
                            job_id: None,
                            queued: false,
                            completed: false,
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
            INSERT INTO resource_operation_plans
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

impl ResourceOperationPlans {
    /// The instance `key` was planned for at `revision`.
    pub(crate) async fn instance_at(
        handle: &Handle,
        key: &str,
        revision: u32,
    ) -> crate::Result<String> {
        {
            let mut connection = handle.connection().await?;
            if super::recovery_documents::active(&mut connection).await? {
                let mut transaction =
                    connection.begin_with("BEGIN IMMEDIATE").await?;
                let result = async {
                    let conn = &mut *transaction;
                    return Ok(Self::read(conn, &key.to_owned())
                        .await?
                        .filter(|row| row.revision == i64::from(revision))
                        .ok_or_else(|| {
                            crate::ErrorKind::InputError(
                                "Operation record is missing or changed".into(),
                            )
                        })?
                        .instance_id);
                }
                .await;
                if result.is_ok() {
                    transaction.commit().await?;
                }
                return result;
            }
        }

        let instance_id: String = sqlx::query_scalar(
            "
            SELECT instance_id
            FROM resource_operation_plans
            WHERE id = ? AND revision = ?
            ",
        )
        .bind(key)
        .bind(revision)
        .fetch_one(handle.pool())
        .await?;

        Ok(instance_id)
    }

    /// The job `key` was queued as at `revision`, read inside the caller's
    /// unit so the answer cannot go stale before its write lands.
    pub(crate) async fn job_at_in(
        conn: &mut SqliteConnection,
        key: &str,
        revision: u32,
    ) -> crate::Result<Option<String>> {
        if super::recovery_documents::active(conn).await? {
            return Ok(Self::read(conn, &key.to_owned())
                .await?
                .filter(|row| row.revision == i64::from(revision))
                .and_then(|row| row.job_id));
        }

        let job_id: Option<Option<String>> = sqlx::query_scalar(
            "
            SELECT job_id
            FROM resource_operation_plans
            WHERE id = ? AND revision = ?
            ",
        )
        .bind(key)
        .bind(revision)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(job_id.flatten())
    }

    /// The payload `key` stored at `revision`, as long as it is neither queued
    /// nor completed, read inside the caller's unit.
    pub(crate) async fn queueable_payload_in(
        conn: &mut SqliteConnection,
        key: &str,
        revision: u32,
    ) -> crate::Result<Option<String>> {
        if super::recovery_documents::active(conn).await? {
            return Ok(Self::read(conn, &key.to_owned())
                .await?
                .filter(|row| {
                    row.revision == i64::from(revision)
                        && !row.queued
                        && row.job_id.is_none()
                })
                .map(|row| row.payload));
        }

        let payload: Option<String> = sqlx::query_scalar(
            "
            SELECT payload
            FROM resource_operation_plans
            WHERE id = ? AND revision = ? AND queued = 0 AND job_id IS NULL
            ",
        )
        .bind(key)
        .bind(revision)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(payload)
    }

    /// Records the job `key` was queued as, and only while the plan is still
    /// unqueued at that revision. Answers how many rows it reached, so a plan
    /// that moved on is visible to the caller.
    pub(crate) async fn queue_in(
        conn: &mut SqliteConnection,
        key: &str,
        revision: u32,
        job_id: &str,
    ) -> crate::Result<u64> {
        if super::recovery_documents::active(conn).await? {
            let Some(mut row) = Self::read(conn, &key.to_owned()).await? else {
                return Ok(0);
            };
            if row.revision != i64::from(revision)
                || row.queued
                || row.job_id.is_some()
            {
                return Ok(0);
            }
            row.job_id = Some(job_id.into());
            row.queued = true;
            Self::write(conn, &key.to_owned(), Some(&row)).await?;
            return Ok(1);
        }

        let updated = sqlx::query(
            "
            UPDATE resource_operation_plans
            SET job_id = ?, queued = 1
            WHERE id = ? AND revision = ? AND queued = 0 AND job_id IS NULL
            ",
        )
        .bind(job_id)
        .bind(key)
        .bind(revision)
        .execute(&mut *conn)
        .await?;

        Ok(updated.rows_affected())
    }
}
