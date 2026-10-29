use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::*;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::atom;
use crate::state::store::resource_favorites::{
    ResourceFavoriteKey, ResourceFavorites,
};

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
fn favorite_key(project: &SourceProjectRef) -> ResourceFavoriteKey {
    ResourceFavoriteKey {
        source: project.source.as_str().to_owned(),
        project_id: project.project_id.as_str().to_owned(),
    }
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

/// Removes exactly the favorite a caller names, leaving the other projects of
/// the same resource favorited.
pub(crate) async fn remove_project(
    pool: &sqlx::SqlitePool,
    project: &SourceProjectRef,
) -> ResourceResult<()> {
    let handle = Handle::new(pool.clone());
    ResourceFavorites::set(&handle, &favorite_key(project), None).await?;
    Ok(())
}

/// One saved favorite row, exactly as the user saved it: the provider/project
/// pair, its content type, when it was first saved, and the resource its
/// project maps to (if the identity graph knows it).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedFavorite {
    pub project: SourceProjectRef,
    pub content_type: ResourceType,
    pub saved_at: i64,
    pub resource_id: Option<ResourceId>,
}

pub(crate) async fn saved(
    pool: &sqlx::SqlitePool,
) -> ResourceResult<Vec<SavedFavorite>> {
    let rows = ResourceFavorites::saved(&Handle::new(pool.clone())).await?;
    let mut favorites = Vec::with_capacity(rows.len());
    for (source, project_id, content_type, saved_at, resource_id) in rows {
        favorites.push(SavedFavorite {
            project: SourceProjectRef {
                source: SourceId::new(source)?,
                project_id: SourceProjectId::new(project_id)?,
            },
            content_type: favorite_type(&content_type)?,
            saved_at,
            resource_id: resource_id.map(ResourceId::new).transpose()?,
        });
    }
    Ok(favorites)
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
        let handle = Handle::new(pool.clone());
        let rows = ResourceFavorites::saved(&handle).await?;
        let mut favorites: Vec<ResourceFavorite> = Vec::new();
        let mut groups = HashMap::new();
        let links = ResourceFavorites::favorited_projects(&handle).await?;
        let mut projects_by_resource: HashMap<
            ResourceId,
            Vec<SourceProjectRef>,
        > = HashMap::new();
        let mut aliases = HashMap::new();
        for (resource_id, source, project_id, alias) in links {
            let project = SourceProjectRef {
                source: SourceId::new(source)?,
                project_id: SourceProjectId::new(project_id)?,
            };
            let id = ResourceId::new(resource_id)?;
            aliases.insert(project.clone(), alias);
            projects_by_resource.entry(id).or_default().push(project);
        }
        for (source, project_id, content_type, saved_at, id) in rows {
            let project = SourceProjectRef {
                source: SourceId::new(source)?,
                project_id: SourceProjectId::new(project_id)?,
            };
            let id = id.map(ResourceId::new).transpose()?;
            let content_type = favorite_type(&content_type)?;
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
                    saved_at,
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
        let saved_at = chrono::Utc::now().timestamp_millis();
        ResourceFavorites::save(
            &Handle::new(pool.clone()),
            &favorite_key(&project),
            key,
            saved_at,
        )
        .await?;
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
        let handle = Handle::new(pool.clone());
        let mut unit = atom(&handle).await?;
        for project in projects {
            ResourceFavorites::atom_set(
                &mut unit,
                &favorite_key(&project),
                None,
            )
            .await?;
        }
        unit.commit().await?;
        Ok(())
    }
}
