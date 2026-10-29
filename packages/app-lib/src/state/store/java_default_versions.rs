//! Which installed Java runtime answers for each major version.
//!
//! A major version has one default, naming the runtime it uses until another is
//! chosen.

use sqlx::SqliteConnection;

use super::Collection;

/// The defaults, one per major version.
pub(crate) struct JavaDefaultVersions;

/// What a default carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaDefaultVersionRecord {
    pub path: String,
}

impl Collection for JavaDefaultVersions {
    /// A default is identified by the major version it answers for.
    type Key = u32;
    type Scope = u32;
    type Record = JavaDefaultVersionRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let path: Option<String> = sqlx::query_scalar(
            "
            SELECT path
            FROM java_default_versions
            WHERE major_version = ?
            ",
        )
        .bind(i64::from(*key))
        .fetch_optional(&mut *conn)
        .await?;

        Ok(path.map(|path| Self::Record { path }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM java_default_versions WHERE major_version = ?",
            )
            .bind(i64::from(*key))
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT major_version, path FROM java_default_versions",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().filter_map(to_record).collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        delete_omitted(conn, records).await?;
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
            .map(|record| vec![(*scope, record)])
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

fn to_record(
    (major_version, path): (i64, String),
) -> Option<(u32, JavaDefaultVersionRecord)> {
    let Some(major_version) = u32::try_from(major_version).ok() else {
        tracing::warn!(
            %path,
            major_version,
            "Ignoring a Java default whose major version is out of range"
        );
        return None;
    };

    Some((major_version, JavaDefaultVersionRecord { path }))
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &u32,
    record: &JavaDefaultVersionRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO java_default_versions (major_version, path)
        VALUES (?, ?)
        ON CONFLICT (major_version) DO UPDATE SET
            path = excluded.path
        ",
    )
    .bind(i64::from(*key))
    .bind(&record.path)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the defaults a replacement leaves out, including one whose major
/// version a read would not hand out.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    records: &[(u32, JavaDefaultVersionRecord)],
) -> crate::Result<()> {
    let existing: Vec<i64> =
        sqlx::query_scalar("SELECT major_version FROM java_default_versions")
            .fetch_all(&mut *conn)
            .await?;

    for major_version in existing {
        let kept = u32::try_from(major_version)
            .is_ok_and(|key| records.iter().any(|(kept, _)| *kept == key));
        if kept {
            continue;
        }
        sqlx::query(
            "DELETE FROM java_default_versions WHERE major_version = ?",
        )
        .bind(major_version)
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}
