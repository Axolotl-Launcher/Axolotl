//! An install job: what it was asked to do, how far it got, and its state.

use sqlx::SqliteConnection;

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::read(conn, "install_jobs", key)
                .await;
        }

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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::write(
                conn,
                "install_jobs",
                key,
                record,
            )
            .await;
        }

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
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::all(conn, "install_jobs").await;
        }

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
        if super::recovery_documents::active(conn).await? {
            for (key, _) in super::recovery_documents::strict_all::<
                InstallJobRecord,
            >(conn, "install_jobs")
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
        if super::recovery_documents::active(conn).await? {
            return Ok(Self::read_all(conn)
                .await?
                .into_iter()
                .filter(|(key, record)| match scope {
                    InstallJobScope::Status(status) => &record.status == status,
                    InstallJobScope::Instance(id) => {
                        record.instance_id.as_deref() == Some(id)
                    }
                })
                .collect());
        }

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
        if super::recovery_documents::active(conn).await? {
            for (key, _) in records {
                if let Some(existing) = Self::read(conn, key).await? {
                    let record = &existing;
                    if !(match scope {
                        InstallJobScope::Status(status) => {
                            &record.status == status
                        }
                        InstallJobScope::Instance(id) => {
                            record.instance_id.as_deref() == Some(id)
                        }
                    }) {
                        return Err(crate::ErrorKind::InputError(
                            "Operation key belongs to another scope".into(),
                        )
                        .into());
                    }
                }
            }
            for (key, record) in records {
                if !(match scope {
                    InstallJobScope::Status(status) => &record.status == status,
                    InstallJobScope::Instance(id) => {
                        record.instance_id.as_deref() == Some(id)
                    }
                }) {
                    return Err(crate::ErrorKind::InputError(
                        "Operation record outside replacement scope".into(),
                    )
                    .into());
                }
            }
            for (key, _) in match scope {
                InstallJobScope::Instance(owner) => {
                    super::recovery_documents::strict_owner::<InstallJobRecord>(
                        conn,
                        "install_jobs",
                        owner,
                    )
                    .await?
                }
                InstallJobScope::Status(_) => {
                    super::recovery_documents::strict_all::<InstallJobRecord>(
                        conn,
                        "install_jobs",
                    )
                    .await?
                }
            }
            .into_iter()
            .filter(|(key, record)| match scope {
                InstallJobScope::Status(status) => &record.status == status,
                InstallJobScope::Instance(id) => {
                    record.instance_id.as_deref() == Some(id)
                }
            }) {
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
        if super::recovery_documents::active(conn).await? {
            if Self::read(conn, key).await?.is_some() {
                return Err(crate::ErrorKind::InputError(
                    "Job id already exists".into(),
                )
                .into());
            }
            return Self::write(conn, key, Some(record)).await;
        }

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
    pub(crate) async fn unfinished_in(
        exec: &mut SqliteConnection,
        instance_id: &str,
        statuses: &[&str],
    ) -> crate::Result<bool> {
        if super::recovery_documents::active(exec).await? {
            let keys:Vec<String>=sqlx::query_scalar("SELECT id FROM main.operation_publications WHERE kind='install_jobs' AND instance_id=? AND deleted=0").bind(instance_id).fetch_all(&mut *exec).await?;
            for key in keys {
                match super::recovery_documents::read_raw(
                    exec,
                    "install_jobs",
                    &key,
                )
                .await
                {
                    Ok(Some(row)) => {
                        if row["status"] == "succeeded"
                            && row["finished"].as_i64().is_some()
                        {
                            continue;
                        }
                        if super::recovery_documents::read::<InstallJobRecord>(
                            exec,
                            "install_jobs",
                            &key,
                        )
                        .await
                        .is_err()
                            || row["status"].as_str().is_some_and(|status| {
                                statuses.contains(&status)
                            })
                        {
                            return Ok(true);
                        }
                    }
                    Ok(None) => {}
                    Err(_) => return Ok(true),
                }
            }
            return Ok(false);
        }

        let filter = status_filter(statuses);
        let unknown = super::compatibility::unsupported_active_job("");
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM install_jobs WHERE instance_id = ? AND ({filter} OR {unknown}))"
        );
        let mut query = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
            .bind(instance_id);
        for status in statuses {
            query = query.bind(*status);
        }
        let busy: i64 = query.fetch_one(exec).await?;

        Ok(busy != 0)
    }

    /// Whether any job is in one of `statuses`, or holds a rollback that has
    /// not been restored yet: one that still keeps its content snapshot, or
    /// one whose restore failed. A completed rollback discards its content but
    /// keeps the instance metadata, so failed and interrupted jobs stop
    /// counting once their rollback has succeeded.
    pub(crate) async fn unfinished_or_rollback(
        exec: &mut SqliteConnection,
        statuses: &[&str],
    ) -> crate::Result<bool> {
        if super::recovery_documents::active(exec).await? {
            return Ok(
                super::recovery_documents::strict_all::<InstallJobRecord>(
                    exec,
                    "install_jobs",
                )
                .await?
                .into_iter()
                .any(|(_, row)| {
                    statuses.contains(&row.status.as_str())
                        || serde_json::from_str::<serde_json::Value>(&row.state)
                            .ok()
                            .is_some_and(|state| {
                                !state["rollback"]["content"].is_null()
                                    || (!state["rollback"].is_null()
                                        && !state["rollback_error"].is_null())
                            })
                }),
            );
        }

        let filter = status_filter(statuses);
        let unknown = super::compatibility::unsupported_active_job("");
        let opaque = super::compatibility::unsupported_job("");
        let sql = format!(
            "SELECT COUNT(*) FROM install_jobs WHERE {filter} OR {unknown} OR CASE WHEN ({opaque}) THEN 0 ELSE json_extract(state, '$.rollback.content') IS NOT NULL OR (json_extract(state, '$.rollback') IS NOT NULL AND json_extract(state, '$.rollback_error') IS NOT NULL) END"
        );
        let mut query = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql));
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
        if super::recovery_documents::active(conn).await? {
            let Some(mut row) =
                Self::read(conn, &finish.key.to_owned()).await?
            else {
                return Ok(0);
            };
            if row.status != finish.running_status
                && !(row.status == finish.canceling_status
                    && finish.accept_canceling)
            {
                return Ok(0);
            }
            row.instance_id = finish.instance_id.map(str::to_owned);
            row.status = finish.status.into();
            row.state = finish.state.into();
            row.modified = finish.modified;
            row.finished = Some(finish.finished);
            Self::write(conn, &finish.key.to_owned(), Some(&row)).await?;
            return Ok(1);
        }

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
