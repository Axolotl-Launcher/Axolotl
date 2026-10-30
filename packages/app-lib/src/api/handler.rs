use std::path::PathBuf;

use crate::{
    event::{
        CommandPayload, SettingChange,
        emit::{emit_command, emit_warning},
    },
    state::Settings,
    util::io,
};
use url::form_urlencoded;
use urlencoding::decode as url_decode;

fn query_map(query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        map.entry(key.into_owned()).or_insert(value.into_owned());
    }
    map
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

fn launch_params(
    query: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let map = query_map(query);
    (
        non_empty(map.get("instance_id").cloned()),
        non_empty(map.get("server").cloned()),
        non_empty(map.get("singleplayer_world").cloned()),
    )
}

async fn launch_payload(
    id: Option<String>,
    server: Option<String>,
    singleplayer_world: Option<String>,
) -> crate::Result<CommandPayload> {
    if server.is_some() && singleplayer_world.is_some() {
        emit_warning("Cannot launch both a server and a singleplayer world")
            .await?;
        return Err(crate::ErrorKind::InputError(
            "Cannot launch both a server and a singleplayer world".to_string(),
        )
        .into());
    }
    match id {
        Some(id) => Ok(CommandPayload::LaunchInstance {
            id,
            server,
            singleplayer_world,
        }),
        None => Err(crate::ErrorKind::InputError(
            "Launch command requires an instance_id query parameter"
                .to_string(),
        )
        .into()),
    }
}

async fn join_payload(
    id: Option<String>,
    server: Option<String>,
    singleplayer_world: Option<String>,
) -> crate::Result<CommandPayload> {
    if server.is_none() && singleplayer_world.is_none() {
        return Err(crate::ErrorKind::InputError(
            "Join command requires a server or singleplayer_world query parameter".to_string(),
        )
        .into());
    }
    launch_payload(id, server, singleplayer_world).await
}

async fn unknown_path(sublink: &str) -> crate::Result<CommandPayload> {
    emit_warning(&format!("Invalid command, unrecognized path: {sublink}"))
        .await?;
    Err(crate::ErrorKind::InputError(format!(
        "Invalid command, unrecognized path: {sublink}"
    ))
    .into())
}

fn valid_project_type(value: &str) -> bool {
    matches!(
        value,
        "mod"
            | "modpack"
            | "resourcepack"
            | "datapack"
            | "shader"
            | "plugin"
            | "server"
    )
}

fn valid_lab_tool(value: &str) -> bool {
    matches!(
        value,
        "skin-editor"
            | "gradient-text"
            | "recipe-generator"
            | "schematic-preview"
            | "mod-translation"
    )
}

fn open_route(
    path: String,
    query: Option<&str>,
) -> crate::Result<CommandPayload> {
    let query = query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_string);
    Ok(CommandPayload::OpenRoute { path, query })
}

fn split_path_query(sublink: &str) -> (&str, &str) {
    sublink.split_once('?').unwrap_or((sublink, ""))
}

fn decode_segment(raw: &str) -> Option<String> {
    match url_decode(raw) {
        Ok(v) => {
            let v = v.to_string();
            if v.trim().is_empty() { None } else { Some(v) }
        }
        Err(_) => None,
    }
}

async fn install_payload(
    project: Option<String>,
    version: Option<String>,
    kind: &str,
) -> crate::Result<CommandPayload> {
    match (project, version) {
        (_, Some(version)) => {
            Ok(CommandPayload::InstallVersion { id: version })
        }
        (Some(project), None) => match kind {
            "modpack" => Ok(CommandPayload::InstallModpack { id: project }),
            "server" => Ok(CommandPayload::InstallServer { id: project }),
            _ => Ok(CommandPayload::InstallMod { id: project }),
        },
        _ => Err(crate::ErrorKind::InputError(
            "Install command requires a project or version query parameter"
                .to_string(),
        )
        .into()),
    }
}

pub async fn handle_url(sublink: &str) -> crate::Result<CommandPayload> {
    let sublink = sublink.trim().trim_start_matches('/');
    if sublink.is_empty() {
        return open_route("/".to_string(), None);
    }
    let (path_part, query_part) = split_path_query(sublink);
    let mut segments = path_part.split('/').filter(|s| !s.is_empty());
    let namespace = segments.next().unwrap_or("");
    let first = segments.next().unwrap_or("");
    let second = segments.next().unwrap_or("");
    let extra = segments.next().is_some();
    match namespace {
        "open" if !extra => {
            let map = query_map(query_part);
            if let Some(path) = non_empty(map.get("path").cloned()) {
                let extra: Vec<String> =
                    form_urlencoded::parse(query_part.as_bytes())
                        .filter(|(k, _)| k != "path")
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect();
                let query = (!extra.is_empty()).then(|| extra.join("&"));
                return open_route(
                    format!("/{}", path.trim_start_matches('/')),
                    query.as_deref(),
                );
            }
            return unknown_path(sublink).await;
        }
        "home" if first.is_empty() && !extra => {
            return open_route(
                "/".to_string(),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "launch" if first.is_empty() && !extra => {
            let (id, server, world) = launch_params(query_part);
            return launch_payload(id, server, world).await;
        }
        "launch" if first == "instance" && !extra => {
            let map = query_map(query_part);
            return launch_payload(
                decode_segment(second),
                non_empty(map.get("server").cloned()),
                non_empty(map.get("singleplayer_world").cloned()),
            )
            .await;
        }
        "join" if first.is_empty() && !extra => {
            let (id, server, world) = launch_params(query_part);
            return join_payload(id, server, world).await;
        }
        "join" if !extra => {
            let map = query_map(query_part);
            return join_payload(
                decode_segment(first),
                non_empty(map.get("server").cloned()),
                non_empty(map.get("singleplayer_world").cloned()),
            )
            .await;
        }
        "install" if first.is_empty() && !extra => {
            let map = query_map(query_part);
            return install_payload(
                non_empty(map.get("project").cloned()),
                non_empty(map.get("version").cloned()),
                map.get("kind").map(String::as_str).unwrap_or("mod"),
            )
            .await;
        }
        "settings" if first == "set" && second.is_empty() && !extra => {
            require_privileged().await?;
            return privileged_settings_payload(sublink, query_part).await;
        }
        "stop" if first.is_empty() && !extra => {
            require_privileged().await?;
            let map = query_map(query_part);
            return stop_payload(
                sublink,
                non_empty(map.get("instance_id").cloned()),
            );
        }
        "stop" if second.is_empty() && !extra => {
            require_privileged().await?;
            return stop_payload(sublink, decode_segment(first));
        }
        "settings" if second.is_empty() && !extra => {
            let (tab, entry) = settings_params(query_part, first);
            return Ok(CommandPayload::OpenSettings { tab, entry });
        }
        "project" if !first.is_empty() && !extra => {
            let id = decoded(first);
            let map = query_map(query_part);
            let tab = map.get("tab").map(String::as_str).unwrap_or("");
            let version = non_empty(map.get("version").cloned());
            if let Some(version) = version {
                return Ok(CommandPayload::InstallVersion { id: version });
            }
            let suffix = match tab {
                "versions" => "/versions".to_string(),
                "gallery" => "/gallery".to_string(),
                "changelog" => "/changelog".to_string(),
                _ => String::new(),
            };
            return open_route(
                format!("/project/{id}{suffix}"),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "browse" if valid_project_type(first) && !extra => {
            return open_route(
                format!("/browse/{first}"),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "library"
            if ["", "downloaded", "modpacks", "servers", "custom"]
                .contains(&first)
                && !extra =>
        {
            let path = if first.is_empty() {
                "/library".to_string()
            } else {
                format!("/library/{first}")
            };
            return open_route(
                path,
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "instance" if !first.is_empty() && !extra => {
            let id = decoded(first);
            let map = query_map(query_part);
            let tab = map.get("tab").map(String::as_str).unwrap_or("");
            let path = match tab {
                "mods" | "" => format!("/instance/{id}"),
                "files" => format!("/instance/{id}/files"),
                "studio" => format!("/instance/{id}/files/studio"),
                "logs" => format!("/instance/{id}/logs"),
                "worlds" => format!("/instance/{id}/worlds"),
                "screenshots" => format!("/instance/{id}/screenshots"),
                "upgrade" => format!("/instance/{id}/upgrade"),
                _ => return unknown_path(sublink).await,
            };
            return open_route(path, None);
        }
        "downloads" if first.is_empty() && !extra => {
            return open_route(
                "/downloads".to_string(),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "create" if first.is_empty() && !extra => {
            return open_route(
                "/create".to_string(),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "skins" | "worlds" | "screenshots" if first.is_empty() && !extra => {
            return open_route(
                format!("/{namespace}"),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "multiplayer"
            if ["", "servers", "rooms"].contains(&first)
                && (second.is_empty() || first == "servers")
                && !extra =>
        {
            let path = if first.is_empty() {
                "/multiplayer/servers".to_string()
            } else if first == "rooms" {
                "/multiplayer/rooms".to_string()
            } else if second.is_empty() {
                "/multiplayer/servers".to_string()
            } else {
                format!("/multiplayer/servers/{second}")
            };
            return open_route(
                path,
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "lab"
            if (first.is_empty()
                || first == "seed-map"
                || valid_lab_tool(first))
                && second.is_empty()
                && !extra =>
        {
            let path = if first.is_empty() {
                "/lab".to_string()
            } else {
                format!("/lab/{first}")
            };
            return open_route(
                path,
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "help" if first == "drop" && second.is_empty() && !extra => {
            return open_route("/help/drop".to_string(), None);
        }

        // 旧别名
        "discovery" if first.is_empty() && !extra => {
            return open_route(
                "/browse/mod".to_string(),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "favorites" if first.is_empty() && !extra => {
            return open_route(
                "/browse/favorites".to_string(),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "seed-map" if first.is_empty() && !extra => {
            return open_route(
                "/lab/seed-map".to_string(),
                (!query_part.is_empty()).then_some(query_part),
            );
        }
        "mod" if !first.is_empty() && second.is_empty() && !extra => {
            return install_payload(Some(first.to_string()), None, "mod").await;
        }
        "version" if !first.is_empty() && second.is_empty() && !extra => {
            return install_payload(None, Some(first.to_string()), "mod").await;
        }
        "modpack" if !first.is_empty() && second.is_empty() && !extra => {
            return install_payload(Some(first.to_string()), None, "modpack")
                .await;
        }
        "server" if !first.is_empty() && second.is_empty() && !extra => {
            return install_payload(Some(first.to_string()), None, "server")
                .await;
        }
        _ => {}
    }
    unknown_path(sublink).await
}

fn settings_params(
    query: &str,
    tab_segment: &str,
) -> (Option<String>, Option<String>) {
    let map = query_map(query);
    let tab = non_empty(map.get("tab").cloned()).or_else(|| {
        let seg = tab_segment.trim();
        (!seg.is_empty()).then(|| seg.to_string())
    });
    (tab, non_empty(map.get("entry").cloned()))
}

// 特权双闸：两开关均开启才放行，取不到状态时拒绝
async fn require_privileged() -> crate::Result<()> {
    let allowed = match crate::State::get().await {
        Ok(state) => match Settings::get(&state.data).await {
            Ok(settings) => {
                settings.allow_external_scheme
                    && settings.allow_privileged_scheme
            }
            Err(_) => false,
        },
        Err(_) => false,
    };
    if allowed {
        return Ok(());
    }
    emit_warning("Privileged links are disabled in settings").await?;
    Err(crate::ErrorKind::InputError(
        "Privileged links are disabled in settings".to_string(),
    )
    .into())
}

fn in_range(value: &str, min: i64, max: i64) -> bool {
    value.parse::<i64>().is_ok_and(|n| (min..=max).contains(&n))
}

fn no_control(value: &str) -> bool {
    value.chars().all(|c| !c.is_control())
}

fn valid_accent_color(value: &str) -> bool {
    matches!(
        value,
        "pink" | "orange" | "green" | "blue" | "purple" | "system"
    ) || value.strip_prefix("custom:#").is_some_and(|hex| {
        hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
    })
}

// 特权设置键白名单：未知键/非法值整单拒绝
fn valid_privileged_setting(key: &str, value: &str) -> bool {
    let text = |v: &str| no_control(v) && v.len() <= 1024;
    match key {
        "collapsed_navigation"
        | "toggle_sidebar"
        | "hide_nametag_skins_page"
        | "nametag_uses_app_font"
        | "advanced_rendering"
        | "native_decorations"
        | "auto_hide_downloads_button"
        | "show_files_tab_in_instances"
        | "show_worlds_tab_in_instances"
        | "show_screenshots_tab_in_instances"
        | "show_skin_selector_in_sidebar"
        | "hide_on_process_start"
        | "enter_lightweight_mode_on_game_launch"
        | "auto_set_java_high_performance_mode"
        | "force_fullscreen"
        | "maximize_window"
        | "bypass_curseforge_download_restrictions"
        | "ignore_ssl_errors"
        | "auto_concurrent_downloads"
        | "transparent_background"
        | "transparent_background_blur"
        | "custom_window_title_enabled"
        | "memory_auto"
        | "memory_optimize" => matches!(value, "true" | "false"),
        "theme" => matches!(value, "dark" | "light" | "oled" | "system"),
        "default_page" => {
            matches!(value, "Home" | "DiscoverContent" | "Library")
        }
        "close_behavior" => {
            matches!(value, "ask" | "close" | "lightweight")
        }
        "log_level" => {
            matches!(value, "error" | "warn" | "info" | "debug" | "trace")
        }
        "home_layout" => matches!(value, "standard" | "minimal"),
        "download_engine" => matches!(value, "legacy" | "xmcl"),
        "minecraft_metadata_source"
        | "minecraft_file_source"
        | "modrinth_source"
        | "curseforge_source"
        | "mojang_auth_source" => matches!(
            value,
            "auto"
                | "official_only"
                | "mirror_preferred"
                | "official_preferred"
        ),
        "accent_color" => valid_accent_color(value),
        "sidebar_instance_count" => in_range(value, 0, 50),
        "max_concurrent_downloads" | "max_concurrent_writes" => {
            in_range(value, 1, 256)
        }
        "memory_max" => in_range(value, 1, 1_048_576),
        "game_resolution_x" | "game_resolution_y" => in_range(value, 1, 65535),
        "custom_background_blur" => in_range(value, 0, 40),
        "custom_background_opacity" => in_range(value, 10, 100),
        "custom_background_component_opacity"
        | "home_widget_background_opacity"
        | "transparent_background_opacity" => in_range(value, 0, 100),
        "locale" | "default_window_title" => {
            !value.trim().is_empty() && value.len() <= 128 && no_control(value)
        }
        "default_game_language" => {
            value.len() <= 32
                && no_control(value)
                && (value.trim().is_empty()
                    || value.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric()
                            || byte == b'_'
                            || byte == b'-'
                    }))
        }
        // 空串表示清空
        "hooks_pre_launch"
        | "hooks_wrapper"
        | "hooks_post_exit"
        | "custom_dir"
        | "backup_repository_path"
        | "custom_background_path" => text(value),
        _ => false,
    }
}

async fn privileged_settings_payload(
    sublink: &str,
    query: &str,
) -> crate::Result<CommandPayload> {
    let mut changes: Vec<SettingChange> = Vec::new();
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        let (key, value) = (key.into_owned(), value.into_owned());
        if !valid_privileged_setting(&key, &value) {
            emit_warning(&format!(
                "Invalid privileged settings change: {key}={value}"
            ))
            .await?;
            return Err(crate::ErrorKind::InputError(format!(
                "Invalid privileged settings change: {key}={value}"
            ))
            .into());
        }
        if changes.iter().any(|change| change.key == key) {
            emit_warning(&format!("Duplicate privileged settings key: {key}"))
                .await?;
            return Err(crate::ErrorKind::InputError(format!(
                "Duplicate privileged settings key: {key}"
            ))
            .into());
        }
        changes.push(SettingChange { key, value });
    }
    if changes.is_empty() {
        emit_warning("Settings update requires at least one key").await?;
        return Err(crate::ErrorKind::InputError(
            "Settings update requires at least one key".to_string(),
        )
        .into());
    }
    Ok(CommandPayload::UpdateSettings {
        changes,
        source: format!("axolotl://{sublink}"),
    })
}

fn stop_payload(
    sublink: &str,
    instance_id: Option<String>,
) -> crate::Result<CommandPayload> {
    match instance_id {
        Some(instance_id) => Ok(CommandPayload::StopInstance {
            instance_id,
            source: format!("axolotl://{sublink}"),
        }),
        None => Err(crate::ErrorKind::InputError(
            "Stop command requires an instance_id query parameter".to_string(),
        )
        .into()),
    }
}

fn decoded(raw: &str) -> String {
    decode_segment(raw).unwrap_or_default()
}

async fn external_scheme_allowed() -> bool {
    match crate::State::get().await {
        Ok(state) => match Settings::get(&state.data).await {
            Ok(settings) => settings.allow_external_scheme,
            Err(_) => true,
        },
        Err(_) => true,
    }
}

pub async fn parse_command(
    command_string: &str,
) -> crate::Result<CommandPayload> {
    tracing::debug!("Parsing command: {}", &command_string);
    if let Some(sublink) = command_string.strip_prefix("axolotl://") {
        if !external_scheme_allowed().await {
            emit_warning("External links are disabled in settings").await?;
            return Err(crate::ErrorKind::InputError(
                "External links are disabled in settings".to_string(),
            )
            .into());
        }
        Ok(handle_url(sublink).await?)
    } else {
        let path = PathBuf::from(command_string);
        let path = io::canonicalize(path)?;
        if let Some(ext) = path.extension()
            && (ext == "mrpack" || ext == "zip")
        {
            return Ok(CommandPayload::RunMRPack { path });
        }
        emit_warning(&format!(
            "Invalid command, unrecognized filetype: {}",
            path.display()
        ))
        .await?;
        Err(crate::ErrorKind::InputError(format!(
            "Invalid command, unrecognized filetype: {}",
            path.display()
        ))
        .into())
    }
}

pub async fn parse_and_emit_command(command_string: &str) -> crate::Result<()> {
    let command = parse_command(command_string).await?;
    emit_command(command).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_launch_query_command() {
        let command =
            parse_command(
                "axolotl://launch?instance_id=example%20instance&server=example.org%3A25565",
            )
            .await
            .unwrap();
        assert!(matches!(
            command,
            CommandPayload::LaunchInstance { id, server: Some(server), singleplayer_world: None }
                if id == "example instance" && server == "example.org:25565"
        ));
    }

    #[tokio::test]
    async fn parses_legacy_aliases() {
        assert!(matches!(
            parse_command("axolotl://discovery").await.unwrap(),
            CommandPayload::OpenRoute { path, .. } if path == "/browse/mod"
        ));
        assert!(matches!(
            parse_command("axolotl://favorites?x=1").await.unwrap(),
            CommandPayload::OpenRoute { path, query } if path == "/browse/favorites" && query == Some("x=1".to_string())
        ));
        assert!(matches!(
            parse_command("axolotl://seed-map?seed=1").await.unwrap(),
            CommandPayload::OpenRoute { path, query } if path == "/lab/seed-map" && query == Some("seed=1".to_string())
        ));
        assert!(matches!(
            parse_command("axolotl://mod/sodium").await.unwrap(),
            CommandPayload::InstallMod { id } if id == "sodium"
        ));
        assert!(matches!(
            parse_command("axolotl://launch/instance/my-pack?server=example.org").await.unwrap(),
            CommandPayload::LaunchInstance { id, server: Some(server), .. }
                if id == "my-pack" && server == "example.org"
        ));
    }

    #[tokio::test]
    async fn parses_unified_commands() {
        assert!(matches!(
            parse_command("axolotl://project/sodium?tab=versions").await.unwrap(),
            CommandPayload::OpenRoute { path, .. } if path == "/project/sodium/versions"
        ));
        assert!(matches!(
            parse_command("axolotl://browse/mod?q=carpet").await.unwrap(),
            CommandPayload::OpenRoute { path, query } if path == "/browse/mod" && query == Some("q=carpet".to_string())
        ));
        assert!(matches!(
            parse_command("axolotl://instance/my-pack?tab=logs").await.unwrap(),
            CommandPayload::OpenRoute { path, .. } if path == "/instance/my-pack/logs"
        ));
        assert!(matches!(
            parse_command("axolotl://settings?tab=privacy-data").await.unwrap(),
            CommandPayload::OpenSettings { tab: Some(tab), .. } if tab == "privacy-data"
        ));
        assert!(matches!(
            parse_command("axolotl://open?path=/library/servers").await.unwrap(),
            CommandPayload::OpenRoute { path, .. } if path == "/library/servers"
        ));
        assert!(matches!(
            parse_command("axolotl://install?version=abc123").await.unwrap(),
            CommandPayload::InstallVersion { id } if id == "abc123"
        ));
    }
}
