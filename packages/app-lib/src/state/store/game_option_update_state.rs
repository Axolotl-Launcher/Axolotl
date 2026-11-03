//! The `options.txt` an instance had before a modpack update replaced it.

use sqlx::SqliteConnection;

use super::Collection;

/// The update state, one per instance.
pub(crate) struct GameOptionUpdateState;

/// An instance's update state is identified by the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateStateKey {
    pub instance_id: String,
}

/// What the instance's `options.txt` looked like before the update.
///
/// A file the instance had is kept together with its bytes, so a state cannot
/// claim one without them: restoring the previous options needs the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateStateRecord {
    Replaced { sha1: String, document: Vec<u8> },
    Absent,
}

impl Collection for GameOptionUpdateState {
    type Key = UpdateStateKey;
    type Scope = UpdateStateKey;
    type Record = UpdateStateRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(bool, Option<String>, Option<Vec<u8>>)> =
            sqlx::query_as(
                "
                SELECT had_file, sha1, document
                FROM instance_game_option_update_state
                WHERE instance_id = ?
                ",
            )
            .bind(&key.instance_id)
            .fetch_optional(&mut *conn)
            .await?;

        Ok(row.and_then(|(had_file, sha1, document)| {
            to_record(&key.instance_id, had_file, sha1, document)
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM instance_game_option_update_state
                 WHERE instance_id = ?",
            )
            .bind(&key.instance_id)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        let (had_file, sha1, document) = columns(record);
        sqlx::query(
            "
            INSERT INTO instance_game_option_update_state
                (instance_id, had_file, sha1, document)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (instance_id) DO UPDATE SET
                had_file = excluded.had_file,
                sha1 = excluded.sha1,
                document = excluded.document
            ",
        )
        .bind(&key.instance_id)
        .bind(had_file)
        .bind(sha1)
        .bind(document)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, bool, Option<String>, Option<Vec<u8>>)> =
            sqlx::query_as(
                "
                SELECT instance_id, had_file, sha1, document
                FROM instance_game_option_update_state
                ",
            )
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, had_file, sha1, document)| {
                let record = to_record(&instance_id, had_file, sha1, document)?;
                Some((UpdateStateKey { instance_id }, record))
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM instance_game_option_update_state")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
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

fn columns(
    record: &UpdateStateRecord,
) -> (bool, Option<&String>, Option<&Vec<u8>>) {
    match record {
        UpdateStateRecord::Replaced { sha1, document } => {
            (true, Some(sha1), Some(document))
        }
        UpdateStateRecord::Absent => (false, None, None),
    }
}

/// The record a row describes, or nothing when it claims a file without
/// keeping its bytes. Such a row cannot restore what it promises, and reading
/// it as an empty state would delete the file the player had.
fn to_record(
    instance_id: &str,
    had_file: bool,
    sha1: Option<String>,
    document: Option<Vec<u8>>,
) -> Option<UpdateStateRecord> {
    match (had_file, sha1, document) {
        (false, _, _) => Some(UpdateStateRecord::Absent),
        (true, Some(sha1), Some(document)) => {
            Some(UpdateStateRecord::Replaced { sha1, document })
        }
        (true, _, _) => {
            tracing::warn!(
                %instance_id,
                "Ignoring an update state that lost the document it kept"
            );
            None
        }
    }
}
