//! The `options.txt` an instance had before a modpack update replaced it.

use super::{Collection, Handle};

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
    type Record = UpdateStateRecord;

    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM instance_game_option_update_state
                 WHERE instance_id = ?",
            )
            .bind(&key.instance_id)
            .execute(handle.pool())
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
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    async fn get(
        handle: &Handle,
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
            .fetch_optional(handle.pool())
            .await?;

        Ok(row.and_then(|(had_file, sha1, document)| {
            to_record(&key.instance_id, had_file, sha1, document)
        }))
    }

    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let mut transaction = handle.pool().begin().await?;
        // The call replaces the collection, so a key the caller leaves out is
        // gone rather than left behind for a later read to return.
        sqlx::query("DELETE FROM instance_game_option_update_state")
            .execute(&mut *transaction)
            .await?;
        for (key, record) in records {
            let (had_file, sha1, document) = columns(record);
            sqlx::query(
                "
                INSERT INTO instance_game_option_update_state
                    (instance_id, had_file, sha1, document)
                VALUES (?, ?, ?, ?)
                ",
            )
            .bind(&key.instance_id)
            .bind(had_file)
            .bind(sha1)
            .bind(document)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, bool, Option<String>, Option<Vec<u8>>)> =
            sqlx::query_as(
                "
                SELECT instance_id, had_file, sha1, document
                FROM instance_game_option_update_state
                ",
            )
            .fetch_all(handle.pool())
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, had_file, sha1, document)| {
                let record = to_record(&instance_id, had_file, sha1, document)?;
                Some((UpdateStateKey { instance_id }, record))
            })
            .collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn handle(instances: &[&str]) -> Handle {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        for instance in instances {
            sqlx::query(
                "INSERT INTO instances
                    (id, path, install_stage, launcher_feature_version,
                     name, created, modified)
                 VALUES (?, ?, 'installed', '1', ?, 1, 1)",
            )
            .bind(instance)
            .bind(instance)
            .bind(instance)
            .execute(&pool)
            .await
            .unwrap();
        }
        Handle::new(pool)
    }

    fn key(instance_id: &str) -> UpdateStateKey {
        UpdateStateKey {
            instance_id: instance_id.to_string(),
        }
    }

    fn replaced() -> UpdateStateRecord {
        UpdateStateRecord::Replaced {
            sha1: "abc".to_string(),
            document: b"options".to_vec(),
        }
    }

    #[tokio::test]
    async fn an_update_state_round_trips_and_clears() {
        let handle = handle(&["instance"]).await;

        GameOptionUpdateState::set(
            &handle,
            &key("instance"),
            Some(&replaced()),
        )
        .await
        .unwrap();
        assert_eq!(
            GameOptionUpdateState::get(&handle, &key("instance"))
                .await
                .unwrap(),
            Some(replaced())
        );

        GameOptionUpdateState::set(
            &handle,
            &key("instance"),
            Some(&UpdateStateRecord::Absent),
        )
        .await
        .unwrap();
        assert_eq!(
            GameOptionUpdateState::get(&handle, &key("instance"))
                .await
                .unwrap(),
            Some(UpdateStateRecord::Absent)
        );

        GameOptionUpdateState::set(&handle, &key("instance"), None)
            .await
            .unwrap();
        assert_eq!(
            GameOptionUpdateState::get(&handle, &key("instance"))
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn writing_a_whole_collection_drops_what_it_leaves_out() {
        let handle = handle(&["kept", "dropped"]).await;
        GameOptionUpdateState::set(&handle, &key("kept"), Some(&replaced()))
            .await
            .unwrap();
        GameOptionUpdateState::set(&handle, &key("dropped"), Some(&replaced()))
            .await
            .unwrap();

        GameOptionUpdateState::set_all(&handle, &[(key("kept"), replaced())])
            .await
            .unwrap();

        assert_eq!(
            GameOptionUpdateState::get_all(&handle).await.unwrap(),
            vec![(key("kept"), replaced())]
        );
    }

    #[tokio::test]
    async fn a_state_that_lost_its_document_is_ignored() {
        let handle = handle(&["instance"]).await;
        sqlx::query(
            "INSERT INTO instance_game_option_update_state
                (instance_id, had_file, sha1, document)
             VALUES ('instance', 1, 'abc', NULL)",
        )
        .execute(handle.pool())
        .await
        .unwrap();

        assert_eq!(
            GameOptionUpdateState::get(&handle, &key("instance"))
                .await
                .unwrap(),
            None
        );
        assert!(
            GameOptionUpdateState::get_all(&handle)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
