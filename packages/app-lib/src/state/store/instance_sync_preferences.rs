//! Whether one instance takes part in one sync feature.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The preferences, one per instance and feature.
pub(crate) struct InstanceSyncPreferences;

/// A preference is identified by the instance and the feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstancePreferenceKey {
    pub instance_id: String,
    pub feature: String,
}

/// What a preference carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InstancePreferenceRecord {
    pub enabled: bool,
}

impl Collection for InstanceSyncPreferences {
    type Key = InstancePreferenceKey;
    type Scope = String;
    type Record = InstancePreferenceRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        key.instance_id == *scope
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<bool> = sqlx::query_scalar(
            "
            SELECT enabled
            FROM instance_sync_preferences
            WHERE instance_id = ? AND feature = ?
            ",
        )
        .bind(&key.instance_id)
        .bind(&key.feature)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|enabled| Self::Record { enabled }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM instance_sync_preferences
                 WHERE instance_id = ? AND feature = ?",
            )
            .bind(&key.instance_id)
            .bind(&key.feature)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, bool)> = sqlx::query_as(
            "
            SELECT instance_id, feature, enabled
            FROM instance_sync_preferences
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(instance_id, feature, enabled)| {
                (
                    InstancePreferenceKey {
                        instance_id,
                        feature,
                    },
                    InstancePreferenceRecord { enabled },
                )
            })
            .collect())
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
        let rows: Vec<(String, bool)> = sqlx::query_as(
            "
            SELECT feature, enabled
            FROM instance_sync_preferences
            WHERE instance_id = ?
            ",
        )
        .bind(scope)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(feature, enabled)| {
                (
                    InstancePreferenceKey {
                        instance_id: scope.clone(),
                        feature,
                    },
                    InstancePreferenceRecord { enabled },
                )
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        delete_omitted(conn, Some(scope), records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

impl InstanceSyncPreferences {
    /// The instances that take part in `feature`.
    pub(crate) async fn instances_with_feature_enabled(
        handle: &Handle,
        feature: &str,
    ) -> crate::Result<Vec<String>> {
        let mut connection = handle.connection().await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "
            SELECT instance_id
            FROM instance_sync_preferences
            WHERE feature = ? AND enabled = 1
            ",
        )
        .bind(feature)
        .fetch_all(&mut *connection)
        .await?;

        Ok(rows)
    }
}

/// Turns `feature` on or off for every instance at once, adding a row to the
/// instances that had none.
pub(in crate::state::store) async fn set_feature_for_every_instance(
    conn: &mut SqliteConnection,
    feature: &str,
    enabled: bool,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
         SELECT id, ?, ? FROM instances
         WHERE 1
         ON CONFLICT(instance_id, feature) DO UPDATE SET enabled = excluded.enabled",
    )
    .bind(feature)
    .bind(enabled)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &InstancePreferenceKey,
    record: &InstancePreferenceRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
        VALUES (?, ?, ?)
        ON CONFLICT (instance_id, feature) DO UPDATE SET
            enabled = excluded.enabled
        ",
    )
    .bind(&key.instance_id)
    .bind(&key.feature)
    .bind(record.enabled)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the preferences the replacement leaves out, and only those: a row
/// the replacement keeps has to survive, because the checkpoints of that
/// instance and feature hang off it.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<&String>,
    records: &[(InstancePreferenceKey, InstancePreferenceRecord)],
) -> crate::Result<()> {
    let existing: Vec<(String, String)> =
        match scope {
            Some(instance_id) => {
                sqlx::query_as(
                    "SELECT instance_id, feature
             FROM instance_sync_preferences
             WHERE instance_id = ?",
                )
                .bind(instance_id)
                .fetch_all(&mut *conn)
                .await?
            }
            None => sqlx::query_as(
                "SELECT instance_id, feature FROM instance_sync_preferences",
            )
            .fetch_all(&mut *conn)
            .await?,
        };

    for (instance_id, feature) in existing {
        let kept = records.iter().any(|(key, _)| {
            key.instance_id == instance_id && key.feature == feature
        });
        if kept {
            continue;
        }
        sqlx::query(
            "DELETE FROM instance_sync_preferences
             WHERE instance_id = ? AND feature = ?",
        )
        .bind(&instance_id)
        .bind(&feature)
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}
