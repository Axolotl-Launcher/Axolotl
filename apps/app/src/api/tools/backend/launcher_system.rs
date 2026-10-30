use super::{LauncherBackend, business_error, files::paths, serialize};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
#[cfg(test)]
#[path = "launcher_system_tests.rs"]
mod tests;
#[derive(Clone)]
pub(super) struct UpdateHandle {
    rid: u32,
    version: String,
    _lease: Option<std::sync::Arc<UpdateResource>>,
}
struct UpdateResource {
    view: tauri::Webview<tauri::Wry>,
    rid: u32,
}
impl Drop for UpdateResource {
    fn drop(&mut self) {
        let _ = self.view.resources_table().close(self.rid);
    }
}
fn error(e: impl std::fmt::Display) -> ToolError {
    tracing::warn!(%e,"MCP launcher operation failed");
    ToolError::new(
        "launcher_operation_failed",
        "Launcher action failed; inspect native updater/lifecycle state",
    )
}
fn app(backend: &LauncherBackend) -> Result<tauri::AppHandle, ToolError> {
    backend
        .app
        .clone()
        .ok_or_else(|| error("desktop unavailable"))
}
async fn quiet() -> Result<(), ToolError> {
    if theseus::instance::has_active_backup_operations()
        .await
        .map_err(business_error)?
        || theseus::install::list_jobs(false)
            .await
            .map_err(business_error)?
            .iter()
            .any(|j| !j.status.is_finished())
        || !theseus::process::get_all()
            .await
            .map_err(business_error)?
            .is_empty()
        || theseus::servers::list()
            .await
            .map_err(business_error)?
            .iter()
            .any(|s| s.running || s.starting || s.stopping)
    {
        return Err(ToolError::new(
            "launcher_busy",
            "Finish game/server processes, installation and backup jobs before exiting or restarting",
        ));
    }
    Ok(())
}
fn safe_url(text: &str) -> Result<url::Url, ToolError> {
    let url = url::Url::parse(text).map_err(error)?;
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if url.scheme() != "https"
        || !matches!(url.host(), Some(url::Host::Domain(_)))
        || !url.username().is_empty()
        || url.password().is_some()
        || host.is_empty()
        || host.trim_end_matches('.') == "localhost"
        || host.trim_end_matches('.').ends_with(".localhost")
        || host.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(ToolError::new(
            "invalid_browser_url",
            "Use a public HTTPS URL without embedded credentials",
        ));
    }
    Ok(url)
}
pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::LauncherGetUpdateSettings {} => {
            let app = app(backend)?;
            let preferences = serialize(
                crate::get_update_preferences(app.clone())
                    .await
                    .map_err(error)?,
            )?;
            let mut result = json!({"channel":crate::resolve_update_channel(&app).await.map_err(error)?,"preferences":preferences,"auto_download_updates":theseus::settings::get().await.map_err(business_error)?.auto_download_updates});
            #[cfg(feature = "updater")]
            {
                let pending =
                    app.state::<crate::updater_impl::PendingUpdateData>();
                let data = pending.0.lock().map_err(error)?;
                result["pending_update"]=json!(data.as_ref().map(|(u,b)|json!({"version":u.version,"downloaded_bytes":b.len()})));
            }
            #[cfg(not(feature = "updater"))]
            {
                result["updater_available"] = json!(false);
            }
            Ok(result)
        }
        Command::LauncherSetUpdateChannel { channel } => {
            crate::set_update_channel(app(backend)?, channel.clone())
                .await
                .map_err(error)?;
            Ok(
                json!({"saved":true,"channel":channel,"applies_to_future_startups":true}),
            )
        }
        Command::LauncherSetUpdatePreferences {
            immediate_update_fetch,
            updates_paused,
            auto_download_updates,
        } => {
            let app = app(backend)?;
            crate::set_update_preferences(
                app.clone(),
                immediate_update_fetch,
                updates_paused,
            )
            .await
            .map_err(error)?;
            if let Some(enabled) = auto_download_updates {
                crate::api::settings::settings_patch(
                    app,
                    json!({"auto_download_updates":enabled}),
                )
                .await
                .map_err(error)?;
            }
            Ok(json!({"saved":true}))
        }
        Command::LauncherCheckUpdate { channel } => {
            #[cfg(feature = "updater")]
            {
                let app = app(backend)?;
                let channel = match channel {
                    Some(c) => c,
                    None => crate::resolve_update_channel(&app)
                        .await
                        .map_err(error)?,
                };
                let view = app
                    .get_webview_window("main")
                    .ok_or_else(|| error("main webview"))?;
                let update = crate::updater_impl::check_app_update(
                    view.as_ref().clone(),
                    channel,
                )
                .await
                .map_err(error)?;
                let Some(update) = update else {
                    return Ok(json!({"available":false}));
                };
                let meta = serialize(update)?;
                let rid = meta["rid"]
                    .as_u64()
                    .ok_or_else(|| error("update resource"))?
                    as u32;
                let version = meta["version"]
                    .as_str()
                    .ok_or_else(|| error("version"))?
                    .to_owned();
                let id = backend.updates.insert(
                    &context.session_id,
                    1,
                    UpdateHandle {
                        rid,
                        version: version.clone(),
                        _lease: Some(std::sync::Arc::new(UpdateResource {
                            view: view.as_ref().clone(),
                            rid,
                        })),
                    },
                )?;
                Ok(
                    json!({"available":true,"update_id":id,"current_version":meta["currentVersion"],"version":version,"published_at":meta["publishedAt"],"force_update":meta["forceUpdate"]}),
                )
            }
            #[cfg(not(feature = "updater"))]
            {
                let _ = channel;
                Err(ToolError::new(
                    "updater_unavailable",
                    "This build has no native updater",
                ))
            }
        }
        Command::LauncherGetUpdateSize { update_id } => {
            #[cfg(feature = "updater")]
            {
                let update =
                    backend.updates.get(&context.session_id, update_id, 1)?;
                let view = app(backend)?
                    .get_webview_window("main")
                    .ok_or_else(|| error("webview"))?;
                let bytes = crate::updater_impl::get_update_size(
                    view.as_ref().clone(),
                    update.rid,
                )
                .await
                .map_err(error)?;
                Ok(json!({"version":update.version,"bytes":bytes}))
            }
            #[cfg(not(feature = "updater"))]
            {
                let _ = update_id;
                Err(ToolError::new(
                    "updater_unavailable",
                    "This build has no native updater",
                ))
            }
        }
        Command::LauncherDownloadUpdate { update_id } => {
            #[cfg(feature = "updater")]
            {
                let app = app(backend)?;
                if app
                    .state::<crate::updater_impl::PendingUpdateData>()
                    .0
                    .lock()
                    .map_err(error)?
                    .is_some()
                {
                    return Err(ToolError::new(
                        "update_already_pending",
                        "Cancel or install pending update before another download",
                    ));
                }
                let update =
                    backend.updates.take(&context.session_id, update_id, 1)?;
                let view = app
                    .get_webview_window("main")
                    .ok_or_else(|| error("webview"))?;
                tokio::time::timeout(
                    std::time::Duration::from_secs(15 * 60),
                    crate::updater_impl::enqueue_update_for_installation(
                        view.as_ref().clone(),
                        update.rid,
                    ),
                )
                .await
                .map_err(error)?
                .map_err(error)?;
                Ok(
                    json!({"downloaded":true,"queued_for_install_on_exit":true,"version":update.version}),
                )
            }
            #[cfg(not(feature = "updater"))]
            {
                let _ = update_id;
                Err(ToolError::new(
                    "updater_unavailable",
                    "This build has no native updater",
                ))
            }
        }
        Command::LauncherCancelUpdate {} => {
            #[cfg(feature = "updater")]
            {
                let app = app(backend)?;
                let view = app
                    .get_webview_window("main")
                    .ok_or_else(|| error("webview"))?;
                crate::updater_impl::remove_enqueued_update(
                    view.as_ref().clone(),
                );
                Ok(json!({"pending_update_cleared":true}))
            }
            #[cfg(not(feature = "updater"))]
            {
                Err(ToolError::new(
                    "updater_unavailable",
                    "This build has no native updater",
                ))
            }
        }
        c @ (Command::LauncherRestart {} | Command::LauncherExit {}) => {
            quiet().await?;
            let restart = matches!(c, Command::LauncherRestart {});
            let app = app(backend)?;
            let _ = app.emit_to(
                "main",
                "mcp-launcher-exiting",
                json!({"restart":restart}),
            );
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1200))
                    .await;
                if quiet().await.is_err() {
                    return;
                }
                let result = if restart {
                    crate::restart_app(app).await
                } else {
                    crate::exit_app(app, false).await
                };
                if let Err(e) = result {
                    tracing::warn!(%e,"Scheduled MCP lifecycle action failed");
                }
            });
            Ok(
                json!({"scheduled":true,"restart":restart,"connection_will_close":true,"grace_period_ms":1200}),
            )
        }
        Command::ShortcutCreateInstance {
            instance_id,
            server_address,
            world_id,
        } => {
            if server_address.is_some() && world_id.is_some() {
                return Err(ToolError::new(
                    "invalid_shortcut",
                    "Choose server or world, not both",
                ));
            }
            let instance = theseus::instance::get(&instance_id)
                .await
                .map_err(business_error)?
                .ok_or_else(|| error("instance"))?;
            let path = crate::api::shortcuts::create_instance_shortcut(
                app(backend)?,
                instance.instance.name,
                instance_id,
                None,
                server_address,
                world_id,
            )
            .await
            .map_err(error)?;
            Ok(
                json!({"created":true,"file_name":path.file_name().map(|s|s.to_string_lossy().into_owned())}),
            )
        }
        Command::SystemOpenPath {
            target_kind,
            target_id,
            path,
            reveal,
        } => {
            let root = paths::root(&target_kind, &target_id, false).await?;
            let check_path = path.clone();
            let root_check = root.clone();
            tokio::task::spawn_blocking(move || {
                let dir = paths::open_root(&root_check)?;
                let (parent, name) = paths::parent(&dir, &check_path)?;
                let m = parent.symlink_metadata(&name).map_err(paths::error)?;
                if m.file_type().is_symlink() {
                    return Err(paths::error("linked path"));
                }
                Ok(())
            })
            .await
            .map_err(error)??;
            let absolute = root.join(path);
            if reveal {
                app(backend)?
                    .opener()
                    .reveal_item_in_dir(absolute)
                    .map_err(error)?;
            } else {
                app(backend)?
                    .opener()
                    .open_path(absolute.to_string_lossy(), None::<&str>)
                    .map_err(error)?;
            }
            Ok(json!({"opened":true,"reveal":reveal}))
        }
        Command::SystemOpenUrl { url } => {
            let url = safe_url(&url)?;
            app(backend)?
                .opener()
                .open_url(url.as_str(), None::<&str>)
                .map_err(error)?;
            Ok(json!({"opened":true}))
        }
        Command::LauncherParseLink { link } => {
            if !link.starts_with("axolotl://") {
                return Err(ToolError::new(
                    "invalid_launcher_link",
                    "Only axolotl:// links are accepted",
                ));
            }
            let payload = theseus::handler::parse_command(&link)
                .await
                .map_err(error)?;
            let mut value = serialize(payload)?;
            if let Some(v) = value.as_object_mut() {
                v.remove("query");
                if let Some(changes) =
                    v.get_mut("changes").and_then(Value::as_array_mut)
                {
                    for change in changes {
                        change["value"] = json!("[redacted]");
                    }
                }
                v.remove("source");
            }
            Ok(value)
        }
        Command::NewsGetMinecraft { limit } => {
            let news = theseus::minecraft_news::get_minecraft_news(
                limit.unwrap_or(12),
            )
            .await
            .map_err(business_error)?;
            Ok(
                json!({"items":news.into_iter().map(|n|json!({"title":n.title.chars().take(512).collect::<String>(),"date":n.date,"url":n.read_more_url,"category":n.category,"tag":n.tag})).collect::<Vec<_>>()}),
            )
        }
        _ => Err(error("command")),
    }
}
