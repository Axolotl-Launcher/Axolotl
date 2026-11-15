use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::*;
use crate::event::emit::emit_favorite_groups_changed;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::resource_source_projects::{
    ResourceSourceProjectKey, ResourceSourceProjects,
};
use crate::state::user_state::favorites as documents;

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
/// The key favorite groups are stored under. Kept separate from
/// document tuple keys so the existing grouping file keeps its key format.
fn favorite_group_key(project: &SourceProjectRef) -> String {
    format!(
        "{}:{}",
        project.source.as_str(),
        project.project_id.as_str()
    )
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
        let enrichment = async {
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
            Ok::<(), ResourceError>(())
        }
        .await;
        if let Err(error) = enrichment {
            tracing::warn!(%error,"Favorite locator enrichment unavailable; saving the selected project");
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
    #[cfg(test)]
    prepare_fixture(pool).await;
    documents::remove(
        &crate::state::store::persistence_for_pool(pool),
        vec![(
            project.source.as_str().to_owned(),
            project.project_id.as_str().to_owned(),
        )],
    )
    .await?;
    let group_key = favorite_group_key(project);
    crate::state::favorite_groups::forget(&group_key);
    emit_favorite_groups_changed(std::slice::from_ref(&group_key)).await?;
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
    #[cfg(test)]
    prepare_fixture(pool).await;
    let rows = documents::all(&crate::state::store::persistence_for_pool(pool))
        .await?;
    let handle = Handle::new(pool.clone());
    let mut favorites = Vec::with_capacity(rows.len());
    for row in rows {
        let (Ok(source), Ok(project_id), Ok(content_type)) = (
            SourceId::new(row.source.clone()),
            SourceProjectId::new(row.project_id.clone()),
            favorite_type(&row.content_type),
        ) else {
            continue;
        };
        let key = ResourceSourceProjectKey {
            source: row.source,
            project_id: row.project_id,
        };
        let resource_id = match ResourceSourceProjects::get(&handle, &key).await
        {
            Ok(Some(record)) => ResourceId::new(record.resource_id).ok(),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error,"Favorite identity enrichment unavailable");
                None
            }
        };
        favorites.push(SavedFavorite {
            project: SourceProjectRef { source, project_id },
            content_type,
            saved_at: row.saved_at,
            resource_id,
        });
    }
    Ok(favorites)
}

impl ResourceService {
    pub async fn favorites(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> ResourceResult<Vec<ResourceFavorite>> {
        let preferred = self.preferred_source().await.ok();
        let rows = saved(pool).await?;
        let mut favorites: Vec<ResourceFavorite> = Vec::new();
        let mut groups = HashMap::new();
        let mut related = HashMap::<ResourceId, Vec<SourceProjectRef>>::new();
        for row in rows {
            // A future provider remains in the document for a reader that can
            // understand it; it does not invalidate supported favorites.
            if self.source_namespace(&row.project.source).is_err() {
                continue;
            }
            let key = (
                row.resource_id
                    .as_ref()
                    .map(|id| id.as_str().to_owned())
                    .unwrap_or_else(|| favorite_group_key(&row.project)),
                type_key(row.content_type)?,
            );
            if let Some(index) = groups.get(&key).copied() {
                let favorite: &mut ResourceFavorite = &mut favorites[index];
                if !favorite.projects.contains(&row.project) {
                    favorite.projects.push(row.project.clone());
                }
                if preferred.as_ref() == Some(&row.project.source) {
                    favorite.project = row.project;
                }
                continue;
            }
            let mut projects = vec![row.project.clone()];
            if let (Some(id), Some(store)) = (&row.resource_id, self.store()) {
                let known = if let Some(known) = related.get(id) {
                    known.clone()
                } else {
                    let known=store.projects(id).await.unwrap_or_else(|error|{tracing::warn!(%error,"Favorite project links unavailable");Vec::new()});
                    related.insert(id.clone(), known.clone());
                    known
                };
                projects.extend(known.into_iter().filter(|project| {
                    self.source_namespace(&project.source).is_ok()
                }));
                projects.sort_by(|a, b| {
                    (a.source.as_str(), a.project_id.as_str())
                        .cmp(&(b.source.as_str(), b.project_id.as_str()))
                });
                projects.dedup();
            }
            let project = projects
                .iter()
                .find(|project| preferred.as_ref() == Some(&project.source))
                .cloned()
                .unwrap_or(row.project);
            let locator = if let Some(store) = self.store() {
                store.project_alias(&project).await.ok().flatten()
            } else {
                None
            };
            groups.insert(key, favorites.len());
            favorites.push(ResourceFavorite {
                locator,
                resource_id: row.resource_id,
                project,
                projects,
                content_type: row.content_type,
                saved_at: row.saved_at,
            });
        }
        for favorite in &mut favorites {
            if !favorite.projects.contains(&favorite.project) {
                favorite.projects.push(favorite.project.clone());
            }
            if let Some(project) = favorite
                .projects
                .iter()
                .find(|project| preferred.as_ref() == Some(&project.source))
                .cloned()
            {
                favorite.project = project;
            }
            favorite.locator = if let Some(store) = self.store() {
                store.project_alias(&favorite.project).await.ok().flatten()
            } else {
                None
            };
        }
        Ok(favorites)
    }

    pub async fn favorite_add(
        &self,
        pool: &sqlx::SqlitePool,
        project: SourceProjectRef,
        content_type: ResourceType,
    ) -> ResourceResult<()> {
        #[cfg(test)]
        prepare_fixture(pool).await;
        let content_type_key = type_key(content_type)?;
        self.source_namespace(&project.source)?;
        if let Some(store) = self.store() {
            if let Err(error) =
                store.ensure_project(&project, &[content_type]).await
            {
                tracing::warn!(%error,"Favorite saved without optional identity enrichment");
            }
        }
        documents::save(
            &crate::state::store::persistence_for_pool(pool),
            documents::Favorite {
                source: project.source.as_str().to_owned(),
                project_id: project.project_id.as_str().to_owned(),
                content_type: content_type_key.into(),
                saved_at: chrono::Utc::now().timestamp_millis(),
            },
        )
        .await?;
        Ok(())
    }

    pub async fn favorite_remove(
        &self,
        pool: &sqlx::SqlitePool,
        project: &SourceProjectRef,
    ) -> ResourceResult<()> {
        #[cfg(test)]
        prepare_fixture(pool).await;
        self.source_namespace(&project.source)?;
        let mut projects = vec![project.clone()];
        if let Some(store) = self.store() {
            match store.lookup(project).await {
                Ok(Some(id)) => match store.projects(&id).await {
                    Ok(known) => projects.extend(known),
                    Err(error) => {
                        tracing::warn!(%error,"Only the selected favorite can be resolved")
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(%error,"Only the selected favorite can be resolved")
                }
            }
        }
        projects
            .retain(|project| self.source_namespace(&project.source).is_ok());
        projects.sort_by(|a, b| {
            (a.source.as_str(), a.project_id.as_str())
                .cmp(&(b.source.as_str(), b.project_id.as_str()))
        });
        projects.dedup();
        documents::remove(
            &crate::state::store::persistence_for_pool(pool),
            projects
                .iter()
                .map(|project| {
                    (
                        project.source.as_str().to_owned(),
                        project.project_id.as_str().to_owned(),
                    )
                })
                .collect(),
        )
        .await?;
        let group_keys =
            projects.iter().map(favorite_group_key).collect::<Vec<_>>();
        for group_key in &group_keys {
            crate::state::favorite_groups::forget(group_key);
        }
        emit_favorite_groups_changed(&group_keys).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(source: &str, project_id: &str) -> SourceProjectRef {
        SourceProjectRef {
            source: SourceId::new(source).unwrap(),
            project_id: SourceProjectId::new(project_id).unwrap(),
        }
    }

    #[test]
    fn favorite_group_key_is_source_and_project_id() {
        assert_eq!(
            favorite_group_key(&project("modrinth", "sodium")),
            "modrinth:sodium"
        );
    }
}

#[cfg(test)]
async fn prepare_fixture(pool: &sqlx::SqlitePool) {
    if pool
        .connect_options()
        .get_filename()
        .file_name()
        .is_some_and(|name| name == "system.db")
    {
        return;
    }
    let persistence = crate::state::store::persistence_for_pool(pool);
    let read = documents::domain(&persistence)
        .document
        .read()
        .await
        .expect("fixture authority");
    if read.availability
        == crate::state::persistence::status::Availability::Missing
    {
        crate::state::store::legacy_user_state::initialize_test_pool(pool)
            .await;
    }
}
