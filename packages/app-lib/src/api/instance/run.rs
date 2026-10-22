use super::content::get_projects;
use crate::server_address::ServerAddress;
use crate::state::{
    Credentials, InstanceInstallStage, InstanceLink, ProcessMetadata, Settings,
    State,
};
use crate::util::fetch;
use crate::util::io::IOError;
use crate::util::mojang::mojang_service_url;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

pub use crate::launcher::jvm_args::{GcLaunchIntent, GcLaunchReport};

const DEFAULT_LAUNCH_PREPARATION_TIMEOUT: u64 = 60;
const MIN_LAUNCH_PREPARATION_TIMEOUT: u64 = 30;
const MAX_LAUNCH_PREPARATION_TIMEOUT: u64 = 600;

#[derive(Debug, Clone)]
pub enum QuickPlayType {
    None,
    Singleplayer(String),
    Server(ServerAddress),
}

#[tracing::instrument]
pub async fn run(
    instance_id: &str,
    quick_play_type: QuickPlayType,
    offline_mode: bool,
) -> crate::Result<ProcessMetadata> {
    run_with_extra_launch_args(instance_id, quick_play_type, offline_mode, None)
        .await
}

#[tracing::instrument]
pub async fn run_with_extra_launch_args(
    instance_id: &str,
    quick_play_type: QuickPlayType,
    offline_mode: bool,
    extra_launch_args: Option<Vec<String>>,
) -> crate::Result<ProcessMetadata> {
    Ok(run_with_extra_launch_args_inner(
        instance_id,
        quick_play_type,
        offline_mode,
        extra_launch_args,
        None,
    )
    .await?
    .0)
}

/// Like [`run_with_extra_launch_args`], but additionally resolves a GC-args
/// intent against the actual JVM and reports what was actually used (useful
/// for surfacing strategy fallback / flag pruning to the user).
#[tracing::instrument]
pub async fn run_with_extra_launch_args_with_gc(
    instance_id: &str,
    quick_play_type: QuickPlayType,
    offline_mode: bool,
    extra_launch_args: Option<Vec<String>>,
    gc_intent: Option<GcLaunchIntent>,
) -> crate::Result<(ProcessMetadata, Option<GcLaunchReport>)> {
    run_with_extra_launch_args_inner(
        instance_id,
        quick_play_type,
        offline_mode,
        extra_launch_args,
        gc_intent,
    )
    .await
}

async fn run_with_extra_launch_args_inner(
    instance_id: &str,
    quick_play_type: QuickPlayType,
    offline_mode: bool,
    extra_launch_args: Option<Vec<String>>,
    gc_intent: Option<GcLaunchIntent>,
) -> crate::Result<(ProcessMetadata, Option<GcLaunchReport>)> {
    if crate::api::instance::has_active_instance_operation(instance_id) {
        return Err(crate::ErrorKind::InputError(
			"Wait for the active backup operation to finish before launching this instance"
				.to_string(),
		)
		.into());
    }
    let _maintenance_guard =
        crate::api::instance::lock_instance_maintenance(instance_id).await;
    let state = State::get().await?;
    let launch_preparation_timeout =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .and_then(|context| context.launch_overrides.launch_preparation_timeout)
        .unwrap_or(DEFAULT_LAUNCH_PREPARATION_TIMEOUT)
        .clamp(
            MIN_LAUNCH_PREPARATION_TIMEOUT,
            MAX_LAUNCH_PREPARATION_TIMEOUT,
        );
    let default_account = if offline_mode {
        Credentials::get_offline_credential(&state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::LauncherError(
                    "Offline mode requires an offline Minecraft account"
                        .to_string(),
                )
                .as_error()
            })?
    } else {
        Credentials::get_default_credential(&state.pool)
            .await?
            .ok_or_else(|| crate::ErrorKind::NoCredentialsError.as_error())?
    };

    if default_account.is_offline() {
        crate::anti_piracy::ensure_offline_allowed().await?;
    }

    tokio::time::timeout(
        Duration::from_secs(launch_preparation_timeout),
        run_credentials(
            instance_id,
            &default_account,
            quick_play_type,
            offline_mode,
            extra_launch_args,
            gc_intent,
            launch_preparation_timeout,
        ),
    )
    .await
    .map_err(|_| {
        crate::ErrorKind::LauncherError(
            format!(
                "Minecraft launch preparation timed out after {launch_preparation_timeout} seconds"
            ),
        )
        .as_error()
    })?
}

#[tracing::instrument(skip(credentials))]
async fn run_credentials(
    instance_id: &str,
    credentials: &Credentials,
    quick_play_type: QuickPlayType,
    offline_mode: bool,
    extra_launch_args: Option<Vec<String>>,
    gc_intent: Option<GcLaunchIntent>,
    launch_preparation_timeout: u64,
) -> crate::Result<(ProcessMetadata, Option<GcLaunchReport>)> {
    let state = State::get().await?;
    if let Err(error) =
        crate::api::instance::sync_game_options_before_launch(instance_id).await
    {
        tracing::warn!(
            "Failed to reconcile game options before launching {instance_id}: {error}"
        );
    }
    let settings = Settings::get(&state.pool).await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to run a nonexistent instance {instance_id}!"
            ))
        })?;

    if offline_mode
        && context.instance.install_stage != InstanceInstallStage::Installed
    {
        return Err(crate::ErrorKind::LauncherError(
            "Offline mode can only launch fully downloaded instances"
                .to_string(),
        )
        .as_error());
    }

    let pre_launch_hooks = context
        .launch_overrides
        .hooks
        .pre_launch
        .as_ref()
        .or(settings.hooks.pre_launch.as_ref())
        .filter(|hook_command| !hook_command.is_empty());
    if let Some(hook) = pre_launch_hooks {
        let mut cmd = shlex::split(hook)
            .ok_or_else(|| {
                crate::ErrorKind::LauncherError(format!(
                    "Invalid pre-launch command: {hook}",
                ))
            })?
            .into_iter();

        if let Some(command) = cmd.next() {
            let full_path =
                crate::state::instances::commands::instance_content_root(
                    &state.directories,
                    &context.instance,
                )?;
            let mut command = Command::new(command);
            crate::util::java_environment::configure_child_environment(
                command.as_std_mut(),
            );
            command.args(cmd).current_dir(&full_path).kill_on_drop(true);
            let result = command
                .spawn()
                .map_err(|e| IOError::with_path(e, &full_path))?
                .wait()
                .await
                .map_err(IOError::from)?;

            if !result.success() {
                return Err(crate::ErrorKind::LauncherError(format!(
                    "Non-zero exit code for pre-launch hook: {}",
                    result.code().unwrap_or(-1)
                ))
                .as_error());
            }
        }
    }

    let java_args = if let Some(extra_launch_args) = extra_launch_args {
        extra_launch_args
    } else {
        context
            .launch_overrides
            .extra_launch_args
            .clone()
            .unwrap_or(settings.extra_launch_args)
    };
    let wrapper = context
        .launch_overrides
        .hooks
        .wrapper
        .clone()
        .or(settings.hooks.wrapper)
        .filter(|hook_command| !hook_command.is_empty());
    let mut memory = context.launch_overrides.memory.unwrap_or(settings.memory);
    let resolution = context
        .launch_overrides
        .game_resolution
        .unwrap_or(settings.game_resolution);
    let maximize_window = context
        .launch_overrides
        .maximize_window
        .unwrap_or(settings.maximize_window);
    let window_title = settings
        .custom_window_title_enabled
        .then(|| {
            context
                .launch_overrides
                .window_title
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| settings.default_window_title.clone())
        })
        .filter(|value| !value.trim().is_empty());
    let env_args = context
        .launch_overrides
        .custom_env_vars
        .clone()
        .unwrap_or(settings.custom_env_vars);
    let post_exit_hook = context
        .launch_overrides
        .hooks
        .post_exit
        .clone()
        .or(settings.hooks.post_exit)
        .filter(|hook_command| !hook_command.is_empty());

    let mut mc_set_options: Vec<(String, String)> = vec![];
    if let Some(fullscreen) = context.launch_overrides.force_fullscreen {
        mc_set_options.push(("fullscreen".to_string(), fullscreen.to_string()));
    } else if settings.force_fullscreen {
        mc_set_options.push(("fullscreen".to_string(), "true".to_string()));
    }

    crate::api::instance::apply_game_options_launcher_overrides(
        instance_id,
        &mc_set_options,
    )
    .await?;

    if credentials.is_microsoft()
        && let Some(project_id) = server_play_project_id(&context.link)
        && !project_id.trim().is_empty()
    {
        let server_id = uuid::Uuid::new_v4().to_string();
        let join_url = mojang_service_url(
            "https://sessionserver.mojang.com/session/minecraft/join",
            state.mojang_auth_use_mirror(),
        );
        let join_result = fetch::authentication_client()
            .post(join_url.as_ref())
            .json(&json!({
                "accessToken": &credentials.access_token,
                "selectedProfile": credentials.offline_profile.id.simple().to_string(),
                "serverId": &server_id,
            }))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match join_result {
            Ok(resp) if resp.status().is_success() => {
                let result = fetch::post_json(
                    concat!(
                        env!("MODRINTH_API_BASE_URL"),
                        "analytics/minecraft-server-play"
                    ),
                    json!({
                        "project_id": project_id,
                        "username": &credentials.offline_profile.name,
                        "server_id": &server_id,
                    }),
                    &state.api_semaphore,
                    &state.pool,
                )
                .await;

                match result {
                    Ok(()) => {
                        info!(
                            "Tracked server play for '{project_id}' in analytics"
                        )
                    }
                    Err(err) => warn!("Failed to report server play: {err:?}"),
                }
            }
            Ok(resp) => warn!(
                "Failed to join Mojang session server: HTTP {}",
                resp.status()
            ),
            Err(err) => warn!("Failed to join Mojang session server: {err:?}"),
        }
    }

    if offline_mode {
        crate::minecraft_skins::flush_pending_skin_change_for_profile(
            credentials.offline_profile.id,
        )
        .await?;
    } else {
        crate::minecraft_skins::flush_pending_skin_change().await?;
    }
    if memory.optimize_before_launch
        && crate::api::memory::optimization_supported()
    {
        tracing::info!("Optimizing memory before launching Minecraft");
        crate::api::memory::optimize().await?;
    }

    if memory.automatic {
        let instance_path =
            crate::state::instances::commands::instance_content_root(
                &state.directories,
                &context.instance,
            )?;
        memory.maximum = crate::api::jre::automatic_memory_max_mb_for_instance(
            &instance_path,
            matches!(
                context.applied_content_set.loader,
                crate::state::ModLoader::Forge
                    | crate::state::ModLoader::Fabric
                    | crate::state::ModLoader::Quilt
                    | crate::state::ModLoader::NeoForge
                    | crate::state::ModLoader::Cleanroom
                    | crate::state::ModLoader::LiteLoader
                    | crate::state::ModLoader::LegacyFabric
                    | crate::state::ModLoader::Babric
            ),
        );
        tracing::info!(
            "Automatically allocated {} MiB of memory",
            memory.maximum
        );
    }

    let mut gc_report: Option<GcLaunchReport> = None;
    if credentials.is_offline() {
        crate::anti_piracy::ensure_offline_allowed().await?;
    }
    let process = crate::launcher::launch_minecraft(
        &java_args,
        &env_args,
        &mc_set_options,
        &wrapper,
        &memory,
        &resolution,
        maximize_window,
        window_title,
        launch_preparation_timeout,
        credentials,
        post_exit_hook,
        &context,
        gc_intent,
        &mut gc_report,
        quick_play_type,
        offline_mode,
    )
    .await?;
    Ok((process, gc_report))
}

fn server_play_project_id(link: &InstanceLink) -> Option<&String> {
    match link {
        InstanceLink::ServerProject { project_id }
        | InstanceLink::ServerProjectModpack {
            server_project_id: project_id,
            ..
        } => Some(project_id),
        InstanceLink::Unmanaged
        | InstanceLink::ModrinthModpack { .. }
        | InstanceLink::CurseForgeModpack { .. }
        | InstanceLink::ImportedModpack { .. }
        | InstanceLink::SharedInstance { .. } => None,
    }
}

fn modrinth_pack_version_id(link: &InstanceLink) -> Option<&str> {
    match link {
        InstanceLink::ModrinthModpack { version_id, .. }
        | InstanceLink::ServerProjectModpack {
            content_version_id: version_id,
            ..
        } => Some(version_id),
        InstanceLink::Unmanaged
        | InstanceLink::ServerProject { .. }
        | InstanceLink::CurseForgeModpack { .. }
        | InstanceLink::ImportedModpack { .. }
        | InstanceLink::SharedInstance { .. } => None,
    }
}

fn playtime_api_url(base_url: &str) -> String {
    format!("{}/analytics/playtime", base_url.trim_end_matches('/'))
}

pub async fn kill(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let processes =
        crate::api::process::get_by_instance_id(instance_id).await?;

    for process in processes {
        state.process_manager.kill(process.uuid).await?;
    }

    Ok(())
}

#[tracing::instrument]
pub async fn try_update_playtime_by_instance_id(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to update playtime for nonexistent instance {instance_id}!"
            ))
        })?;
    let updated_recent_playtime = context.instance.recent_time_played;
    let res = if updated_recent_playtime > 0 {
        let modrinth_pack_version_id = modrinth_pack_version_id(&context.link);
        let playtime_update_json = json!({
            "seconds": updated_recent_playtime,
            "loader": context.applied_content_set.loader.as_str(),
            "game_version": &context.applied_content_set.game_version,
            "parent": modrinth_pack_version_id,
        });
        let mut hashmap: HashMap<String, serde_json::Value> = HashMap::new();

        for (_, project) in get_projects(instance_id, None).await? {
            if let Some(metadata) = project.modrinth {
                hashmap.insert(
                    metadata.version_id.to_string(),
                    playtime_update_json.clone(),
                );
            }
        }

        let playtime_url = playtime_api_url(env!("MODRINTH_API_BASE_URL"));
        fetch::post_json(
            &playtime_url,
            serde_json::to_value(hashmap)?,
            &state.api_semaphore,
            &state.pool,
        )
        .await
    } else {
        Ok(())
    };

    if res.is_ok() {
        crate::state::instances::commands::mark_instance_playtime_submitted(
            &context.instance.id,
            updated_recent_playtime,
            &state.pool,
        )
        .await?;
    }

    res
}

#[cfg(test)]
mod tests {
    use super::{modrinth_pack_version_id, playtime_api_url};
    use crate::state::InstanceLink;

    #[test]
    fn playtime_parent_requires_an_explicit_modrinth_link() {
        let modrinth = InstanceLink::ModrinthModpack {
            project_id: "project".to_string(),
            version_id: "version".to_string(),
        };
        let curseforge = InstanceLink::CurseForgeModpack {
            project_id: "123".to_string(),
            version_id: "456".to_string(),
        };
        let imported = InstanceLink::ImportedModpack {
            project_id: Some("legacy-project".to_string()),
            version_id: Some("legacy-version".to_string()),
            name: None,
            version_number: None,
            filename: None,
        };

        assert_eq!(modrinth_pack_version_id(&modrinth), Some("version"));
        assert_eq!(modrinth_pack_version_id(&curseforge), None);
        assert_eq!(modrinth_pack_version_id(&imported), None);
    }

    #[test]
    fn playtime_url_has_a_single_path_separator() {
        assert_eq!(
            playtime_api_url("https://api.modrinth.com"),
            "https://api.modrinth.com/analytics/playtime"
        );
        assert_eq!(
            playtime_api_url("https://api.modrinth.com/"),
            "https://api.modrinth.com/analytics/playtime"
        );
    }
}
