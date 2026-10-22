use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceFavorite {
    pub locator: Option<String>,
    pub resource_id: Option<ResourceId>,
    pub project: SourceProjectRef,
    pub projects: Vec<SourceProjectRef>,
    pub content_type: ResourceType,
    pub saved_at: i64,
}

fn favorite_type(value: &str) -> ResourceResult<ResourceType> {
    match value {
        "mod" => Ok(ResourceType::Mod),
        "resourcepack" => Ok(ResourceType::ResourcePack),
        "datapack" => Ok(ResourceType::DataPack),
        "shader" => Ok(ResourceType::Shader),
        _ => Err(ResourceError::InvalidRequest(
            "unsupported favorite type".into(),
        )),
    }
}
fn type_key(value: ResourceType) -> ResourceResult<&'static str> {
    match value {
        ResourceType::Mod => Ok("mod"),
        ResourceType::ResourcePack => Ok("resourcepack"),
        ResourceType::DataPack => Ok("datapack"),
        ResourceType::Shader => Ok("shader"),
        _ => Err(ResourceError::InvalidRequest(
            "unsupported favorite type".into(),
        )),
    }
}
fn db(error: sqlx::Error) -> ResourceError {
    ResourceError::Database(error.to_string())
}

pub async fn list() -> ResourceResult<Vec<ResourceFavorite>> {
    let state = crate::State::get().await?;
    state.resources().await?.favorites(&state.pool).await
}
pub async fn add(
    project: SourceProjectRef,
    content_type: ResourceType,
    locator: Option<String>,
) -> ResourceResult<()> {
    let state = crate::State::get().await?;
    let service = state.resources().await?;
    if let Some(locator) = locator {
        let document = service
            .project_document(
                &project,
                Some(locator),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await?;
        let store = service.store().ok_or_else(|| {
            ResourceError::Database("resource store unavailable".into())
        })?;
        store.ensure_project(&project, &[content_type]).await?;
        if let Some(alias) = document.metadata.slug {
            store.add_alias(&project, &alias).await?;
        }
    }
    service
        .favorite_add(&state.pool, project, content_type)
        .await
}
pub async fn remove(project: &SourceProjectRef) -> ResourceResult<()> {
    let state = crate::State::get().await?;
    state
        .resources()
        .await?
        .favorite_remove(&state.pool, project)
        .await
}

impl ResourceService {
    pub async fn favorites(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> ResourceResult<Vec<ResourceFavorite>> {
        self.store().ok_or_else(|| {
            ResourceError::Database("resource store unavailable".into())
        })?;
        let preferred = self.preferred_source().await?;
        let rows = sqlx::query("SELECT f.*, p.resource_id FROM (
            SELECT source, project_id, content_type, saved_at FROM resource_favorites
            UNION ALL SELECT provider AS source, project_id, content_type, saved_at FROM content_favorites AS old
                WHERE NOT EXISTS (SELECT 1 FROM resource_favorites AS current WHERE current.source = old.provider AND current.project_id = old.project_id)
            ) AS f LEFT JOIN resource_source_projects AS p ON p.source = f.source AND p.project_id = f.project_id ORDER BY f.saved_at DESC, f.source, f.project_id")
            .fetch_all(pool).await.map_err(db)?;
        let mut favorites: Vec<ResourceFavorite> = Vec::new();
        let mut groups = HashMap::new();
        let links = sqlx::query("SELECT project.resource_id, project.source, project.project_id, (SELECT alias FROM resource_aliases alias WHERE alias.source = project.source AND alias.project_id = project.project_id ORDER BY rowid DESC LIMIT 1) AS alias FROM resource_source_projects project WHERE project.resource_id IN (SELECT project.resource_id FROM resource_source_projects project JOIN resource_favorites favorite ON favorite.source = project.source AND favorite.project_id = project.project_id UNION SELECT project.resource_id FROM resource_source_projects project JOIN content_favorites favorite ON favorite.provider = project.source AND favorite.project_id = project.project_id) ORDER BY project.source, project.project_id").fetch_all(pool).await.map_err(db)?;
        let mut projects_by_resource: HashMap<
            ResourceId,
            Vec<SourceProjectRef>,
        > = HashMap::new();
        let mut aliases = HashMap::new();
        for link in links {
            let project = SourceProjectRef {
                source: SourceId::new(
                    link.try_get::<String, _>("source").map_err(db)?,
                )?,
                project_id: SourceProjectId::new(
                    link.try_get::<String, _>("project_id").map_err(db)?,
                )?,
            };
            let id = ResourceId::new(
                link.try_get::<String, _>("resource_id").map_err(db)?,
            )?;
            aliases.insert(
                project.clone(),
                link.try_get::<Option<String>, _>("alias").map_err(db)?,
            );
            projects_by_resource.entry(id).or_default().push(project);
        }
        for row in rows {
            let project = SourceProjectRef {
                source: SourceId::new(
                    row.try_get::<String, _>("source").map_err(db)?,
                )?,
                project_id: SourceProjectId::new(
                    row.try_get::<String, _>("project_id").map_err(db)?,
                )?,
            };
            let id = row
                .try_get::<Option<String>, _>("resource_id")
                .map_err(db)?
                .map(ResourceId::new)
                .transpose()?;
            let content_type = favorite_type(
                &row.try_get::<String, _>("content_type").map_err(db)?,
            )?;
            let key = (
                id.as_ref().map(|id| id.as_str().to_owned()).unwrap_or_else(
                    || {
                        format!(
                            "{}:{}",
                            project.source.as_str(),
                            project.project_id.as_str()
                        )
                    },
                ),
                type_key(content_type)?,
            );
            if let Some(index) = groups.get(&key).copied() {
                let current: &mut ResourceFavorite = &mut favorites[index];
                if project.source == preferred {
                    current.project = project;
                }
            } else {
                let projects = if let Some(id) = &id {
                    projects_by_resource.get(id).cloned().unwrap_or_default()
                } else {
                    vec![project.clone()]
                };
                groups.insert(key, favorites.len());
                favorites.push(ResourceFavorite {
                    locator: aliases.get(&project).cloned().flatten(),
                    resource_id: id,
                    project,
                    projects,
                    content_type,
                    saved_at: row.try_get("saved_at").map_err(db)?,
                });
            }
        }
        for favorite in &mut favorites {
            if let Some(project) = favorite
                .projects
                .iter()
                .find(|project| project.source == preferred)
            {
                favorite.project = project.clone();
                favorite.locator = aliases.get(project).cloned().flatten();
            }
        }
        Ok(favorites)
    }

    pub async fn favorite_add(
        &self,
        pool: &sqlx::SqlitePool,
        project: SourceProjectRef,
        content_type: ResourceType,
    ) -> ResourceResult<()> {
        let key = type_key(content_type)?;
        let store = self.store().ok_or_else(|| {
            ResourceError::Database("resource store unavailable".into())
        })?;
        store.ensure_project(&project, &[content_type]).await?;
        let mut transaction = pool.begin().await.map_err(db)?;
        let saved_at = chrono::Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO resource_favorites(source, project_id, content_type, saved_at) VALUES (?, ?, ?, ?) ON CONFLICT(source, project_id) DO UPDATE SET content_type = excluded.content_type")
            .bind(project.source.as_str()).bind(project.project_id.as_str()).bind(key).bind(saved_at).execute(&mut *transaction).await.map_err(db)?;
        if matches!(
            project.source.as_str(),
            "modrinth" | "curseforge" | "mcarchive"
        ) {
            sqlx::query("INSERT INTO content_favorites(provider, project_id, content_type, saved_at) VALUES (?, ?, ?, ?) ON CONFLICT(provider, project_id) DO UPDATE SET content_type = excluded.content_type")
                .bind(project.source.as_str()).bind(project.project_id.as_str()).bind(key).bind(saved_at).execute(&mut *transaction).await.map_err(db)?;
        }
        transaction.commit().await.map_err(db)?;
        Ok(())
    }

    pub async fn favorite_remove(
        &self,
        pool: &sqlx::SqlitePool,
        project: &SourceProjectRef,
    ) -> ResourceResult<()> {
        let store = self.store().ok_or_else(|| {
            ResourceError::Database("resource store unavailable".into())
        })?;
        let id = store.lookup(project).await.or_else(|error| {
            if matches!(error, ResourceError::InvalidRequest(_)) {
                Ok(None)
            } else {
                Err(error)
            }
        })?;
        let mut projects = if let Some(id) = id {
            store.projects(&id).await?
        } else {
            vec![]
        };
        if !projects.contains(project) {
            projects.push(project.clone());
        }
        let mut transaction = pool.begin().await.map_err(db)?;
        for project in projects {
            sqlx::query("DELETE FROM resource_favorites WHERE source = ? AND project_id = ?").bind(project.source.as_str()).bind(project.project_id.as_str()).execute(&mut *transaction).await.map_err(db)?;
            sqlx::query("DELETE FROM content_favorites WHERE provider = ? AND project_id = ?").bind(project.source.as_str()).bind(project.project_id.as_str()).execute(&mut *transaction).await.map_err(db)?;
        }
        transaction.commit().await.map_err(db)?;
        Ok(())
    }
}
