//! The Java runtimes a scan of the disk has already recognised.

use sqlx::SqliteConnection;

use super::Collection;

/// The runtimes the launcher found, one row per install path.
pub(crate) struct DiscoveredJavas;

/// A Java runtime is identified by where it is installed.
pub(crate) type DiscoveredJavaKey = String;

/// A runtime is grouped by the major version it answers.
pub(crate) type DiscoveredJavaScope = u32;

/// What a discovered runtime carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscoveredJavaRecord {
    pub major_version: u32,
    pub full_version: String,
    pub architecture: String,
    pub distribution: Option<String>,
    pub file_size: i64,
    pub file_mtime_ms: i64,
}

impl Collection for DiscoveredJavas {
    type Key = DiscoveredJavaKey;
    type Scope = DiscoveredJavaScope;
    type Record = DiscoveredJavaRecord;

    /// The major version lives in the record, so the collection cannot tell from
    /// the path alone whether a runtime is in a given version's group.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, String, String, Option<String>, i64, i64)> =
            sqlx::query_as(
                "
                SELECT major_version, full_version, architecture, distribution,
                    file_size, file_mtime_ms
                FROM discovered_javas
                WHERE path = ?
                ",
            )
            .bind(key)
            .fetch_optional(&mut *conn)
            .await?;

        Ok(row.and_then(
            |(
                major_version,
                full_version,
                architecture,
                distribution,
                file_size,
                mtime,
            )| {
                Some(Self::Record {
                    major_version: u32::try_from(major_version).ok()?,
                    full_version,
                    architecture,
                    distribution,
                    file_size,
                    file_mtime_ms: mtime,
                })
            },
        ))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM discovered_javas WHERE path = ?")
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
        let rows: Vec<(String, i64, String, String, Option<String>, i64, i64)> =
            sqlx::query_as(
                "
                SELECT path, major_version, full_version, architecture,
                    distribution, file_size, file_mtime_ms
                FROM discovered_javas
                ",
            )
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(
                    path,
                    major_version,
                    full_version,
                    architecture,
                    distribution,
                    size,
                    m,
                )| {
                    Some((
                        path,
                        Self::Record {
                            major_version: u32::try_from(major_version).ok()?,
                            full_version,
                            architecture,
                            distribution,
                            file_size: size,
                            file_mtime_ms: m,
                        },
                    ))
                },
            )
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM discovered_javas")
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
        let rows: Vec<(String, i64, String, String, Option<String>, i64, i64)> =
            sqlx::query_as(
                "
                SELECT path, major_version, full_version, architecture,
                    distribution, file_size, file_mtime_ms
                FROM discovered_javas
                WHERE major_version = ?
                ",
            )
            .bind(i64::from(*scope))
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(
                    path,
                    major_version,
                    full_version,
                    architecture,
                    distribution,
                    size,
                    m,
                )| {
                    Some((
                        path,
                        Self::Record {
                            major_version: u32::try_from(major_version).ok()?,
                            full_version,
                            architecture,
                            distribution,
                            file_size: size,
                            file_mtime_ms: m,
                        },
                    ))
                },
            )
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        sqlx::query("DELETE FROM discovered_javas WHERE major_version = ?")
            .bind(i64::from(*scope))
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

fn ensure_records_are_in_scope(
    scope: &DiscoveredJavaScope,
    records: &[(DiscoveredJavaKey, DiscoveredJavaRecord)],
) -> crate::Result<()> {
    if records
        .iter()
        .all(|(_, record)| record.major_version == *scope)
    {
        return Ok(());
    }
    Err(crate::ErrorKind::InputError(
        "A discovered runtime is not among the runtimes of the version it was written to"
            .to_string(),
    )
    .into())
}

/// A scoped replacement writes by path, so a path recorded under another version
/// would move that runtime instead of replacing this one.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &DiscoveredJavaScope,
    records: &[(DiscoveredJavaKey, DiscoveredJavaRecord)],
) -> crate::Result<()> {
    for (key, _) in records {
        let Some(existing) = DiscoveredJavas::read(conn, key).await? else {
            continue;
        };
        if existing.major_version != *scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Discovered runtime {key} belongs to another version"
            ))
            .into());
        }
    }

    Ok(())
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &DiscoveredJavaRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO discovered_javas (
            path, major_version, full_version, architecture, distribution,
            file_size, file_mtime_ms
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (path) DO UPDATE SET
            major_version = excluded.major_version,
            full_version = excluded.full_version,
            architecture = excluded.architecture,
            distribution = excluded.distribution,
            file_size = excluded.file_size,
            file_mtime_ms = excluded.file_mtime_ms
        ",
    )
    .bind(key)
    .bind(i64::from(record.major_version))
    .bind(&record.full_version)
    .bind(&record.architecture)
    .bind(&record.distribution)
    .bind(record.file_size)
    .bind(record.file_mtime_ms)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
