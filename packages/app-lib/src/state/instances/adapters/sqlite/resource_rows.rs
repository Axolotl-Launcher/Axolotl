use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

use crate::resources::{
    ResourceArtifact, ResourceError, ResourceId, ResourceResult, ResourceType,
    SourceId, SourceProjectId, SourceProjectMetadata, SourceProjectRef,
    identity::project_confirmation_rejection,
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

pub(crate) fn project_key(
    source: &str,
    project_id: &str,
) -> Option<SourceProjectRef> {
    let project = SourceProjectRef {
        source: SourceId::new(source).ok()?,
        project_id: SourceProjectId::new(project_id).ok()?,
    };
    validate_project(&project).ok()?;
    Some(project)
}

pub(crate) type EntryProject = (String, String, String, bool);

pub(crate) fn entry_identities(
    entries: &[EntryProject],
    identities: &HashMap<SourceProjectRef, ResourceId>,
) -> HashMap<String, ResourceId> {
    let mut grouped = HashMap::<&str, Vec<(bool, Option<&ResourceId>)>>::new();
    for (entry, source, project_id, is_origin) in entries {
        let identity = project_key(source, project_id)
            .as_ref()
            .and_then(|project| identities.get(project));
        grouped
            .entry(entry)
            .or_default()
            .push((*is_origin, identity));
    }
    grouped
        .into_iter()
        .filter_map(|(entry, refs)| {
            let mut origins = refs.iter().filter(|(is_origin, _)| *is_origin);
            let identity = match origins.next() {
                Some((_, identity)) if origins.next().is_none() => (*identity)?,
                Some(_) => return None,
                None => {
                    let identity = refs.first()?.1?;
                    if !refs.iter().all(|(_, id)| *id == Some(identity)) {
                        return None;
                    }
                    identity
                }
            };
            Some((entry.to_owned(), identity.clone()))
        })
        .collect()
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

    async fn identities(&self) -> ResourceResult<SqlitePool> {
        crate::state::store::resource_identity_db::pool_for(&self.pool)
            .await
            .map_err(|error| ResourceError::Database(error.to_string()))
    }

    async fn writable_identities(&self) -> ResourceResult<SqlitePool> {
        crate::state::store::recovery_mode::ensure_writable(&self.pool)
            .map_err(|error| ResourceError::Database(error.to_string()))?;
        self.identities().await
    }

    pub(crate) async fn identity_available(&self) -> bool {
        match self.identities().await {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!(%error, "Resource identity lookup unavailable");
                false
            }
        }
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
            .fetch_all(&self.identities().await?).await.map_err(database_error)?;
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
            .fetch_optional(&self.identities().await?).await.map_err(database_error)?;
        id.map(ResourceId::new).transpose().map_err(Into::into)
    }

    pub async fn lookup_many(
        &self,
        projects: &[SourceProjectRef],
    ) -> ResourceResult<std::collections::HashMap<SourceProjectRef, ResourceId>>
    {
        if projects.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let pool = self.identities().await?;
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
                .fetch_all(&pool)
                .await
                .map_err(database_error)?
            {
                let raw_id: String =
                    row.try_get("resource_id").map_err(database_error)?;
                let id = match ResourceId::new(raw_id) {
                    Ok(id) => id,
                    Err(error) => {
                        tracing::warn!(%error, "Skipping an unsupported resource identity");
                        continue;
                    }
                };
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
                    id,
                );
            }
        }
        Ok(result)
    }

    pub(crate) async fn enrich_many(
        &self,
        projects: &[SourceProjectRef],
    ) -> std::collections::HashMap<SourceProjectRef, ResourceId> {
        match self.lookup_many(projects).await {
            Ok(known) => known,
            Err(error) => {
                tracing::warn!(%error, "Continuing without optional resource identities");
                std::collections::HashMap::new()
            }
        }
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
        .fetch_optional(&self.identities().await?)
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
        let pool = self.writable_identities().await?;
        let (mut transaction, transaction_diagnostics) =
            crate::state::store::lock_diagnostics::begin(
                &pool,
                "BEGIN IMMEDIATE",
            )
            .await
            .map_err(database_error)?;
        ensure_project(&mut transaction, project, types).await?;
        let id: String = sqlx::query_scalar("SELECT resource_id FROM resource_source_projects WHERE source = ? AND project_id = ?")
            .bind(project.source.as_str()).bind(project.project_id.as_str())
            .fetch_one(&mut *transaction).await.map_err(database_error)?;
        transaction_diagnostics
            .observe("commit_wait", transaction.commit())
            .await
            .map_err(database_error)?;
        transaction_diagnostics.complete("committed");
        ResourceId::new(id).map_err(Into::into)
    }

    pub async fn projects(
        &self,
        id: &ResourceId,
    ) -> ResourceResult<Vec<SourceProjectRef>> {
        let rows = sqlx::query("SELECT source, project_id FROM resource_source_projects
            WHERE resource_id = (SELECT COALESCE(redirect_to, id) FROM resources WHERE id = ?)
            ORDER BY source, project_id")
            .bind(id.as_str()).fetch_all(&self.identities().await?).await.map_err(database_error)?;
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
        let pool = self.writable_identities().await?;
        if self.lookup(project).await?.is_none() {
            return Err(ResourceError::InvalidRequest(
                "alias requires a durable project".into(),
            ));
        }
        sqlx::query("INSERT OR IGNORE INTO resource_aliases(source, alias, project_id) VALUES (?, ?, ?)")
            .bind(project.source.as_str()).bind(alias).bind(project.project_id.as_str())
            .execute(&pool).await.map_err(database_error)?;
        Ok(())
    }

    pub async fn alias_projects(
        &self,
        source: &SourceId,
        alias: &str,
    ) -> ResourceResult<Vec<SourceProjectRef>> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT project_id FROM resource_aliases WHERE source = ? AND alias = ? ORDER BY project_id")
            .bind(source.as_str()).bind(alias).fetch_all(&self.identities().await?).await.map_err(database_error)?;
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
            .fetch_optional(&self.identities().await?).await.map_err(database_error)
    }

    /// Validate the artifact pair before publishing project identity.
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
        if project_confirmation_rejection(
            left,
            right,
            left_artifact,
            right_artifact,
        )
        .is_some()
        {
            return Err(ResourceError::InvalidRequest("project confirmation requires compatible types and matching primary artifacts".into()));
        }
        self.confirm_identity(left, right).await
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
        lookup
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
        self.confirm_identity(left, right).await
    }

    async fn confirm_identity(
        &self,
        left: &SourceProjectMetadata,
        right: &SourceProjectMetadata,
    ) -> ResourceResult<ResourceId> {
        validate_project(&left.project)?;
        validate_project(&right.project)?;
        let pool = self.writable_identities().await?;
        let (mut transaction, transaction_diagnostics) =
            crate::state::store::lock_diagnostics::begin(
                &pool,
                "BEGIN IMMEDIATE",
            )
            .await
            .map_err(database_error)?;
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
        }
        let id = ResourceId::new(winner.clone())?;
        transaction_diagnostics
            .observe("commit_wait", transaction.commit())
            .await
            .map_err(database_error)?;
        transaction_diagnostics.complete("committed");
        Ok(id)
    }

    pub async fn preferred_source(&self) -> ResourceResult<SourceId> {
        if crate::state::store::configuration_documents::runtime(&self.pool) {
            let value =
                crate::state::store::configuration_documents::preferred_source(
                    &crate::state::store::Handle::new(self.pool.clone()),
                )
                .await
                .map_err(|error| {
                    database_error(sqlx::Error::Protocol(error.to_string()))
                })?;
            return SourceId::new(value).map_err(Into::into);
        }
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
        if crate::state::store::configuration_documents::runtime(&self.pool) {
            return crate::state::store::configuration_documents::set_preferred_source(&crate::state::store::Handle::new(self.pool.clone()),source.as_str()).await.map_err(|error|database_error(sqlx::Error::Protocol(error.to_string())));
        }
        let (mut transaction, transaction_diagnostics) =
            crate::state::store::lock_diagnostics::begin(
                &self.pool,
                "BEGIN IMMEDIATE",
            )
            .await
            .map_err(database_error)?;
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source = ? WHERE id = 1",
        )
        .bind(source.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
        crate::state::store::configuration::commit(transaction, &self.pool)
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
        let rows: Vec<EntryProject> = sqlx::query_as(
            "SELECT entry.id, ref.provider, ref.provider_project_id, ref.is_origin
            FROM instance_content_entries entry
            JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id
            WHERE entry.instance_id = ?")
            .bind(instance_id).fetch_all(&self.pool).await.map_err(database_error)?;
        let projects = rows
            .iter()
            .filter_map(|(_, source, id, _)| project_key(source, id))
            .collect::<Vec<_>>();
        let identities = self.enrich_many(&projects).await;
        Ok(entry_identities(&rows, &identities))
    }

    pub async fn pack_resources(
        &self,
        instance_id: &str,
    ) -> ResourceResult<std::collections::HashMap<String, ResourceId>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT member.id, member.provider, member.provider_project_id
            FROM instance_pack_members member JOIN instance_content_sets content_set ON content_set.id = member.content_set_id
            WHERE content_set.instance_id = ? AND member.provider IS NOT NULL AND member.provider_project_id IS NOT NULL")
            .bind(instance_id).fetch_all(&self.pool).await.map_err(database_error)?;
        let projects = rows
            .into_iter()
            .filter_map(|(member, source, id)| {
                Some((member, project_key(&source, &id)?))
            })
            .collect::<Vec<_>>();
        let keys = projects
            .iter()
            .map(|(_, project)| project.clone())
            .collect::<Vec<_>>();
        let identities = self.enrich_many(&keys).await;
        Ok(projects
            .into_iter()
            .filter_map(|(member, project)| {
                Some((member, identities.get(&project)?.clone()))
            })
            .collect())
    }

    pub async fn instance_pack_resource(
        &self,
        instance_id: &str,
    ) -> ResourceResult<Option<ResourceId>> {
        let row: Option<(String, Option<String>)> = sqlx::query_as("SELECT source, project_id FROM resource_instance_pack_projects WHERE instance_id = ?")
            .bind(instance_id).fetch_optional(&self.pool).await.map_err(database_error)?;
        let Some(project) =
            row.and_then(|(source, id)| project_key(&source, id.as_deref()?))
        else {
            return Ok(None);
        };
        Ok(self
            .enrich_many(std::slice::from_ref(&project))
            .await
            .remove(&project))
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
