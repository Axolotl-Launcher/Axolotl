//! Whether a sync feature is on for the launcher and for new instances.

use sqlx::SqliteConnection;

use super::Collection;

/// The feature settings, one per sync feature.
pub(crate) struct SyncFeatureSettings;

/// A feature's settings are identified by the feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeatureSettingsKey {
    pub feature: String,
}

/// What a feature's settings carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FeatureSettingsRecord {
    pub globally_enabled: bool,
    pub new_instance_default: bool,
}

impl Collection for SyncFeatureSettings {
    type Key = FeatureSettingsKey;
    type Scope = FeatureSettingsKey;
    type Record = FeatureSettingsRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(bool, bool)> = sqlx::query_as(
            "
            SELECT globally_enabled, new_instance_default
            FROM sync_feature_settings
            WHERE feature = ?
            ",
        )
        .bind(&key.feature)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(
            row.map(|(globally_enabled, new_instance_default)| Self::Record {
                globally_enabled,
                new_instance_default,
            }),
        )
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM sync_feature_settings WHERE feature = ?")
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
        let rows: Vec<(String, bool, bool)> = sqlx::query_as(
            "
            SELECT feature, globally_enabled, new_instance_default
            FROM sync_feature_settings
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(feature, globally_enabled, new_instance_default)| {
                (
                    FeatureSettingsKey { feature },
                    FeatureSettingsRecord {
                        globally_enabled,
                        new_instance_default,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM sync_feature_settings")
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

async fn write_row(
    conn: &mut SqliteConnection,
    key: &FeatureSettingsKey,
    record: &FeatureSettingsRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO sync_feature_settings
            (feature, globally_enabled, new_instance_default)
        VALUES (?, ?, ?)
        ON CONFLICT (feature) DO UPDATE SET
            globally_enabled = excluded.globally_enabled,
            new_instance_default = excluded.new_instance_default
        ",
    )
    .bind(&key.feature)
    .bind(record.globally_enabled)
    .bind(record.new_instance_default)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
