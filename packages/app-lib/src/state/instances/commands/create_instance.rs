use crate::launcher::get_loader_version_from_profile;
use crate::state::instances::{
    ContentSet, ContentSetStatus, ContentSourceKind, Instance,
    InstanceLaunchOverrides, InstanceLink, LoaderComponent,
    adapters::sqlite::{
        config_sync_rows, content_rows, instance_rows, loader_component_rows,
    },
    config_sync,
};
use crate::state::{
    InstanceInstallStage, LauncherFeatureVersion, ModLoader, ReleaseChannel,
    State,
};
use crate::util::fetch::{self, write_cached_icon};
use crate::util::io;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, trace};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateInstance {
    pub name: String,
    pub path: Option<String>,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub icon_path: Option<String>,
    pub link: InstanceLink,
    #[serde(default)]
    pub symlink_target: Option<String>,
    #[serde(default)]
    pub game_dir_override: Option<String>,
}

pub(crate) async fn create_instance(
    input: CreateInstance,
    state: &State,
) -> crate::Result<Instance> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    trace!("Creating new instance. {}", input.name);

    let (path, full_path) =
        resolve_instance_path(&input.name, input.path.as_deref(), state)
            .await?;

    if let Some(symlink_target) = &input.symlink_target {
        io::create_symlink(symlink_target, &full_path).await?;
    } else {
        io::create_dir_all(&full_path).await?;
    }

    let result = async {
        if let Some(game_dir_override) = input.game_dir_override.as_deref() {
            // External game directories are created before the launcher
            // resolves them with `canonicalize` during installation.
            io::create_dir_all(&PathBuf::from(game_dir_override)).await?;
            if is_version_isolated_game_dir(Path::new(game_dir_override)) {
                // Keep the generic direct-link resolver on the isolated
                // version directory even while it is still empty.
                io::create_dir_all(
                    PathBuf::from(game_dir_override).join("mods"),
                )
                .await?;
            }
        }

        info!(
            "Creating instance at path {}",
            &io::canonicalize(&full_path)?.display()
        );

        let loader_version = if input.loader != ModLoader::Vanilla {
            get_loader_version_from_profile(
                &input.game_version,
                input.loader,
                input.loader_version.as_deref(),
            )
            .await?
            .map(|value| value.id)
        } else {
            None
        };

        let icon_path =
            resolve_icon_path(input.icon_path.as_deref(), state).await?;
        let now = Utc::now();
        let instance_id = format!("local:{}", Uuid::new_v4());
        let content_set_id = format!("content-set:{}", Uuid::new_v4());
        let instance = Instance {
            id: instance_id.clone(),
            path: path.clone(),
            applied_content_set_id: Some(content_set_id.clone()),
            install_stage: InstanceInstallStage::NotInstalled,
            launcher_feature_version: LauncherFeatureVersion::MOST_RECENT,
            update_channel: ReleaseChannel::Release,
            name: input.name,
            icon_path,
            symlink_target: input.symlink_target,
            linked_launcher: None,
            linked_launcher_root: None,
            linked_dot_minecraft: None,
            linked_version_id: None,
            linked_version_json_path: None,
            linked_game_dir_mode: None,
            game_dir_override: input.game_dir_override,
            created: now,
            modified: now,
            last_played: None,
            pinned_at: None,
            submitted_time_played: 0,
            recent_time_played: 0,
        };
        let content_set = ContentSet {
            id: content_set_id,
            instance_id: instance_id.clone(),
            name: "Default".to_string(),
            source_kind: content_source_kind(&input.link),
            status: ContentSetStatus::Available,
            game_version: input.game_version,
            protocol_version: None,
            loader: input.loader,
            loader_version,
            revision: 0,
            created: now,
            modified: now,
        };
        let launch_overrides =
            InstanceLaunchOverrides::empty(instance_id.clone());
        let loader_components = LoaderComponent::from_legacy_projection(
            instance_id.clone(),
            input.loader,
            content_set.loader_version.clone(),
        );

        let mut tx = state.pool.begin().await?;
        instance_rows::insert_instance(&instance, &mut tx).await?;
        sqlx::query(
            "INSERT INTO instance_sync_preferences (instance_id, feature, enabled) SELECT ?, feature, new_instance_default FROM sync_feature_settings",
        )
        .bind(&instance_id)
        .execute(&mut *tx)
        .await?;
        content_rows::insert_content_set(&content_set, &mut tx).await?;
        loader_component_rows::replace_loader_components(
            &instance_id,
            &loader_components,
            &mut tx,
        )
        .await?;
        instance_rows::upsert_instance_link(&instance_id, &input.link, &mut tx)
            .await?;
        instance_rows::replace_instance_groups(&instance_id, &[], &mut tx)
            .await?;
        instance_rows::upsert_instance_launch_overrides(
            &launch_overrides,
            &mut tx,
        )
        .await?;
        config_sync_rows::upsert_config_updated_at(&instance_id, &mut *tx)
            .await?;
        tx.commit().await?;

        config_sync::mark_dirty(&instance_id);

        let content_root = crate::state::instances::commands::instance_content_root(
            &state.directories,
            &instance,
        )?;
        crate::state::instances::watcher::watch_instance_folder(
            &instance.id,
            &instance.path,
            &content_root,
            &state.file_watcher,
        )
        .await;

        Ok(instance)
    }
    .await;

    if result.is_err() {
        let _ = io::remove_dir_all(&full_path).await;
    }

    result
}

pub(crate) async fn resolve_instance_path(
    name: &str,
    path: Option<&str>,
    state: &State,
) -> crate::Result<(String, std::path::PathBuf)> {
    let base_path = path
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| sanitize_instance_name(name));
    let mut path = base_path.clone();
    let mut full_path = state.directories.instances_dir().join(&path);

    if path_available(&path, &full_path, state).await? {
        return Ok((path, full_path));
    }

    let mut which = 1;
    loop {
        path = format!("{base_path} ({which})");
        full_path = state.directories.instances_dir().join(&path);

        if path_available(&path, &full_path, state).await? {
            return Ok((path, full_path));
        }

        which += 1;
    }
}

fn is_version_isolated_game_dir(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
}

async fn path_available(
    path: &str,
    full_path: &std::path::Path,
    state: &State,
) -> crate::Result<bool> {
    if full_path.exists() {
        return Ok(false);
    }

    Ok(instance_rows::get_instance_by_path(path, &state.pool)
        .await?
        .is_none())
}

async fn resolve_icon_path(
    icon_path: Option<&str>,
    state: &State,
) -> crate::Result<Option<String>> {
    let Some(icon) = icon_path else {
        return Ok(None);
    };

    let (bytes, file_name) = if icon.starts_with("https://")
        || icon.starts_with("http://")
    {
        // Icon downloads are best-effort. CurseForge CDN icons often fail
        // through local system proxies; never block instance creation on that.
        let fetched = match fetch_icon_bytes(icon, state).await {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!(
                    "Failed to download instance icon from {icon}: {err}"
                );
                return Ok(None);
            }
        };
        let name = icon.rsplit('/').next().unwrap_or("icon").to_string();
        (fetched, name)
    } else {
        let data =
            match io::read(state.directories.caches_dir().join(icon)).await {
                Ok(data) => data,
                Err(err) => {
                    tracing::warn!(
                        "Failed to read instance icon from {icon}: {err}"
                    );
                    return Ok(None);
                }
            };
        (bytes::Bytes::from(data), icon.to_string())
    };

    let file = write_cached_icon(
        &file_name,
        &state.directories.caches_dir(),
        bytes,
        &state.io_semaphore,
    )
    .await?;

    Ok(Some(file.to_string_lossy().to_string()))
}

async fn fetch_icon_bytes(
    icon_url: &str,
    state: &State,
) -> crate::Result<bytes::Bytes> {
    let normalized =
        crate::util::download::retired_sources::normalize(icon_url)?;
    let icon_url = normalized.as_ref();
    // Prefer a direct connection for CDN hosts that are commonly blocked or
    // broken through local HTTP proxies (e.g. media.forgecdn.net).
    if is_direct_cdn_icon_url(icon_url) {
        let permit = state.fetch_semaphore.0.acquire().await?;
        let response = crate::util::download::native_request::send_cdn_request(
            DIRECT_ICON_CLIENT.get(icon_url),
        )
        .await?;
        drop(permit);
        if !response.status().is_success() {
            return Err(crate::ErrorKind::OtherError(format!(
                "Icon download failed with HTTP {}",
                response.status().as_u16()
            ))
            .into());
        }
        return Ok(response.bytes().await?);
    }

    fetch::fetch(
        icon_url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await
}

fn is_direct_cdn_icon_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    host == "forgecdn.net"
        || host.ends_with(".forgecdn.net")
        || host == "media.forgecdn.net"
}

static DIRECT_ICON_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(15))
            .read_timeout(std::time::Duration::from_secs(30))
            .user_agent(crate::launcher_user_agent())
            .no_proxy()
            .build()
            .expect("Direct icon client configuration should be valid")
    });

fn content_source_kind(link: &InstanceLink) -> ContentSourceKind {
    match link {
        InstanceLink::Unmanaged => ContentSourceKind::Local,
        InstanceLink::ModrinthModpack { .. } => {
            ContentSourceKind::ModrinthModpack
        }
        InstanceLink::CurseForgeModpack { .. } => ContentSourceKind::CurseForge,
        InstanceLink::ServerProject { .. }
        | InstanceLink::ServerProjectModpack { .. } => {
            ContentSourceKind::ServerProject
        }
        InstanceLink::ImportedModpack { .. } => {
            ContentSourceKind::ImportedModpack
        }
        InstanceLink::SharedInstance { .. } => {
            ContentSourceKind::SharedInstance
        }
    }
}

fn sanitize_instance_name(input: &str) -> String {
    input.trim().replace(
        ['/', '\\', '?', '*', ':', '\'', '\"', '|', '<', '>', '!'],
        "_",
    )
}
