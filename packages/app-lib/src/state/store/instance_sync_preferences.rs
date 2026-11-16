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
        Ok(Self::get_all(handle)
            .await?
            .into_iter()
            .filter_map(|(key, record)| {
                (key.feature == feature && record.enabled)
                    .then_some(key.instance_id)
            })
            .collect())
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

impl super::configuration_documents::Preference for InstanceSyncPreferences {
    async fn guard(
        unit: &mut super::Atom,
        key: &InstancePreferenceKey,
        record: Option<&InstancePreferenceRecord>,
    ) -> crate::Result<()> {
        if record.is_some() {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM instances WHERE id=?)",
            )
            .bind(&key.instance_id)
            .fetch_one(unit.connection())
            .await?;
            if !exists {
                return Err(crate::ErrorKind::InputError(
                    "This instance has been deleted".into(),
                )
                .into());
            }
        } else {
            super::instance_sync_checkpoints::InstanceSyncCheckpoints::atom_set_scoped(unit,&super::instance_sync_checkpoints::CheckpointScope{instance_id:key.instance_id.clone(),feature:key.feature.clone()},&[]).await?;
        }
        Ok(())
    }
    fn rows(
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> crate::Result<Vec<(InstancePreferenceKey, InstancePreferenceRecord)>>
    {
        let mut result = Vec::new();
        if let Some(instances) =
            data.get("instances").and_then(serde_json::Value::as_object)
        {
            for (instance_id, value) in instances {
                if let Some(features) = value.as_object() {
                    for (feature, value) in features {
                        if let Some(enabled) = value.as_bool() {
                            result.push((
                                InstancePreferenceKey {
                                    instance_id: instance_id.clone(),
                                    feature: feature.clone(),
                                },
                                InstancePreferenceRecord { enabled },
                            ));
                        }
                    }
                }
            }
        }
        Ok(result)
    }
    fn put(
        data: &mut serde_json::Map<String, serde_json::Value>,
        key: &InstancePreferenceKey,
        record: Option<&InstancePreferenceRecord>,
    ) -> crate::Result<()> {
        let instances = super::configuration_documents::map(data, "instances")?;
        let features =
            super::configuration_documents::map(instances, &key.instance_id)?;
        if features
            .get(&key.feature)
            .is_some_and(|value| !value.is_boolean())
        {
            return Err(crate::ErrorKind::InputError(
                "Unknown instance synchronization preference retained".into(),
            )
            .into());
        }
        if let Some(record) = record {
            features
                .insert(key.feature.clone(), serde_json::json!(record.enabled));
        } else {
            features.remove(&key.feature);
        }
        Ok(())
    }
}
super::configuration_documents::preference_verbs!(InstanceSyncPreferences);

/// Seed defaults in the same core decision as instance creation. The file lock
/// remains owned by Atom until publication has finished.
pub(crate) async fn seed_instance_defaults(
    unit: &mut super::Atom,
    instance_id: &str,
) -> crate::Result<()> {
    let defaults =
        super::sync_feature_settings::SyncFeatureSettings::atom_get_all(unit)
            .await?;
    for (key, record) in defaults {
        InstanceSyncPreferences::atom_set(
            unit,
            &InstancePreferenceKey {
                instance_id: instance_id.into(),
                feature: key.feature,
            },
            Some(&InstancePreferenceRecord {
                enabled: record.new_instance_default,
            }),
        )
        .await?;
    }
    Ok(())
}
