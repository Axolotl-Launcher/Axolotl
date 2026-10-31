use super::{LauncherBackend, business_error, serialize};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use tauri::Emitter;
#[cfg(test)]
#[path = "preferences_tests.rs"]
mod tests;
const PUBLIC: &[&str] = &[
    "resource_chinese_filenames",
    "resource_preserve_filenames",
    "max_concurrent_downloads",
    "max_concurrent_writes",
    "auto_concurrent_downloads",
    "minecraft_metadata_source",
    "minecraft_file_source",
    "modrinth_source",
    "curseforge_source",
    "bypass_curseforge_download_restrictions",
    "ignore_ssl_errors",
    "doh_enabled",
    "mojang_auth_source",
    "theme",
    "accent_color",
    "locale",
    "default_page",
    "collapsed_navigation",
    "hide_nametag_skins_page",
    "nametag_uses_app_font",
    "advanced_rendering",
    "native_decorations",
    "toggle_sidebar",
    "custom_background_path",
    "custom_background_blur",
    "custom_background_opacity",
    "custom_background_component_opacity",
    "ui_font",
    "mono_font",
    "transparent_background",
    "transparent_background_opacity",
    "transparent_background_blur",
    "sidebar_instance_count",
    "close_behavior",
    "log_level",
    "auto_hide_downloads_button",
    "home_layout",
    "minimal_home_instance_id",
    "home_widgets",
    "home_widget_background_opacity",
    "hidden_nav_items",
    "custom_window_title_enabled",
    "default_window_title",
    "terracotta_public_nodes",
    "memory",
    "force_fullscreen",
    "maximize_window",
    "game_resolution",
    "hide_on_process_start",
    "enter_lightweight_mode_on_game_launch",
    "auto_set_java_high_performance_mode",
    "developer_mode",
    "feature_flags",
    "sync_features_across_devices",
    "show_files_tab_in_instances",
    "show_worlds_tab_in_instances",
    "show_screenshots_tab_in_instances",
    "show_skin_selector_in_sidebar",
    "auto_download_updates",
    "allow_external_scheme",
    "allow_privileged_scheme",
];
fn filtered(
    settings: Value,
    fields: Option<Vec<String>>,
) -> Result<Value, ToolError> {
    let requested = fields
        .unwrap_or_else(|| PUBLIC.iter().map(|s| s.to_string()).collect());
    let mut result = serde_json::Map::new();
    for field in requested {
        if !PUBLIC.contains(&field.as_str()) {
            return Err(ToolError::new(
                "unsupported_setting",
                "Use settings_get_schema for the public settings whitelist",
            ));
        }
        if let Some(value) = settings.get(&field) {
            result.insert(field, value.clone());
        }
    }
    Ok(Value::Object(result))
}
fn validate_patch(value: &Value) -> Result<(), ToolError> {
    let patch =
        value.as_object().filter(|p| !p.is_empty()).ok_or_else(|| {
            ToolError::new(
                "invalid_settings_patch",
                "Provide a non-empty JSON object",
            )
        })?;
    for (key, value) in patch {
        if !PUBLIC.contains(&key.as_str()) {
            return Err(ToolError::new(
                "unsupported_setting",
                "Credentials, hooks, environment, migration and internal fields cannot use settings_patch",
            ));
        }
        if key.contains("opacity") && value.as_u64().is_none_or(|v| v > 100) {
            return Err(ToolError::new(
                "invalid_setting_value",
                "Opacity must be an integer from 0 to 100",
            ));
        }
        if matches!(
            key.as_str(),
            "max_concurrent_downloads" | "max_concurrent_writes"
        ) && value.as_u64().is_none_or(|v| !(1..=128).contains(&v))
        {
            return Err(ToolError::new(
                "invalid_setting_value",
                "Concurrency must be 1 to 128",
            ));
        }
    }
    Ok(())
}
fn app(backend: &LauncherBackend) -> Result<tauri::AppHandle, ToolError> {
    backend.app.clone().ok_or_else(|| {
        ToolError::new("app_unavailable", "Desktop app handle is unavailable")
    })
}
pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::SettingsGet { fields } => filtered(
            serialize(theseus::settings::get().await.map_err(business_error)?)?,
            fields,
        ),
        Command::SettingsGetSchema {} => {
            let current = serialize(
                theseus::settings::get().await.map_err(business_error)?,
            )?;
            Ok(
                json!({"fields":PUBLIC.iter().filter_map(|key|current.get(*key).map(|v|json!({"field":key,"current_json_type":match v{Value::Null=>"null_or_native_optional",Value::Bool(_)=>"boolean",Value::Number(_)=>"number",Value::String(_)=>"string",Value::Array(_)=>"array",Value::Object(_)=>"object"},"editable":true}))).collect::<Vec<_>>(),"patch_format":"JSON object merged by native settings validation","requires_full_access_for_patch":true,"opacity_range":[0,100],"concurrency_range":[1,128]}),
            )
        }
        Command::SettingsPatch { patch_json } => {
            let value: Value =
                serde_json::from_str(&patch_json).map_err(|_| {
                    ToolError::new(
                        "invalid_json",
                        "Settings patch is not valid JSON",
                    )
                })?;
            validate_patch(&value)?;
            let settings =
                crate::api::settings::settings_patch(app(backend)?, value)
                    .await
                    .map_err(|_| {
                        ToolError::new(
                            "settings_patch_failed",
                            "Native settings validation rejected the patch",
                        )
                    })?;
            filtered(serialize(settings)?, None)
        }
        Command::PrivacyGet {} => serialize(
            theseus::settings::get_privacy()
                .await
                .map_err(business_error)?,
        ),
        Command::PrivacySet {
            telemetry,
            discord_rpc,
        } => {
            let mut current = theseus::settings::get_privacy()
                .await
                .map_err(business_error)?;
            current.telemetry = telemetry;
            current.discord_rpc = discord_rpc;
            let result = theseus::settings::set_privacy(current)
                .await
                .map_err(business_error)?;
            let _ = app(backend)?.emit("settings", ());
            serialize(result)
        }
        Command::ProxyGet {} => {
            let config = theseus::State::get()
                .await
                .map_err(business_error)?
                .proxy_config()
                .await
                .map_err(business_error)?;
            Ok(
                json!({"mode":config.mode,"has_custom_address":!config.url.trim().is_empty(),"has_credentials":config.has_credentials()}),
            )
        }
        Command::ProxySet {
            mode,
            url,
            preserve_credentials,
        } => {
            let state = theseus::State::get().await.map_err(business_error)?;
            let mut config =
                state.proxy_config().await.map_err(business_error)?;
            config.mode =
                serde_json::from_value(json!(mode)).map_err(|_| {
                    ToolError::new("invalid_proxy_mode", "Unknown proxy mode")
                })?;
            if let Some(address) = url {
                let parsed = url::Url::parse(&address).map_err(|_| {
                    ToolError::new(
                        "invalid_proxy_url",
                        "Use a valid supported proxy URL",
                    )
                })?;
                if !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || parsed.query().is_some()
                    || parsed.fragment().is_some()
                {
                    return Err(ToolError::new(
                        "invalid_proxy_url",
                        "Configure proxy credentials in launcher settings; do not embed them in the URL",
                    ));
                }
                config.url = address;
            }
            if !preserve_credentials {
                config.username.clear();
                config.password.clear();
            }
            config.validate().map_err(|_| {
                ToolError::new(
                    "invalid_proxy_config",
                    "Native proxy validation rejected configuration",
                )
            })?;
            state
                .update_proxy_config(&config)
                .await
                .map_err(business_error)?;
            let _ = app(backend)?.emit("settings", ());
            Ok(
                json!({"saved":true,"mode":config.mode,"has_credentials":config.has_credentials()}),
            )
        }
        Command::ProxyTest {} => {
            let config = theseus::State::get()
                .await
                .map_err(business_error)?
                .proxy_config()
                .await
                .map_err(business_error)?;
            serialize(theseus::test_proxy_connection(&config).await.map_err(
                |_| {
                    ToolError::new(
                        "proxy_test_failed",
                        "Proxy test failed; credentials are not included",
                    )
                },
            )?)
        }
        Command::NetworkTest { target, mode } => {
            let target =
                serde_json::from_value(json!(target)).map_err(|_| {
                    ToolError::new(
                        "invalid_target",
                        "Unknown diagnostic target",
                    )
                })?;
            let mode = serde_json::from_value(json!(mode)).map_err(|_| {
                ToolError::new("invalid_mode", "Unknown diagnostic mode")
            })?;
            serialize(
                theseus::test_network_connection(target, mode)
                    .await
                    .map_err(|_| {
                        ToolError::new(
                            "network_test_failed",
                            "Network diagnostic request failed",
                        )
                    })?,
            )
        }
        Command::NetworkGetEnvironment {} => {
            let state = theseus::State::get().await.map_err(business_error)?;
            let config = state.proxy_config().await.map_err(business_error)?;
            let settings =
                theseus::settings::get().await.map_err(business_error)?;
            Ok(
                json!({"metered":theseus::prelude::is_network_metered().await.map_err(business_error)?,"proxy_mode":config.mode,"doh_enabled":settings.doh_enabled,"tls_verification":!settings.ignore_ssl_errors}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a preference operation",
        )),
    }
}
