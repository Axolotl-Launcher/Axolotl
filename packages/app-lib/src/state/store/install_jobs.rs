//! An install job: what it was asked to do, how far it got, and its state.

use sqlx::{Executor, Sqlite, SqliteConnection};

use super::Collection;

/// The jobs, one per installation run.
pub(crate) struct InstallJobs;

/// A job is identified by its own id.
pub(crate) type InstallJobKey = String;

/// Which jobs a read wants: those in one status, or those of one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstallJobScope {
    Status(String),
    Instance(String),
}

/// What a job carries.
///
/// The progress a job has made is kept as the text it is stored as: what the
/// text says is the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallJobRecord {
    pub instance_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub state: String,
    pub created: i64,
    pub modified: i64,
    pub finished: Option<i64>,
    pub dismissed: bool,
}

impl Collection for InstallJobs {
    type Key = InstallJobKey;
    type Scope = InstallJobScope;
    type Record = InstallJobRecord;

    /// A job's status changes under it, so the collection cannot tell from the
    /// key whether a job is in a given scope: what decides that is the record,
    /// and the replacement checks each record against the scope before it
    /// writes anything.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<JobRow> = sqlx::query_as(
            "
            SELECT instance_id, kind, status, state, created, modified,
                finished, dismissed
            FROM install_jobs
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
            sqlx::query("DELETE FROM install_jobs WHERE id = ?")
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
        let rows: Vec<JobRowWithKey> = sqlx::query_as(
            "
            SELECT id, instance_id, kind, status, state, created, modified,
                finished, dismissed
            FROM install_jobs
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
        delete_omitted(conn, None, records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<JobRowWithKey> = match scope {
            InstallJobScope::Status(status) => {
                sqlx::query_as(
                    "
                    SELECT id, instance_id, kind, status, state, created,
                        modified, finished, dismissed
                    FROM install_jobs
                    WHERE status = ?
                    ",
                )
                .bind(status)
                .fetch_all(&mut *conn)
                .await?
            }
            InstallJobScope::Instance(instance_id) => {
                sqlx::query_as(
                    "
                    SELECT id, instance_id, kind, status, state, created,
                        modified, finished, dismissed
                    FROM install_jobs
                    WHERE instance_id = ?
                    ",
                )
                .bind(instance_id)
                .fetch_all(&mut *conn)
                .await?
            }
        };

        Ok(rows.into_iter().map(split_row).collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        delete_omitted(conn, Some(scope), records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

type JobRow = (
    Option<String>,
    String,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    bool,
);
type JobRowWithKey = (
    String,
    Option<String>,
    String,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    bool,
);

fn split_row(
    (
        id,
        instance_id,
        kind,
        status,
        state,
        created,
        modified,
        finished,
        dismissed,
    ): JobRowWithKey,
) -> (String, InstallJobRecord) {
    (
        id,
        InstallJobRecord {
            instance_id,
            kind,
            status,
            state,
            created,
            modified,
            finished,
            dismissed,
        },
    )
}

fn to_record(row: JobRow) -> InstallJobRecord {
    let (
        instance_id,
        kind,
        status,
        state,
        created,
        modified,
        finished,
        dismissed,
    ) = row;
    InstallJobRecord {
        instance_id,
        kind,
        status,
        state,
        created,
        modified,
        finished,
        dismissed,
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &InstallJobRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO install_jobs (
            id, instance_id, kind, status, state, created, modified,
            finished, dismissed
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            instance_id = excluded.instance_id,
            kind = excluded.kind,
            status = excluded.status,
            state = excluded.state,
            created = excluded.created,
            modified = excluded.modified,
            finished = excluded.finished,
            dismissed = excluded.dismissed
        ",
    )
    .bind(key)
    .bind(&record.instance_id)
    .bind(&record.kind)
    .bind(&record.status)
    .bind(&record.state)
    .bind(record.created)
    .bind(record.modified)
    .bind(record.finished)
    .bind(record.dismissed)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the jobs a replacement leaves out, and only those: a job the
/// replacement keeps has to survive, because the plan it was started from hangs
/// off it.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<&InstallJobScope>,
    records: &[(InstallJobKey, InstallJobRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        Some(InstallJobScope::Status(status)) => {
            sqlx::query_scalar("SELECT id FROM install_jobs WHERE status = ?")
                .bind(status)
                .fetch_all(&mut *conn)
                .await?
        }
        Some(InstallJobScope::Instance(instance_id)) => {
            sqlx::query_scalar(
                "SELECT id FROM install_jobs WHERE instance_id = ?",
            )
            .bind(instance_id)
            .fetch_all(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_scalar("SELECT id FROM install_jobs")
                .fetch_all(&mut *conn)
                .await?
        }
    };

    for id in existing {
        if records.iter().any(|(key, _)| key == &id) {
            continue;
        }
        sqlx::query("DELETE FROM install_jobs WHERE id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

/// A scoped replacement may only touch the jobs of that scope, and the jobs it
/// writes must be in it: a job whose status differs from the status being
/// replaced would move another group, and one that names another instance would
/// move between instances.
fn ensure_records_are_in_scope(
    scope: &InstallJobScope,
    records: &[(InstallJobKey, InstallJobRecord)],
) -> crate::Result<()> {
    for (key, record) in records {
        let in_scope = match scope {
            InstallJobScope::Status(status) => &record.status == status,
            InstallJobScope::Instance(instance_id) => {
                record.instance_id.as_deref() == Some(instance_id)
            }
        };
        if !in_scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Install job {key} is not among the jobs of the scope it was written to"
            ))
            .into());
        }
    }

    Ok(())
}

/// A scoped replacement writes by key, so a key that already belongs to another
/// scope would move that job between groups instead of replacing this one.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &InstallJobScope,
    records: &[(InstallJobKey, InstallJobRecord)],
) -> crate::Result<()> {
    for (key, _) in records {
        let Some(existing) = InstallJobs::read(conn, key).await? else {
            continue;
        };
        let in_scope = match scope {
            InstallJobScope::Status(status) => &existing.status == status,
            InstallJobScope::Instance(instance_id) => {
                existing.instance_id.as_deref() == Some(instance_id)
            }
        };
        if !in_scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Install job {key} belongs to another scope"
            ))
            .into());
        }
    }

    Ok(())
}

impl InstallJobs {
    /// Opens a job under `key`, refusing an id that is already taken: a job is
    /// opened once, and a second row under the same id would stand for the
    /// history the first one keeps.
    pub(crate) async fn insert_in(
        conn: &mut SqliteConnection,
        key: &InstallJobKey,
        record: &InstallJobRecord,
    ) -> crate::Result<()> {
        sqlx::query(
            "
            INSERT INTO install_jobs (
                id, instance_id, kind, status, state, created, modified,
                finished, dismissed
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
        )
        .bind(key)
        .bind(&record.instance_id)
        .bind(&record.kind)
        .bind(&record.status)
        .bind(&record.state)
        .bind(record.created)
        .bind(record.modified)
        .bind(record.finished)
        .bind(record.dismissed)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    /// Whether one instance still has a job in one of `statuses`.
    pub(crate) async fn unfinished_in<'e, E>(
        exec: E,
        instance_id: &str,
        statuses: &[&str],
    ) -> crate::Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let filter = status_filter(statuses);
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM install_jobs WHERE instance_id = ? AND {filter})"
        );
        let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(instance_id);
        for status in statuses {
            query = query.bind(*status);
        }
        let busy: i64 = query.fetch_one(exec).await?;

        Ok(busy != 0)
    }

    /// Whether any job is in one of `statuses`, or still holds a rollback.
    pub(crate) async fn unfinished_or_rollback<'e, E>(
        exec: E,
        statuses: &[&str],
    ) -> crate::Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let filter = status_filter(statuses);
        let sql = format!(
            "SELECT COUNT(*) FROM install_jobs WHERE {filter} OR json_extract(state, '$.rollback') IS NOT NULL"
        );
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for status in statuses {
            query = query.bind(*status);
        }
        let busy: i64 = query.fetch_one(exec).await?;

        Ok(busy > 0)
    }

    /// Records that a job finished, and only while it was still running, or
    /// while a cancel was still being wound down. Answers how many rows it
    /// reached, so the caller can tell a job that moved on under it.
    pub(crate) async fn finish_in(
        conn: &mut SqliteConnection,
        finish: &JobFinish<'_>,
    ) -> crate::Result<u64> {
        let result = sqlx::query(
            "
            UPDATE install_jobs
            SET instance_id = (SELECT id FROM instances WHERE id = ?),
                status = ?, state = ?, modified = ?, finished = ?
            WHERE id = ? AND (status = ? OR (status = ? AND ?))
            ",
        )
        .bind(finish.instance_id)
        .bind(finish.status)
        .bind(finish.state)
        .bind(finish.modified)
        .bind(finish.finished)
        .bind(finish.key)
        .bind(finish.running_status)
        .bind(finish.canceling_status)
        .bind(finish.accept_canceling)
        .execute(&mut *conn)
        .await?;

        Ok(result.rows_affected())
    }
}

/// What the last write of a job records, and the states it may leave.
pub(crate) struct JobFinish<'a> {
    pub key: &'a str,
    /// The instance the job lands on, or none when it no longer exists.
    pub instance_id: Option<&'a str>,
    pub status: &'a str,
    pub state: &'a str,
    pub modified: i64,
    pub finished: i64,
    /// The status the job has to still hold for the write to land.
    pub running_status: &'a str,
    /// A job left in this status lands as well, when `accept_canceling`.
    pub canceling_status: &'a str,
    pub accept_canceling: bool,
}

/// The statuses a read asks about, as the condition it asks them in.
fn status_filter(statuses: &[&str]) -> String {
    if statuses.is_empty() {
        return "0".to_string();
    }
    let placeholders = vec!["?"; statuses.len()].join(", ");
    format!("status IN ({placeholders})")
}
