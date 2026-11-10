use crate::server_address::ServerAddress;
use crate::state::{
    Credentials, InstanceInstallStage, ProcessMetadata, Settings, State,
};
use crate::util::io::IOError;
use std::time::Duration;
use tokio::process::Command;

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
        Credentials::get_offline_credential(&state.data)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::LauncherError(
                    "Offline mode requires an offline Minecraft account"
                        .to_string(),
                )
                .as_error()
            })?
    } else {
        Credentials::get_default_credential(&state.data)
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
    let settings = Settings::get(&state.data).await?;
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

pub async fn kill(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let processes =
        crate::api::process::get_by_instance_id(instance_id).await?;

    for process in processes {
        state.process_manager.kill(process.uuid).await?;
    }

    Ok(())
}
