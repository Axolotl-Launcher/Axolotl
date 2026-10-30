use serde::{Deserialize, Serialize};

use crate::resources::favorites as resource_favorites;
use crate::resources::{
    ResourceType, SourceId, SourceProjectId, SourceProjectRef,
};
pub use crate::state::content_types::{
    ContentFavoriteProvider, ContentFavoriteType,
};
use crate::state::store::Handle;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentFavorite {
    #[serde(default)]
    pub resource_id: Option<String>,
    pub provider: ContentFavoriteProvider,
    pub project_id: String,
    pub content_type: ContentFavoriteType,
    pub saved_at: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentFavoriteInput {
    pub provider: ContentFavoriteProvider,
    pub project_id: String,
    pub content_type: ContentFavoriteType,
}

fn source_project(
    provider: ContentFavoriteProvider,
    project_id: &str,
) -> crate::Result<SourceProjectRef> {
    if project_id.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(
            "A content favorite must have a project ID".to_string(),
        )
        .into());
    }
    Ok(SourceProjectRef {
        source: SourceId::new(provider.as_str())
            .map_err(|error| crate::ErrorKind::InputError(error.to_string()))?,
        project_id: SourceProjectId::new(project_id.trim())
            .map_err(|error| crate::ErrorKind::InputError(error.to_string()))?,
    })
}

fn favorite_type(value: ResourceType) -> Option<ContentFavoriteType> {
    match value {
        ResourceType::Mod => Some(ContentFavoriteType::Mod),
        ResourceType::ResourcePack => Some(ContentFavoriteType::Resourcepack),
        ResourceType::DataPack => Some(ContentFavoriteType::Datapack),
        ResourceType::Shader => Some(ContentFavoriteType::Shader),
        _ => None,
    }
}

fn resource_type(value: ContentFavoriteType) -> ResourceType {
    match value {
        ContentFavoriteType::Mod => ResourceType::Mod,
        ContentFavoriteType::Resourcepack => ResourceType::ResourcePack,
        ContentFavoriteType::Datapack => ResourceType::DataPack,
        ContentFavoriteType::Shader => ResourceType::Shader,
    }
}

/// A source or content type this vocabulary does not name is left out: the
/// command surface only speaks modrinth, curseforge and mcarchive.
fn content_favorite(
    favorite: resource_favorites::SavedFavorite,
) -> Option<ContentFavorite> {
    let provider =
        ContentFavoriteProvider::parse(favorite.project.source.as_str())
            .ok()?;
    Some(ContentFavorite {
        resource_id: favorite.resource_id.map(|id| id.as_str().to_string()),
        provider,
        project_id: favorite.project.project_id.as_str().to_string(),
        content_type: favorite_type(favorite.content_type)?,
        saved_at: favorite.saved_at,
    })
}

pub(crate) async fn list(
    _handle: &Handle,
) -> crate::Result<Vec<ContentFavorite>> {
    let state = crate::State::get().await?;
    let mut favorites = Vec::new();
    for favorite in resource_favorites::saved(&state.pool).await? {
        if let Some(favorite) = content_favorite(favorite) {
            favorites.push(favorite);
        }
    }

    favorites.sort_by(|left, right| {
        right
            .saved_at
            .cmp(&left.saved_at)
            .then_with(|| left.provider.as_str().cmp(right.provider.as_str()))
            .then_with(|| left.project_id.cmp(&right.project_id))
    });
    Ok(favorites)
}

pub(crate) async fn add(
    favorite: ContentFavoriteInput,
    _handle: &Handle,
) -> crate::Result<ContentFavorite> {
    let project = source_project(favorite.provider, &favorite.project_id)?;
    resource_favorites::add(
        project.clone(),
        resource_type(favorite.content_type),
        None,
    )
    .await?;
    // The write reports nothing, and reading the row back keeps the moment an
    // existing favorite was first saved.
    let state = crate::State::get().await?;
    let saved = resource_favorites::saved(&state.pool)
        .await?
        .into_iter()
        .find(|saved| saved.project == project)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "The favorite was removed while it was being saved".to_string(),
            )
        })?;
    Ok(ContentFavorite {
        resource_id: saved.resource_id.map(|id| id.as_str().to_string()),
        provider: favorite.provider,
        project_id: project.project_id.as_str().to_string(),
        content_type: favorite.content_type,
        saved_at: saved.saved_at,
    })
}

pub(crate) async fn remove(
    provider: ContentFavoriteProvider,
    project_id: &str,
    _handle: &Handle,
) -> crate::Result<()> {
    let project = source_project(provider, project_id)?;
    let state = crate::State::get().await?;
    resource_favorites::remove_project(&state.pool, &project).await?;
    Ok(())
}
