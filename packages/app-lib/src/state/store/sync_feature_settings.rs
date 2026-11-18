//! Whether a sync feature is on for the launcher and for new instances.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

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

impl SyncFeatureSettings {
    /// Turns `feature` on or off for the launcher, and for every instance with
    /// it: an instance without a preference row would otherwise keep the old
    /// answer forever, because there is no row to update.
    pub(crate) async fn set_globally_enabled(
        handle: &Handle,
        feature: &str,
        enabled: bool,
    ) -> crate::Result<()> {
        let mut unit = super::atom(handle).await?;
        Self::atom_set(
            &mut unit,
            &FeatureSettingsKey {
                feature: feature.to_string(),
            },
            Some(&FeatureSettingsRecord {
                globally_enabled: enabled,
                new_instance_default: enabled,
            }),
        )
        .await?;
        let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM instances")
            .fetch_all(unit.connection())
            .await?;
        for instance_id in ids {
            super::instance_sync_preferences::InstanceSyncPreferences::atom_set(&mut unit,&super::instance_sync_preferences::InstancePreferenceKey{instance_id,feature:feature.into()},Some(&super::instance_sync_preferences::InstancePreferenceRecord{enabled})).await?;
        }
        unit.commit().await
    }

    /// Whether `feature` is on for the launcher.
    pub(crate) async fn globally_enabled(
        handle: &Handle,
        feature: &str,
    ) -> crate::Result<bool> {
        Ok(Self::get(
            handle,
            &FeatureSettingsKey {
                feature: feature.to_string(),
            },
        )
        .await?
        .is_some_and(|record| record.globally_enabled))
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

impl super::configuration_documents::Preference for SyncFeatureSettings {
    fn rows(
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> crate::Result<Vec<(FeatureSettingsKey, FeatureSettingsRecord)>> {
        let mut result = Vec::new();
        if let Some(features) =
            data.get("features").and_then(serde_json::Value::as_object)
        {
            for (feature, value) in features {
                if let (Some(globally_enabled), Some(new_instance_default)) = (
                    value["globally_enabled"].as_bool(),
                    value["new_instance_default"].as_bool(),
                ) {
                    result.push((
                        FeatureSettingsKey {
                            feature: feature.clone(),
                        },
                        FeatureSettingsRecord {
                            globally_enabled,
                            new_instance_default,
                        },
                    ));
                }
            }
        }
        for feature in [
            "game_options",
            "command_history",
            "multiplayer_servers",
            "creative_hotbars",
            "screenshots",
            "resource_packs",
            "data_packs",
        ] {
            if !result.iter().any(|(key, _)| key.feature == feature) {
                result.push((
                    FeatureSettingsKey {
                        feature: feature.into(),
                    },
                    FeatureSettingsRecord {
                        globally_enabled: false,
                        new_instance_default: false,
                    },
                ));
            }
        }
        Ok(result)
    }
    fn put(
        data: &mut serde_json::Map<String, serde_json::Value>,
        key: &FeatureSettingsKey,
        record: Option<&FeatureSettingsRecord>,
    ) -> crate::Result<()> {
        let features = super::configuration_documents::map(data, "features")?;
        let entry =
            super::configuration_documents::map(features, &key.feature)?;
        for name in ["globally_enabled", "new_instance_default"] {
            if entry.get(name).is_some_and(|value| !value.is_boolean()) {
                return Err(crate::ErrorKind::InputError(
                    "Unknown synchronization preference retained".into(),
                )
                .into());
            }
        }
        if entry.contains_key("protocol")
            || entry.contains_key("schema_version")
        {
            return Err(crate::ErrorKind::InputError(
                "Newer synchronization preference retained".into(),
            )
            .into());
        }
        if let Some(record) = record {
            entry.insert(
                "globally_enabled".into(),
                serde_json::json!(record.globally_enabled),
            );
            entry.insert(
                "new_instance_default".into(),
                serde_json::json!(record.new_instance_default),
            );
        } else {
            entry.remove("globally_enabled");
            entry.remove("new_instance_default");
        }
        Ok(())
    }
}
super::configuration_documents::preference_verbs!(SyncFeatureSettings);
