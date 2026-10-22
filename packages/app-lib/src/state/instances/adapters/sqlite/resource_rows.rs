use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::resources::{
    ResourceArtifact, ResourceError, ResourceId, ResourceResult, ResourceType,
    SourceId, SourceProjectId, SourceProjectMetadata, SourceProjectRef,
    identity::verify_artifact_pair,
};

#[derive(Clone)]
pub struct ResourceStore {
    pool: SqlitePool,
}

fn database_error(error: sqlx::Error) -> ResourceError {
    ResourceError::Database(error.to_string())
}

pub(crate) fn validate_project(
    project: &SourceProjectRef,
) -> ResourceResult<()> {
    validate_stable_id(&project.source, project.project_id.as_str())
}

fn validate_stable_id(source: &SourceId, id: &str) -> ResourceResult<()> {
    let valid = match source.as_str() {
        "modrinth" => {
            id.len() == 8 && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }
        "curseforge" => id
            .parse::<u32>()
            .ok()
            .is_some_and(|number| number > 0 && number.to_string() == id),
        "mcarchive" => uuid::Uuid::parse_str(id).ok().is_some_and(|value| {
            value.hyphenated().to_string().eq_ignore_ascii_case(id)
        }),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(ResourceError::InvalidRequest(
            "expected a stable source project ID".into(),
        ))
    }
}

fn type_name(value: ResourceType) -> &'static str {
    match value {
        ResourceType::Mod => "mod",
        ResourceType::Plugin => "plugin",
        ResourceType::DataPack => "datapack",
        ResourceType::ResourcePack => "resourcepack",
        ResourceType::Shader => "shader",
        ResourceType::ModPack => "modpack",
        ResourceType::World => "world",
    }
}

impl ResourceStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(crate) async fn installed_artifacts(
        &self,
        instance_id: &str,
    ) -> ResourceResult<std::collections::HashMap<String, ResourceArtifact>>
    {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT exact.entry_id, exact.artifact FROM resource_installed_artifacts exact JOIN instance_content_entries entry ON entry.id = exact.entry_id WHERE entry.instance_id = ?"
        ).bind(instance_id).fetch_all(&self.pool).await.map_err(database_error)?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, value)| {
                serde_json::from_str(&value)
                    .ok()
                    .map(|artifact| (id, artifact))
            })
            .collect())
    }

    pub(crate) async fn project_types(
        &self,
        project: &SourceProjectRef,
    ) -> ResourceResult<Vec<ResourceType>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT content_type FROM resource_source_types WHERE source = ? AND project_id = ?"
        ).bind(project.source.as_str()).bind(project.project_id.as_str())
            .fetch_all(&self.pool).await.map_err(database_error)?;
        Ok(rows
            .into_iter()
            .filter_map(|value| match value.as_str() {
                "mod" => Some(ResourceType::Mod),
                "plugin" => Some(ResourceType::Plugin),
                "datapack" => Some(ResourceType::DataPack),
                "resourcepack" => Some(ResourceType::ResourcePack),
                "shader" => Some(ResourceType::Shader),
                "modpack" => Some(ResourceType::ModPack),
                "world" => Some(ResourceType::World),
                _ => None,
            })
            .collect())
    }

    pub async fn lookup(
        &self,
        project: &SourceProjectRef,
    ) -> ResourceResult<Option<ResourceId>> {
        validate_project(project)?;
        let id: Option<String> = sqlx::query_scalar("SELECT resource_id FROM resource_source_projects WHERE source = ? AND project_id = ?")
            .bind(project.source.as_str()).bind(project.project_id.as_str())
            .fetch_optional(&self.pool).await.map_err(database_error)?;
        id.map(ResourceId::new).transpose().map_err(Into::into)
    }

    pub async fn lookup_many(
        &self,
        projects: &[SourceProjectRef],
    ) -> ResourceResult<std::collections::HashMap<SourceProjectRef, ResourceId>>
    {
        let mut unique = std::collections::HashSet::new();
        for project in projects {
            validate_project(project)?;
            unique.insert(project);
        }
        let projects: Vec<_> = unique.into_iter().collect();
        let mut result = std::collections::HashMap::new();
        for batch in projects.chunks(400) {
            let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "WITH selected(source, project_id) AS (",
            );
            query.push_values(batch, |mut row, project| {
                row.push_bind(project.source.as_str())
                    .push_bind(project.project_id.as_str());
            });
            query.push(") SELECT project.source, project.project_id, project.resource_id FROM resource_source_projects project JOIN selected ON selected.source = project.source AND selected.project_id = project.project_id");
            for row in query
                .build()
                .fetch_all(&self.pool)
                .await
                .map_err(database_error)?
            {
                result.insert(
                    SourceProjectRef {
                        source: SourceId::new(
                            row.try_get::<String, _>("source")
                                .map_err(database_error)?,
                        )?,
                        project_id: SourceProjectId::new(
                            row.try_get::<String, _>("project_id")
                                .map_err(database_error)?,
                        )?,
                    },
                    ResourceId::new(
                        row.try_get::<String, _>("resource_id")
                            .map_err(database_error)?,
                    )?,
                );
            }
        }
        Ok(result)
    }

    /// Resolve a saved ID after any number of confirmed project merges.
    pub async fn resolve(
        &self,
        id: &ResourceId,
    ) -> ResourceResult<Option<ResourceId>> {
        let value: Option<String> = sqlx::query_scalar(
            "SELECT COALESCE(redirect_to, id) FROM resources WHERE id = ?",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(database_error)?;
        value.map(ResourceId::new).transpose().map_err(Into::into)
    }

    pub async fn ensure_project(
        &self,
        project: &SourceProjectRef,
        types: &[ResourceType],
    ) -> ResourceResult<ResourceId> {
        validate_project(project)?;
        let mut transaction =
            self.pool.begin().await.map_err(database_error)?;
        ensure_project(&mut transaction, project, types).await?;
        let id: String = sqlx::query_scalar("SELECT resource_id FROM resource_source_projects WHERE source = ? AND project_id = ?")
            .bind(project.source.as_str()).bind(project.project_id.as_str())
            .fetch_one(&mut *transaction).await.map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        ResourceId::new(id).map_err(Into::into)
    }

    pub async fn projects(
        &self,
        id: &ResourceId,
    ) -> ResourceResult<Vec<SourceProjectRef>> {
        let rows = sqlx::query("SELECT source, project_id FROM resource_source_projects
            WHERE resource_id = (SELECT COALESCE(redirect_to, id) FROM resources WHERE id = ?)
            ORDER BY source, project_id")
            .bind(id.as_str()).fetch_all(&self.pool).await.map_err(database_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(SourceProjectRef {
                    source: SourceId::new(
                        row.try_get::<String, _>("source")
                            .map_err(database_error)?,
                    )?,
                    project_id: SourceProjectId::new(
                        row.try_get::<String, _>("project_id")
                            .map_err(database_error)?,
                    )?,
                })
            })
            .collect()
    }

    /// Aliases are locators with explicit source provenance. Multiple projects
    /// may share an alias, so resolving one never silently chooses a project.
    pub async fn add_alias(
        &self,
        project: &SourceProjectRef,
        alias: &str,
    ) -> ResourceResult<()> {
        validate_project(project)?;
        SourceProjectId::new(alias)?;
        if self.lookup(project).await?.is_none() {
            return Err(ResourceError::InvalidRequest(
                "alias requires a durable project".into(),
            ));
        }
        sqlx::query("INSERT OR IGNORE INTO resource_aliases(source, alias, project_id) VALUES (?, ?, ?)")
            .bind(project.source.as_str()).bind(alias).bind(project.project_id.as_str())
            .execute(&self.pool).await.map_err(database_error)?;
        Ok(())
    }

    pub async fn alias_projects(
        &self,
        source: &SourceId,
        alias: &str,
    ) -> ResourceResult<Vec<SourceProjectRef>> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT project_id FROM resource_aliases WHERE source = ? AND alias = ? ORDER BY project_id")
            .bind(source.as_str()).bind(alias).fetch_all(&self.pool).await.map_err(database_error)?;
        ids.into_iter()
            .map(|id| {
                Ok(SourceProjectRef {
                    source: source.clone(),
                    project_id: SourceProjectId::new(id)?,
                })
            })
            .collect()
    }

    pub async fn project_alias(
        &self,
        project: &SourceProjectRef,
    ) -> ResourceResult<Option<String>> {
        sqlx::query_scalar("SELECT alias FROM resource_aliases WHERE source = ? AND project_id = ? ORDER BY rowid DESC LIMIT 1")
            .bind(project.source.as_str()).bind(project.project_id.as_str())
            .fetch_optional(&self.pool).await.map_err(database_error)
    }

    /// Confirmation retains the complete byte/type evidence in the main DB.
    /// Cache eviction cannot undo a relationship used by business records.
    pub async fn confirm_projects(
        &self,
        left: &SourceProjectMetadata,
        right: &SourceProjectMetadata,
        left_artifact: &ResourceArtifact,
        right_artifact: &ResourceArtifact,
    ) -> ResourceResult<ResourceId> {
        validate_project(&left.project)?;
        validate_project(&right.project)?;
        for artifact in [left_artifact, right_artifact] {
            validate_stable_id(
                &artifact.reference.release.project.source,
                artifact.reference.release.release_id.as_str(),
            )?;
        }
        if left_artifact.reference.release.project != left.project
            || right_artifact.reference.release.project != right.project
            || !left.types.iter().any(|kind| right.types.contains(kind))
            || !crate::resources::valid_artifact(left_artifact)
            || !crate::resources::valid_artifact(right_artifact)
            || verify_artifact_pair(left_artifact, right_artifact).is_none()
        {
            return Err(ResourceError::InvalidRequest("project confirmation requires compatible types and matching primary artifacts".into()));
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Confirmation<'a> {
            version: u32,
            left: &'a SourceProjectRef,
            right: &'a SourceProjectRef,
            left_types: &'a [ResourceType],
            right_types: &'a [ResourceType],
            left_artifact: &'a ResourceArtifact,
            right_artifact: &'a ResourceArtifact,
        }
        let payload = serde_json::to_string(&Confirmation {
            version: 1,
            left: &left.project,
            right: &right.project,
            left_types: &left.types,
            right_types: &right.types,
            left_artifact,
            right_artifact,
        })?;
        use sha2::{Digest, Sha256};
        let mut references = [
            serde_json::to_string(&left_artifact.reference)?,
            serde_json::to_string(&right_artifact.reference)?,
        ];
        references.sort();
        let evidence_id = format!(
            "artifact-pair:{:x}",
            Sha256::digest(serde_json::to_vec(&references)?)
        );
        self.confirm_identity(left, right, &evidence_id, &payload)
            .await
    }

    /// Trust the bundled MCMOD source pair only within one exact subvariant.
    /// This establishes project identity, never equivalence of release bytes.
    pub(crate) async fn confirm_curated_projects(
        &self,
        left: &SourceProjectMetadata,
        right: &SourceProjectMetadata,
    ) -> ResourceResult<ResourceId> {
        let (mr, cf) =
            match (left.project.source.as_str(), right.project.source.as_str())
            {
                ("modrinth", "curseforge") => (left, right),
                ("curseforge", "modrinth") => (right, left),
                _ => return Err(ResourceError::InvalidRequest(
                    "curated identity requires a Modrinth and CurseForge pair"
                        .into(),
                )),
            };
        let (Some(mr_slug), Some(cf_slug)) = (&mr.slug, &cf.slug) else {
            return Err(ResourceError::InvalidRequest(
                "curated identity requires exact project slugs".into(),
            ));
        };
        let lookup = crate::content_search::lookup_content_identities(
            &[mr_slug.clone()],
            &[cf_slug.clone()],
        );
        let record = lookup
            .modrinth
            .get(mr_slug)
            .filter(|records| records.len() == 1)
            .and_then(|records| records.first())
            .filter(|record| {
                lookup.curseforge.get(cf_slug).is_some_and(|records| {
                    records.len() == 1 && records[0].key == record.key
                })
            })
            .ok_or_else(|| {
                ResourceError::InvalidRequest(
                    "projects do not share one MCMOD subvariant".into(),
                )
            })?;
        if !left.types.iter().any(|kind| right.types.contains(kind)) {
            return Err(ResourceError::InvalidRequest(
                "curated project types do not match".into(),
            ));
        }
        let payload = serde_json::to_string(&serde_json::json!({
            "version": 1, "kind": "mcmod-source-pair", "mapping": record.key,
            "left": left.project, "right": right.project,
            "leftSlug": left.slug, "rightSlug": right.slug,
        }))?;
        let evidence_id = format!("mcmod-source-pair:{}", record.key);
        self.confirm_identity(left, right, &evidence_id, &payload)
            .await
    }

    async fn confirm_identity(
        &self,
        left: &SourceProjectMetadata,
        right: &SourceProjectMetadata,
        evidence_id: &str,
        payload: &str,
    ) -> ResourceResult<ResourceId> {
        validate_project(&left.project)?;
        validate_project(&right.project)?;
        let mut transaction =
            self.pool.begin().await.map_err(database_error)?;
        ensure_project(&mut transaction, &left.project, &left.types).await?;
        ensure_project(&mut transaction, &right.project, &right.types).await?;
        let ids: Vec<String> = sqlx::query_scalar("SELECT resource_id FROM resource_source_projects WHERE (source = ? AND project_id = ?) OR (source = ? AND project_id = ?) ORDER BY resource_id")
            .bind(left.project.source.as_str()).bind(left.project.project_id.as_str())
            .bind(right.project.source.as_str()).bind(right.project.project_id.as_str())
            .fetch_all(&mut *transaction).await.map_err(database_error)?;
        let winner = &ids[0];
        for loser in ids.iter().filter(|id| *id != winner) {
            sqlx::query("UPDATE resource_source_projects SET resource_id = ? WHERE resource_id = ?")
                .bind(winner).bind(loser).execute(&mut *transaction).await.map_err(database_error)?;
            sqlx::query("UPDATE resources SET redirect_to = ? WHERE id = ? OR redirect_to = ?")
                .bind(winner).bind(loser).bind(loser).execute(&mut *transaction).await.map_err(database_error)?;
            sqlx::query("UPDATE resource_identity_evidence SET resource_id = ? WHERE resource_id = ?")
                .bind(winner).bind(loser).execute(&mut *transaction).await.map_err(database_error)?;
        }
        sqlx::query("INSERT INTO resource_identity_evidence(evidence_id, resource_id, payload, confirmed_at) VALUES (?, ?, ?, ?) ON CONFLICT(evidence_id) DO UPDATE SET resource_id = excluded.resource_id, payload = excluded.payload, confirmed_at = excluded.confirmed_at WHERE resource_identity_evidence.resource_id <> excluded.resource_id OR resource_identity_evidence.payload <> excluded.payload")
            .bind(evidence_id).bind(winner).bind(payload).bind(chrono::Utc::now().timestamp())
            .execute(&mut *transaction).await.map_err(database_error)?;
        let id = ResourceId::new(winner.clone())?;
        transaction.commit().await.map_err(database_error)?;
        Ok(id)
    }

    pub async fn preferred_source(&self) -> ResourceResult<SourceId> {
        let value: String = sqlx::query_scalar(
            "SELECT preferred_source FROM resource_preferences WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(database_error)?;
        SourceId::new(value).map_err(Into::into)
    }

    pub async fn set_preferred_source(
        &self,
        source: &SourceId,
    ) -> ResourceResult<()> {
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source = ? WHERE id = 1",
        )
        .bind(source.as_str())
        .execute(&self.pool)
        .await
        .map_err(database_error)?;
        Ok(())
    }

    /// Preserve actual origin. A malformed origin cannot fall back to a
    /// different known source; multiple unconfirmed references stay unresolved.
    pub async fn entry_resources(
        &self,
        instance_id: &str,
    ) -> ResourceResult<std::collections::HashMap<String, ResourceId>> {
        let rows = sqlx::query("SELECT entry.id AS entry_id,
            COALESCE(MAX(CASE WHEN ref.is_origin = 1 THEN link.resource_id END), MIN(link.resource_id)) AS resource_id
            FROM instance_content_entries entry
            JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id
            LEFT JOIN resource_entry_links link ON link.reference_id = ref.id
            WHERE entry.instance_id = ?
            GROUP BY entry.id
            HAVING (MAX(ref.is_origin) = 0 AND COUNT(link.resource_id) = COUNT(*) AND COUNT(DISTINCT link.resource_id) = 1)
                OR (MAX(ref.is_origin) = 1 AND SUM(CASE WHEN ref.is_origin = 1 AND link.resource_id IS NOT NULL THEN 1 ELSE 0 END) = 1)")
            .bind(instance_id).fetch_all(&self.pool).await.map_err(database_error)?;
        let mut output = std::collections::HashMap::new();
        for row in rows {
            let entry: String =
                row.try_get("entry_id").map_err(database_error)?;
            let id: String =
                row.try_get("resource_id").map_err(database_error)?;
            output.insert(entry, ResourceId::new(id)?);
        }
        Ok(output)
    }

    pub async fn pack_resources(
        &self,
        instance_id: &str,
    ) -> ResourceResult<std::collections::HashMap<String, ResourceId>> {
        let rows = sqlx::query("SELECT link.member_id, link.resource_id FROM resource_pack_links link
            JOIN instance_pack_members member ON member.id = link.member_id
            JOIN instance_content_sets content_set ON content_set.id = member.content_set_id WHERE content_set.instance_id = ?")
            .bind(instance_id).fetch_all(&self.pool).await.map_err(database_error)?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    row.try_get("member_id").map_err(database_error)?,
                    ResourceId::new(
                        row.try_get::<String, _>("resource_id")
                            .map_err(database_error)?,
                    )?,
                ))
            })
            .collect()
    }

    pub async fn instance_pack_resource(
        &self,
        instance_id: &str,
    ) -> ResourceResult<Option<ResourceId>> {
        let id: Option<String> = sqlx::query_scalar("SELECT resource_id FROM resource_instance_pack_links WHERE instance_id = ?")
            .bind(instance_id).fetch_optional(&self.pool).await.map_err(database_error)?;
        id.map(ResourceId::new).transpose().map_err(Into::into)
    }
}

async fn ensure_project(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project: &SourceProjectRef,
    types: &[ResourceType],
) -> ResourceResult<()> {
    sqlx::query("INSERT OR IGNORE INTO resource_source_projects(source, project_id) VALUES (?, ?)")
        .bind(project.source.as_str()).bind(project.project_id.as_str())
        .execute(&mut **transaction).await.map_err(database_error)?;
    sqlx::query("INSERT OR IGNORE INTO resources(id) SELECT resource_id FROM resource_source_projects WHERE source = ? AND project_id = ?")
        .bind(project.source.as_str()).bind(project.project_id.as_str())
        .execute(&mut **transaction).await.map_err(database_error)?;
    for kind in types {
        sqlx::query("INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type) VALUES (?, ?, ?)")
            .bind(project.source.as_str()).bind(project.project_id.as_str()).bind(type_name(*kind))
            .execute(&mut **transaction).await.map_err(database_error)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "resource_rows_tests.rs"]
mod tests;
