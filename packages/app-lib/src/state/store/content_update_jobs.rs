//! The install job a content update plan was started as.

use sqlx::SqliteConnection;

use super::Collection;

/// The plan links, one per content update plan.
pub(crate) struct ContentUpdateJobs;

/// A link is identified by the plan it belongs to.
pub(crate) type ContentUpdateJobKey = String;

/// What a link carries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ContentUpdateJobRecord {
    pub job_id: Option<String>,
}

impl Collection for ContentUpdateJobs {
    type Key = ContentUpdateJobKey;
    type Scope = ContentUpdateJobKey;
    type Record = ContentUpdateJobRecord;

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
                "content_update_jobs",
                key,
            )
            .await;
        }

        let row: Option<Option<String>> = sqlx::query_scalar(
            "SELECT job_id FROM content_update_jobs WHERE plan_id = ?",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|job_id| Self::Record { job_id }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(conn).await? {
            return super::recovery_documents::write(
                conn,
                "content_update_jobs",
                key,
                record,
            )
            .await;
        }

        let Some(record) = record else {
            sqlx::query("DELETE FROM content_update_jobs WHERE plan_id = ?")
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
            return super::recovery_documents::all(conn, "content_update_jobs")
                .await;
        }

        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT plan_id, job_id FROM content_update_jobs")
                .fetch_all(&mut *conn)
                .await?;

        Ok(rows
            .into_iter()
            .map(|(plan_id, job_id)| (plan_id, Self::Record { job_id }))
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
                    "content_update_jobs",
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

        sqlx::query("DELETE FROM content_update_jobs")
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

impl ContentUpdateJobs {
    /// Links a job to the plan it was queued for, refusing a plan that already
    /// carries one: a plan is queued once.
    pub(crate) async fn insert_in(
        exec: &mut SqliteConnection,
        key: &ContentUpdateJobKey,
        record: &ContentUpdateJobRecord,
    ) -> crate::Result<()> {
        if super::recovery_documents::active(exec).await? {
            if Self::read(exec, key).await?.is_some() {
                return Err(crate::ErrorKind::InputError(
                    "Plan already queued".into(),
                )
                .into());
            }
            return Self::write(exec, key, Some(record)).await;
        }

        sqlx::query(
            "INSERT INTO content_update_jobs(plan_id, job_id) VALUES (?, ?)",
        )
        .bind(key)
        .bind(&record.job_id)
        .execute(exec)
        .await?;

        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &ContentUpdateJobRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO content_update_jobs (plan_id, job_id)
        VALUES (?, ?)
        ON CONFLICT (plan_id) DO UPDATE SET job_id = excluded.job_id
        ",
    )
    .bind(key)
    .bind(&record.job_id)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
