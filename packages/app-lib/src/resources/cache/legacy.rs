use chrono::Utc;
use serde_json::Value;
use sqlx::{Row, SqlitePool};

use super::{CacheKey, ResourceCache, cache_error};
use crate::resources::{
    providers::{ProviderQuery, ProviderValue, legacy},
    service::validate_value,
    *,
};

const PAGE_SIZE: i64 = 25;
const MAX_LEGACY_PAYLOAD_BYTES: i64 = 1024 * 1024;
const LEGACY_TTL: i64 = 100 * 365 * 24 * 60 * 60;

impl ResourceCache {
    /// A bounded, resumable, network-free copy of historical resource rows.
    /// Runtime resource queries never write to the original cache table.
    pub async fn import_legacy(&self, main: &SqlitePool) -> ResourceResult<()> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let generation = self.generation().await;
        let high_water: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(rowid), 0) FROM cache")
                .fetch_one(main)
                .await
                .map_err(cache_error)?;
        loop {
            let checkpoint = sqlx::query("SELECT last_rowid, completed FROM resource_legacy_import WHERE id = 1")
                .fetch_one(pool).await.map_err(cache_error)?;
            if checkpoint
                .try_get::<bool, _>("completed")
                .map_err(cache_error)?
                || self.generation().await != generation
            {
                return Ok(());
            }
            let last: i64 =
                checkpoint.try_get("last_rowid").map_err(cache_error)?;
            let rows = sqlx::query("SELECT rowid, id, data_type,
                CASE WHEN data_type IN ('project', 'version', 'curseforge_project', 'modpack_files') AND length(CAST(data AS BLOB)) <= ? THEN data END AS data,
                expires FROM cache WHERE rowid > ? AND rowid <= ? ORDER BY rowid LIMIT ?")
                .bind(MAX_LEGACY_PAYLOAD_BYTES).bind(last).bind(high_water).bind(PAGE_SIZE)
                .fetch_all(main).await.map_err(cache_error)?;
            let mut pending = Vec::new();
            let mut cursor = last;
            for row in &rows {
                cursor = row.try_get("rowid").map_err(cache_error)?;
                let Ok(data) = row.try_get::<String, _>("data") else {
                    continue;
                };
                let Ok(payload) = serde_json::from_str::<Value>(&data) else {
                    continue;
                };
                let data_type: String =
                    row.try_get("data_type").map_err(cache_error)?;
                let id: String = row.try_get("id").map_err(cache_error)?;
                let expires: i64 =
                    row.try_get("expires").map_err(cache_error)?;
                if data_type == "modpack_files" {
                    if let Ok(index) = serde_json::from_value::<
                        crate::state::CachedModpackFiles,
                    >(payload.clone())
                        && index.version_id == id
                        && crate::resources::packs::valid_index(&index)
                    {
                        pending.push((
                            crate::resources::packs::index_key(&id)?,
                            serde_json::to_string(&ProviderValue::Protocol(
                                serde_json::to_value(index)?,
                            ))?,
                            0,
                            expires,
                        ));
                    }
                    continue;
                }
                let retrieved = expires.saturating_sub(LEGACY_TTL);
                if retrieved < 0 || retrieved > Utc::now().timestamp() {
                    continue;
                }
                let values = project_values(&data_type, &id, payload);
                for (namespace, query, value) in values {
                    let source = match &value {
                        ProviderValue::Project(document) => {
                            &document.metadata.project.source
                        }
                        ProviderValue::Release(document) => {
                            &document.release.reference.project.source
                        }
                        ProviderValue::Description(text)
                        | ProviderValue::Changelog(text) => &text.source,
                        _ => continue,
                    };
                    if validate_value(source, &query, &value).is_err() {
                        continue;
                    }
                    let key = CacheKey::new(
                        namespace.into(),
                        source.clone(),
                        &query.cache_identity(),
                    )?;
                    let expiry = retrieved
                        .saturating_add(query.ttl(&value))
                        .min(expires);
                    pending.push((
                        key,
                        serde_json::to_string(&value)?,
                        retrieved,
                        expiry,
                    ));
                }
            }
            let _guard = self.gate.read().await;
            let mut transaction = pool.begin().await.map_err(cache_error)?;
            for (key, payload, retrieved, expiry) in pending {
                self.import_value(
                    &key,
                    &payload,
                    retrieved,
                    expiry,
                    generation,
                    &mut transaction,
                )
                .await?;
            }
            sqlx::query("UPDATE resource_legacy_import SET last_rowid = MAX(last_rowid, ?), completed = ? WHERE id = 1 AND completed = 0 AND EXISTS (SELECT 1 FROM resource_cache_state WHERE id = 1 AND generation = ?)")
                .bind(cursor).bind(rows.len() < PAGE_SIZE as usize).bind(generation)
                .execute(&mut *transaction).await.map_err(cache_error)?;
            transaction.commit().await.map_err(cache_error)?;
            drop(_guard);
            tokio::task::yield_now().await;
        }
    }

    async fn import_value(
        &self,
        key: &CacheKey,
        payload: &str,
        retrieved: i64,
        expires: i64,
        generation: i64,
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> ResourceResult<()> {
        sqlx::query("INSERT OR IGNORE INTO resource_cache_entries(namespace, source, query_key, value_version, payload, retrieved_at, expires_at)
            SELECT ?, ?, ?, ?, ?, ?, ? FROM resource_cache_state WHERE id = 1 AND generation = ? AND EXISTS (SELECT 1 FROM resource_legacy_import WHERE id = 1 AND completed = 0)")
            .bind(&key.namespace).bind(key.source.as_str()).bind(&key.query)
            .bind(i64::from(RESOURCE_CONTRACT_VERSION)).bind(payload)
            .bind(retrieved.saturating_mul(1_000_000)).bind(expires).bind(generation)
            .execute(&mut **transaction).await.map_err(cache_error)?;
        Ok(())
    }
}

fn project_values(
    data_type: &str,
    id: &str,
    payload: Value,
) -> Vec<(&'static str, ProviderQuery, ProviderValue)> {
    let mut output = vec![];
    match data_type {
        "project" | "curseforge_project" => {
            let Ok(document) = legacy::project(data_type, payload.clone())
            else {
                return output;
            };
            if document.metadata.project.project_id.as_str() != id {
                return output;
            }
            let project_id = document.metadata.project.project_id.clone();
            let namespace = if data_type == "project" {
                "modrinth-v2-dates"
            } else {
                "curseforge-v1-dates"
            };
            if data_type == "project" {
                output.push((
                    namespace,
                    ProviderQuery::Description {
                        id: project_id.clone(),
                        slug: None,
                    },
                    ProviderValue::Description(document.description.clone()),
                ));
            }
            // The old public-team cache discarded accepted membership status.
            // A missing author projection must not masquerade as a complete project.
            if data_type != "project"
                || payload["team"].as_str().is_none_or(str::is_empty)
            {
                output.push((
                    namespace,
                    ProviderQuery::Project {
                        id: project_id,
                        slug: None,
                    },
                    ProviderValue::Project(document),
                ));
            }
            if data_type == "curseforge_project" {
                for file in
                    payload["latestFiles"].as_array().into_iter().flatten()
                {
                    if let Ok(release) = legacy::curseforge_file(file.clone())
                        && release.release.reference.project.project_id.as_str()
                            == id
                    {
                        output.extend(release_values(namespace, release));
                    }
                }
            }
        }
        "version" => {
            let Ok(document) = legacy::release(payload) else {
                return output;
            };
            if document.release.reference.release_id.as_str() != id {
                return output;
            }
            output.extend(release_values("modrinth-v2-dates", document));
        }
        _ => {}
    }
    output
}

fn release_values(
    namespace: &'static str,
    document: crate::resources::providers::ReleaseDocument,
) -> Vec<(&'static str, ProviderQuery, ProviderValue)> {
    let reference = &document.release.reference;
    let project_id = reference.project.project_id.clone();
    let release_id = reference.release_id.clone();
    let mut output = vec![];
    if let Some(text) = &document.changelog {
        output.push((
            namespace,
            ProviderQuery::Changelog {
                project_id: project_id.clone(),
                release_id: release_id.clone(),
                slug: None,
            },
            ProviderValue::Changelog(text.clone()),
        ));
    }
    // Both cache keys are used by exact-reference and ID-only legacy callers.
    for project_id in [None, Some(project_id)] {
        output.push((
            namespace,
            ProviderQuery::Release {
                project_id,
                release_id: release_id.clone(),
                slug: None,
            },
            ProviderValue::Release(document.clone()),
        ));
    }
    output
}
