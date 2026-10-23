use crate::State;
use crate::state::{CacheBehaviour, CachedEntry};
pub use daedalus::minecraft::VersionManifest;
pub use daedalus::modded::Manifest;

#[tracing::instrument]
pub async fn get_minecraft_versions() -> crate::Result<VersionManifest> {
    match get_minecraft_versions_with_cache(Some(
        CacheBehaviour::MustRevalidate,
    ))
    .await
    {
        Ok(manifest) => Ok(manifest),
        Err(refresh_error) => {
            match get_minecraft_versions_with_cache(Some(
                CacheBehaviour::CacheOnly,
            ))
            .await
            {
                Ok(manifest) => {
                    tracing::warn!(
                        error = %refresh_error,
                        "Minecraft manifest refresh failed; serving cached data"
                    );
                    Ok(manifest)
                }
                Err(_) => Err(refresh_error),
            }
        }
    }
}

pub async fn get_minecraft_versions_with_cache(
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<VersionManifest> {
    let state = State::get().await?;
    let minecraft_versions = CachedEntry::get_minecraft_manifest(
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor("minecraft versions".to_string())
    })?;

    Ok(minecraft_versions)
}

// #[tracing::instrument]
pub async fn get_loader_versions(loader: &str) -> crate::Result<Manifest> {
    match get_loader_versions_with_cache(
        loader,
        Some(CacheBehaviour::StaleWhileRevalidate),
    )
    .await
    {
        Ok(manifest) => Ok(manifest),
        Err(refresh_error) => {
            match get_loader_versions_with_cache(
                loader,
                Some(CacheBehaviour::CacheOnly),
            )
            .await
            {
                Ok(manifest) => {
                    tracing::warn!(
                        loader,
                        error = %refresh_error,
                        "Loader manifest refresh failed; serving cached data"
                    );
                    Ok(manifest)
                }
                Err(_) => Err(refresh_error),
            }
        }
    }
}

pub async fn get_loader_versions_with_cache(
    loader: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Manifest> {
    let state = State::get().await?;
    let cache_key =
        daedalus::modded::loader_manifest_metadata(loader).cache_key;
    let loaders = CachedEntry::get_loader_manifest(
        &cache_key,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor(format!("{loader} loader versions"))
    })?;

    Ok(loaders.manifest)
}

pub async fn get_loader_versions_for_game(
    loader: &str,
    game_version: &str,
) -> crate::Result<Manifest> {
    match get_loader_versions_for_game_with_cache(
        loader,
        game_version,
        Some(CacheBehaviour::StaleWhileRevalidate),
    )
    .await
    {
        Ok(manifest) => Ok(manifest),
        Err(refresh_error) => {
            match get_loader_versions_for_game_with_cache(
                loader,
                game_version,
                Some(CacheBehaviour::CacheOnly),
            )
            .await
            {
                Ok(manifest) => {
                    tracing::warn!(
                        loader,
                        game_version,
                        error = %refresh_error,
                        "Scoped loader metadata refresh failed; serving cached data"
                    );
                    Ok(manifest)
                }
                Err(_) => Err(refresh_error),
            }
        }
    }
}

pub async fn get_loader_versions_for_game_with_cache(
    loader: &str,
    game_version: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Manifest> {
    let state = State::get().await?;
    let cache_key = daedalus::modded::loader_manifest_metadata_for_game(
        loader,
        game_version,
    )
    .cache_key;
    let loaders = CachedEntry::get_loader_manifest(
        &cache_key,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor(format!(
            "{loader} loader versions for Minecraft {game_version}"
        ))
    })?;

    Ok(loaders.manifest)
}
