//! Clear rebuildable downloads without removing installed content or recovery data.

use super::*;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[derive(Clone, Debug, Default, Serialize)]
pub struct CacheCleanupResult {
    pub removed_bytes: u64,
    pub remaining_bytes: u64,
    pub failures: Vec<String>,
}

fn invalid(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

async fn retained_content(
    directories: &DirectoryInfo,
) -> crate::Result<HashSet<String>> {
    let mut retained = HashSet::new();
    let mut databases = 0;
    for path in [
        directories.settings_dir.join("release/app.db"),
        directories.settings_dir.join("beta/app.db"),
        directories.settings_dir.join("app.db"),
    ] {
        if !fs::try_exists(&path).await? {
            continue;
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .read_only(true)
                    .create_if_missing(false),
            )
            .await?;
        let result = retained_in_channel(&pool).await;
        pool.close().await;
        retained.extend(result?);
        databases += 1;
    }
    if databases == 0 {
        return Err(invalid(
            "Launcher records are unavailable. Cache cleanup was not started.",
        ));
    }
    Ok(retained)
}

async fn retained_in_channel(
    pool: &sqlx::SqlitePool,
) -> crate::Result<Vec<String>> {
    let busy: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM install_jobs WHERE status IN ('queued', 'running', 'canceling', 'waiting_for_user') OR json_extract(state, '$.rollback') IS NOT NULL").fetch_one(pool).await?;
    if busy > 0 {
        return Err(invalid(
            "Finish pending installations and recovery before clearing downloaded caches.",
        ));
    }
    let store_tables: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('store_blobs', 'store_instance_files', 'store_retained_refs', 'store_operations')").fetch_one(pool).await?;
    if store_tables == 0 {
        return Ok(Vec::new());
    }
    if store_tables != 4 {
        return Err(invalid(
            "Content store records are incomplete. Cache cleanup was not started.",
        ));
    }
    Ok(sqlx::query_scalar::<_, String>("SELECT digest FROM store_instance_files UNION SELECT digest FROM store_retained_refs UNION SELECT digest FROM store_operations WHERE state IN ('staged', 'published')")
        .fetch_all(pool).await?.into_iter().map(|digest| digest.to_ascii_lowercase()).collect())
}

/// Only known download folders are disposable. Icons and native archives can be
/// required offline; staging and unknown folders may hold resumable work.
fn disposable(path: &str, retained: &HashSet<String>) -> bool {
    let mut parts = path.split('/');
    match parts.next().unwrap_or_default() {
        STORAGE_CACHE_FILE => true,
        "content" | "curseforge" | "modpacks" => true,
        "content-store" => parts.next().is_some_and(|digest| {
            parts.next().is_none()
                && digest.len() == 128
                && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !retained.contains(&digest.to_ascii_lowercase())
        }),
        _ => false,
    }
}

fn cache_root(directories: &DirectoryInfo) -> std::io::Result<Dir> {
    Dir::open_ambient_dir(
        &directories.config_dir,
        cap_std::ambient_authority(),
    )?
    .open_dir_nofollow("caches")
}

fn cache_bytes(
    directories: &DirectoryInfo,
    failures: &mut Vec<String>,
) -> std::io::Result<u64> {
    let mut paths = vec![
        directories.caches_dir(),
        directories.settings_dir.join("caches"),
    ];
    paths.sort_by_key(|path| path.components().count());
    let mut roots = Vec::<PathBuf>::new();
    let mut bytes = 0;
    for path in paths {
        if roots.iter().any(|root| path.starts_with(root)) {
            continue;
        }
        roots.push(path.clone());
        let opened = Dir::open_ambient_dir(
            path.parent().unwrap(),
            cap_std::ambient_authority(),
        )
        .and_then(|parent| parent.open_dir_nofollow("caches"));
        match opened {
            Ok(root) => bytes += walk(&root, "", None, failures)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    Ok(bytes)
}

fn verify_resource_cache(
    directories: &DirectoryInfo,
    path: &Path,
) -> std::io::Result<()> {
    let relative = path
        .strip_prefix(directories.caches_dir())
        .map_err(std::io::Error::other)?;
    let mut dir = cache_root(directories)?;
    for part in relative.parent().into_iter().flat_map(Path::iter) {
        dir = dir.open_dir_nofollow(part)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    dir.open_with(
        relative
            .file_name()
            .ok_or_else(|| std::io::Error::other("Invalid cache path"))?,
        &options,
    )?;
    Ok(())
}

fn walk(
    dir: &Dir,
    relative: &str,
    retained: Option<&HashSet<String>>,
    failures: &mut Vec<String>,
) -> std::io::Result<u64> {
    let mut bytes = 0;
    for entry in dir.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let path = if relative.is_empty() {
            name.to_string_lossy().into_owned()
        } else {
            format!("{relative}/{}", name.to_string_lossy())
        };
        let outcome = (|| {
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Ok(0);
            }
            if kind.is_dir() {
                let child = dir.open_dir_nofollow(&name)?;
                return walk(&child, &path, retained, failures);
            }
            if !kind.is_file() {
                return Ok(0);
            }
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = dir.open_with(&name, &options)?;
            let metadata = file.metadata()?;
            let size = metadata.len();
            drop(file);
            if let Some(retained) = retained {
                if disposable(&path, retained) {
                    dir.remove_file(&name)?;
                }
                Ok(0)
            } else {
                Ok(size)
            }
        })();
        match outcome {
            Ok(size) => bytes += size,
            Err(error) => failures.push(format!("{path}: {error}")),
        }
    }
    Ok(bytes)
}

pub async fn clear_cache() -> crate::Result<CacheCleanupResult> {
    let _scan = SCAN_LOCK.lock().await;
    let _usage = usage::exclusive().await?;
    let state = crate::State::get().await?;
    let directories = dirs()?;
    let retained = retained_content(&directories).await?;
    let measure_directories = dirs()?;
    let (before, mut failures) = tokio::task::spawn_blocking(move || {
        let mut failures = Vec::new();
        let before = cache_bytes(&measure_directories, &mut failures)?;
        Ok::<_, std::io::Error>((before, failures))
    })
    .await
    .map_err(std::io::Error::other)??;
    let resources = state
        .resources()
        .await
        .map_err(crate::resources::protocol::error)?;
    verify_resource_cache(&directories, resources.cache().path())?;
    resources
        .cache()
        .info()
        .await
        .map_err(crate::resources::protocol::error)?;
    use crate::state::{CacheValueType as Kind, CachedEntry};
    if let Err(error) = CachedEntry::purge_cache_types(
        &[
            Kind::Project,
            Kind::ProjectV3,
            Kind::CurseForgeProject,
            Kind::Version,
            Kind::VersionV3,
            Kind::User,
            Kind::Team,
            Kind::Organization,
            Kind::File,
            Kind::LoaderManifest,
            Kind::MinecraftManifest,
            Kind::Categories,
            Kind::ReportTypes,
            Kind::Loaders,
            Kind::GameVersions,
            Kind::DonationPlatforms,
            Kind::FileHash,
            Kind::FileUpdate,
            Kind::SearchResults,
            Kind::SearchResultsV3,
            Kind::ModpackFiles,
            Kind::ProjectVersions,
        ],
        &state.pool,
    )
    .await
    {
        failures.push(format!("API cache: {error}"));
    }
    if let Err(error) = async {
        resources.cache().clear().await?;
        resources.cache().compact().await
    }
    .await
    {
        failures.push(format!("Resource cache: {error}"));
    }
    for channel in ["release", "beta"] {
        let path = directories.resource_cache_db(channel)?;
        if path == resources.cache().path() || !fs::try_exists(&path).await? {
            continue;
        }
        if let Err(error) = verify_resource_cache(&directories, &path) {
            failures.push(format!("resources/{channel}: {error}"));
            continue;
        }
        let cache = crate::resources::cache::ResourceCache::open(path).await;
        let result = async {
            cache.info().await?;
            cache.clear().await?;
            cache.compact().await
        }
        .await;
        cache.close().await;
        if let Err(error) = result {
            failures.push(format!("resources/{channel}: {error}"));
        }
    }
    let result = tokio::task::spawn_blocking(move || {
        let _usage = _usage;
        let root = match cache_root(&directories) {
            Ok(root) => root,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CacheCleanupResult::default());
            }
            Err(error) => return Err(error),
        };
        walk(&root, "", Some(&retained), &mut failures)?;
        let remaining_bytes = cache_bytes(&directories, &mut failures)?;
        Ok::<_, std::io::Error>(CacheCleanupResult {
            removed_bytes: before.saturating_sub(remaining_bytes),
            remaining_bytes,
            failures,
        })
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(result)
}

#[cfg(test)]
mod tests;
