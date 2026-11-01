//! The Java runtimes the launcher has installed, one per executable path.
//!
//! A runtime's path identifies it, and the major version it answers for groups
//! the runtimes of that version.

use sqlx::SqliteConnection;

use super::Collection;

/// The installed Java runtimes.
pub(crate) struct JavaVersions;

/// A runtime is identified by the path of its executable.
pub(crate) type JavaVersionKey = String;

/// What a runtime carries beyond its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaVersionRecord {
    pub major_version: u32,
    pub full_version: String,
    pub architecture: String,
    pub distribution: Option<String>,
}

impl Collection for JavaVersions {
    type Key = JavaVersionKey;
    type Scope = u32;
    type Record = JavaVersionRecord;

    /// The major version a runtime answers for is part of the record rather
    /// than of the key, so the collection cannot tell from a key whether it is
    /// in a scope: the replacement checks each record before it writes.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<StoredRow> = sqlx::query_as(
            "
            SELECT path, major_version, full_version, architecture,
                distribution
            FROM java_versions
            WHERE path = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(to_record).map(|(_, record)| record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM java_versions WHERE path = ?")
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
        let rows: Vec<StoredRow> = sqlx::query_as(
            "
            SELECT path, major_version, full_version, architecture,
                distribution
            FROM java_versions
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().filter_map(to_record).collect())
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
        let rows: Vec<StoredRow> = sqlx::query_as(
            "
            SELECT path, major_version, full_version, architecture,
                distribution
            FROM java_versions
            WHERE major_version = ?
            ",
        )
        .bind(i64::from(*scope))
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().filter_map(to_record).collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        delete_omitted(conn, Some(*scope), records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

type StoredRow = (String, i64, String, String, Option<String>);

/// The runtime a row describes, or nothing when its major version does not fit
/// the record.
fn to_record(row: StoredRow) -> Option<(JavaVersionKey, JavaVersionRecord)> {
    let (path, major_version, full_version, architecture, distribution) = row;
    let Some(major_version) = u32::try_from(major_version).ok() else {
        tracing::warn!(
            %path,
            %full_version,
            major_version,
            "Ignoring a Java runtime whose major version is out of range"
        );
        return None;
    };

    Some((
        path,
        JavaVersionRecord {
            major_version,
            full_version,
            architecture,
            distribution,
        },
    ))
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &JavaVersionKey,
    record: &JavaVersionRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO java_versions
            (major_version, full_version, architecture, path, distribution)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (path) DO UPDATE SET
            major_version = excluded.major_version,
            full_version = excluded.full_version,
            architecture = excluded.architecture,
            distribution = excluded.distribution
        ",
    )
    .bind(i64::from(record.major_version))
    .bind(&record.full_version)
    .bind(&record.architecture)
    .bind(key)
    .bind(&record.distribution)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the runtimes a replacement leaves out, and only those: the default
/// of a major version hangs off the runtime it names.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<u32>,
    records: &[(JavaVersionKey, JavaVersionRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        Some(major_version) => {
            sqlx::query_scalar(
                "SELECT path FROM java_versions WHERE major_version = ?",
            )
            .bind(i64::from(major_version))
            .fetch_all(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_scalar("SELECT path FROM java_versions")
                .fetch_all(&mut *conn)
                .await?
        }
    };

    for path in existing {
        if records.iter().any(|(key, _)| key == &path) {
            continue;
        }
        sqlx::query("DELETE FROM java_versions WHERE path = ?")
            .bind(&path)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

/// A scoped replacement may only touch the runtimes of that major version, so a
/// record answering for another one would move a runtime between scopes.
fn ensure_records_are_in_scope(
    scope: &u32,
    records: &[(JavaVersionKey, JavaVersionRecord)],
) -> crate::Result<()> {
    for (key, record) in records {
        if record.major_version == *scope {
            continue;
        }
        return Err(crate::ErrorKind::InputError(format!(
            "Java runtime {key} is not among the runtimes of that major version"
        ))
        .into());
    }

    Ok(())
}

/// A scoped replacement writes by path, so a path that is already recorded under
/// another version would move that runtime instead of replacing this one.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &<JavaVersions as Collection>::Scope,
    records: &[(
        <JavaVersions as Collection>::Key,
        <JavaVersions as Collection>::Record,
    )],
) -> crate::Result<()> {
    for (key, _) in records {
        let Some(existing) = JavaVersions::read(conn, key).await? else {
            continue;
        };
        if existing.major_version != *scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Java runtime {key} belongs to another version"
            ))
            .into());
        }
    }

    Ok(())
}

/// The runtimes as the rows hold them, for the document handover and for the
/// fallback the domain answers with while it has no document.
pub(crate) async fn read_legacy_java_versions(
    pool: &sqlx::SqlitePool,
) -> crate::Result<Vec<(JavaVersionKey, JavaVersionRecord)>> {
    let rows: Vec<StoredRow> = sqlx::query_as(
        "SELECT path, major_version, full_version, architecture, distribution FROM java_versions ORDER BY path",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(to_record).collect())
}
