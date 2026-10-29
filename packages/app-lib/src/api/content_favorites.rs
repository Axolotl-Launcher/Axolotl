pub use crate::state::content_favorites::{
    ContentFavorite, ContentFavoriteInput, ContentFavoriteProvider,
    ContentFavoriteType,
};

#[tracing::instrument]
pub async fn list() -> crate::Result<Vec<ContentFavorite>> {
    let state = crate::State::get().await?;
    crate::state::content_favorites::list(&state.data).await
}

#[tracing::instrument]
pub async fn add(
    favorite: ContentFavoriteInput,
) -> crate::Result<ContentFavorite> {
    let state = crate::State::get().await?;
    crate::state::content_favorites::add(favorite, &state.data).await
}

#[tracing::instrument]
pub async fn remove(
    provider: ContentFavoriteProvider,
    project_id: &str,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::content_favorites::remove(provider, project_id, &state.data)
        .await
}
