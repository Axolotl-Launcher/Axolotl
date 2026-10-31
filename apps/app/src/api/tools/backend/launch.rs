use super::business_error;
use launcher_tools::{Command, LaunchField, ToolError};
use serde_json::{Value, json};
use theseus::data::{EditInstance, InstanceLaunchOverridesPatch};
use theseus::settings::{MemorySettings, WindowSize};

async fn metadata(
    instance_id: &str,
) -> Result<theseus::data::InstanceMetadata, ToolError> {
    theseus::instance::get(instance_id)
        .await
        .map_err(business_error)?
        .ok_or_else(|| ToolError::new("not_found", "Instance not found"))
}

pub(super) async fn config(instance_id: &str) -> Result<Value, ToolError> {
    let metadata = metadata(instance_id).await?;
    let settings = theseus::settings::get().await.map_err(business_error)?;
    let overrides = metadata.launch_overrides;
    let memory = overrides.memory.unwrap_or(settings.memory);
    let resolution = overrides
        .game_resolution
        .unwrap_or(settings.game_resolution);
    let configured_hook = |local: &Option<String>, global: &Option<String>| {
        local
            .as_ref()
            .or(global.as_ref())
            .is_some_and(|value| !value.is_empty())
    };
    Ok(
        json!({"instance_id":instance_id,"applies_to":"future_launches",
            "overrides":{"memory":overrides.memory,"fullscreen":overrides.force_fullscreen,"maximized":overrides.maximize_window,
                "resolution":overrides.game_resolution,"java_path":overrides.java_path},
            "effective":{"memory":{"maximum_mib":memory.maximum,"automatic":memory.automatic,"optimize_before_launch":memory.optimize_before_launch},
                "fullscreen":overrides.force_fullscreen.unwrap_or(settings.force_fullscreen),"maximized":overrides.maximize_window.unwrap_or(settings.maximize_window),
                "width":resolution.0,"height":resolution.1,"java_selection":if overrides.java_path.is_some() {"instance_override"} else {"automatic"},
                "custom_window_title_enabled":settings.custom_window_title_enabled},
            "configured_commands":{"pre_launch_hook":configured_hook(&overrides.hooks.pre_launch,&settings.hooks.pre_launch),
                "wrapper":configured_hook(&overrides.hooks.wrapper,&settings.hooks.wrapper),"post_exit_hook":configured_hook(&overrides.hooks.post_exit,&settings.hooks.post_exit),
                "custom_arguments":!overrides.extra_launch_args.as_ref().unwrap_or(&settings.extra_launch_args).is_empty(),
                "custom_environment":!overrides.custom_env_vars.as_ref().unwrap_or(&settings.custom_env_vars).is_empty()}
        }),
    )
}

fn memory_patch(
    automatic: bool,
    maximum_mib: u32,
    optimize_before_launch: bool,
    total_mib: u64,
) -> Result<InstanceLaunchOverridesPatch, ToolError> {
    if !(256..=1048576).contains(&maximum_mib) {
        return Err(ToolError::new(
            "invalid_memory",
            "Memory must be between 256 and 1048576 MiB",
        ));
    }
    if !automatic && u64::from(maximum_mib) > total_mib {
        return Err(ToolError::new(
            "insufficient_physical_memory",
            "Manual allocation exceeds total physical RAM",
        ));
    }
    Ok(InstanceLaunchOverridesPatch {
        memory: Some(Some(MemorySettings {
            maximum: maximum_mib,
            automatic,
            optimize_before_launch,
        })),
        ..Default::default()
    })
}
fn window_patch(
    fullscreen: bool,
    maximized: bool,
    width: u16,
    height: u16,
) -> Result<InstanceLaunchOverridesPatch, ToolError> {
    if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
        return Err(ToolError::new(
            "invalid_resolution",
            "Resolution dimensions must be between 1 and 16384 pixels",
        ));
    }
    Ok(InstanceLaunchOverridesPatch {
        force_fullscreen: Some(Some(fullscreen)),
        maximize_window: Some(Some(maximized)),
        game_resolution: Some(Some(WindowSize(width, height))),
        ..Default::default()
    })
}
fn reset_patch(
    fields: Vec<LaunchField>,
) -> Result<InstanceLaunchOverridesPatch, ToolError> {
    if fields.is_empty() {
        return Err(ToolError::new(
            "no_changes",
            "Choose at least one override group",
        ));
    }
    let mut patch = InstanceLaunchOverridesPatch::default();
    for field in fields {
        match field {
            LaunchField::Memory => patch.memory = Some(None),
            LaunchField::Window => {
                patch.force_fullscreen = Some(None);
                patch.maximize_window = Some(None);
                patch.game_resolution = Some(None);
            }
            LaunchField::Java => patch.java_path = Some(None),
        }
    }
    Ok(patch)
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    let (instance_id, patch) = match command {
        Command::LaunchConfigGet { instance_id } => {
            return config(&instance_id).await;
        }
        Command::LaunchGetMemoryStatus { instance_id } => {
            let metadata = metadata(&instance_id).await?;
            let settings =
                theseus::settings::get().await.map_err(business_error)?;
            let memory =
                metadata.launch_overrides.memory.unwrap_or(settings.memory);
            let status = theseus::jre::get_memory_status(
                Some(&instance_id),
                memory.maximum,
                memory.automatic,
            )
            .await
            .map_err(business_error)?;
            let running = theseus::process::get_by_instance_id(&instance_id)
                .await
                .map_err(business_error)?
                .len();
            return Ok(
                json!({"instance_id":instance_id,"automatic":memory.automatic,"total_bytes":status.total_bytes,"available_bytes":status.available_bytes,
                "allocated_mb":status.allocated_mb,"optimization_supported":status.optimization_supported,"running_processes":running,
                "manual_exceeds_available":!memory.automatic && u64::from(memory.maximum)*1024*1024>status.available_bytes}),
            );
        }
        Command::LaunchSetMemory {
            instance_id,
            automatic,
            maximum_mib,
            optimize_before_launch,
        } => {
            let total_mib = theseus::jre::get_max_memory()
                .await
                .map_err(business_error)?
                / 1024;
            (
                instance_id,
                memory_patch(
                    automatic,
                    maximum_mib,
                    optimize_before_launch,
                    total_mib,
                )?,
            )
        }
        Command::LaunchSetWindowOptions {
            instance_id,
            fullscreen,
            maximized,
            width,
            height,
        } => (
            instance_id,
            window_patch(fullscreen, maximized, width, height)?,
        ),
        Command::LaunchSetJava {
            instance_id,
            registered_path,
        } => {
            let registered = theseus::jre::get_registered_java_versions()
                .await
                .map_err(business_error)?;
            if !registered
                .runtimes
                .iter()
                .any(|runtime| runtime.path == registered_path)
            {
                return Err(ToolError::new(
                    "unregistered_java",
                    "Select an exact path returned by java_list",
                ));
            }
            (
                instance_id,
                InstanceLaunchOverridesPatch {
                    java_path: Some(Some(registered_path)),
                    ..Default::default()
                },
            )
        }
        Command::LaunchResetOverrides {
            instance_id,
            fields,
        } => (instance_id, reset_patch(fields)?),
        _ => {
            return Err(ToolError::new(
                "unsupported_command",
                "Not a launch configuration operation",
            ));
        }
    };
    metadata(&instance_id).await?;
    theseus::instance::edit(
        &instance_id,
        EditInstance {
            launch_overrides: Some(patch),
            ..Default::default()
        },
    )
    .await
    .map_err(business_error)?;
    config(&instance_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_patch_only_changes_memory_and_respects_physical_limit() {
        let patch = memory_patch(false, 4096, true, 8192).unwrap();
        let value = serde_json::to_value(patch).unwrap();
        assert_eq!(value["memory"]["maximum"], 4096);
        assert_eq!(value["memory"]["automatic"], false);
        assert!(value["hooks"].is_null());
        assert!(value.get("java_path").is_none());
        assert_eq!(
            memory_patch(false, 16384, false, 8192).unwrap_err().code,
            "insufficient_physical_memory"
        );
        assert!(memory_patch(true, 16384, false, 8192).is_ok());
    }
    #[test]
    fn window_patch_has_explicit_false_values_and_no_other_changes() {
        let value = serde_json::to_value(
            window_patch(false, false, 1280, 720).unwrap(),
        )
        .unwrap();
        assert_eq!(value["force_fullscreen"], false);
        assert_eq!(value["maximize_window"], false);
        assert_eq!(value["game_resolution"], json!([1280, 720]));
        assert!(value.get("memory").is_none());
        assert!(window_patch(false, false, 0, 720).is_err());
    }
    #[test]
    fn resets_clear_only_named_groups_and_preserve_command_fields() {
        let value = serde_json::to_value(
            reset_patch(vec![LaunchField::Memory, LaunchField::Java]).unwrap(),
        )
        .unwrap();
        assert!(value.get("memory").unwrap().is_null());
        assert!(value.get("java_path").unwrap().is_null());
        assert!(value.get("game_resolution").is_none());
        assert!(value.get("extra_launch_args").is_none());
        assert!(value.get("custom_env_vars").is_none());
        assert!(reset_patch(vec![]).is_err());
    }
}
