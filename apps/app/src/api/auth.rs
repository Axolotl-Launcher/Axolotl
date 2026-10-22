use crate::api::Result;
use crate::api::oauth_utils;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration as StdDuration;
use tauri::plugin::TauriPlugin;
use tauri::{Emitter, Manager, Runtime, UserAttentionType};
use tauri_plugin_opener::OpenerExt;
use theseus::prelude::*;
use tokio::sync::{mpsc, oneshot};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("auth")
        .invoke_handler(tauri::generate_handler![
            check_reachable,
            check_mojang_services,
            network_scope,
            set_mojang_auth_use_mirror,
            mojang_auth_use_mirror,
            login,
            browser_login,
            begin_device_login,
            poll_device_login,
            begin_yggdrasil_login,
            finish_yggdrasil_login,
            list_yggdrasil_saved_logins,
            get_yggdrasil_password,
            set_yggdrasil_password,
            delete_yggdrasil_password,
            add_offline_user,
            remove_user,
            get_default_user,
            set_default_user,
            get_users,
            get_anti_piracy_status,
            clear_official_login_marker,
        ])
        .build()
}

/// Checks if the authentication servers are reachable.
#[tauri::command]
pub async fn check_reachable() -> Result<()> {
    minecraft_auth::check_reachable().await?;
    Ok(())
}

/// Checks all Mojang services that the Fallen proxy mirrors.
#[tauri::command]
pub async fn check_mojang_services(
    use_mirror: Option<bool>,
) -> Result<Vec<minecraft_auth::MojangServiceStatus>> {
    Ok(
        minecraft_auth::check_mojang_services_for(use_mirror.unwrap_or(false))
            .await,
    )
}

#[tauri::command]
pub async fn network_scope() -> Result<String> {
    Ok(State::get().await?.network_scope())
}

/// Stores whether the launcher should route Mojang service requests through
/// the Fallen proxy.
#[tauri::command]
pub async fn set_mojang_auth_use_mirror(
    use_mirror: bool,
    expected_source: theseus::settings::DownloadSourceMode,
) -> Result<bool> {
    Ok(
        minecraft_auth::set_mojang_auth_use_mirror(use_mirror, expected_source)
            .await?,
    )
}

/// Whether the launcher currently routes Mojang service requests through the
/// Fallen proxy.
#[tauri::command]
pub fn mojang_auth_use_mirror() -> bool {
    minecraft_auth::mojang_auth_use_mirror()
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftLoginTroubleLinks {
    trouble: String,
    browser_login: String,
    device_code: String,
}

enum MinecraftLoginAlternative {
    Browser,
    DeviceCode,
}

async fn notify_anti_piracy_status<R: Runtime>(app: &tauri::AppHandle<R>) {
    let status = theseus::anti_piracy::status().await;
    if let Err(error) = app.emit("anti-piracy-status-changed", status) {
        tracing::warn!(%error, "Could not notify frontend of official Minecraft login");
    }
}

#[tauri::command]
pub async fn login<R: Runtime>(
    app: tauri::AppHandle<R>,
    trouble_links: MinecraftLoginTroubleLinks,
) -> Result<Option<Credentials>> {
    let flow = minecraft_auth::begin_login().await?;

    let start = Utc::now();

    if let Some(window) = app.get_webview_window("signin") {
        window.close()?;
    }

    let (alternative_tx, mut alternative_rx) = mpsc::unbounded_channel();
    let trouble_links =
        serde_json::to_string(&trouble_links).map_err(|error| {
            theseus::ErrorKind::OtherError(format!(
                "Failed to serialize Minecraft sign-in help links: {error}"
            ))
            .as_error()
        })?;
    let help_bar_script = format!(
        r#"
        (() => {{
            const labels = {trouble_links};
            const addHelpBar = () => {{
                if (document.getElementById('axolotl-minecraft-login-help')) return;

                const bar = document.createElement('div');
                bar.id = 'axolotl-minecraft-login-help';
                bar.style.cssText = 'box-sizing:border-box;position:sticky;top:0;z-index:2147483647;display:flex;align-items:center;gap:8px;width:100%;min-height:36px;padding:8px 16px;background:#fff;color:#1f1f1f;border-bottom:1px solid #d1d1d1;font:13px/20px system-ui,sans-serif;';
                const trouble = document.createElement('span');
                trouble.textContent = labels.trouble;
                bar.append(trouble);

                const createLink = (label, destination) => {{
                    const link = document.createElement('a');
                    link.href = destination;
                    link.textContent = label;
                    link.style.cssText = 'color:#0067b8;text-decoration:underline;cursor:pointer;';
                    return link;
                }};

                bar.append(createLink(labels.browserLogin, 'axolotl-auth://browser'));
                const separator = document.createElement('span');
                separator.textContent = '|';
                bar.append(separator);
                bar.append(createLink(labels.deviceCode, 'axolotl-auth://device-code'));
                document.body.prepend(bar);
            }};

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', addHelpBar, {{ once: true }});
            }} else {{
                addHelpBar();
            }}
        }})();
        "#,
    );
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "signin",
        tauri::WebviewUrl::External(flow.auth_request_uri.parse().map_err(
            |_| {
                theseus::ErrorKind::OtherError(
                    "Error parsing auth redirect URL".to_string(),
                )
                .as_error()
            },
        )?),
    )
    .title("Sign into Axolotl Launcher")
    .always_on_top(true)
    .center()
    .initialization_script(help_bar_script)
    .on_navigation(move |url| {
        let alternative = match url.host_str() {
            Some("browser") if url.scheme() == "axolotl-auth" => {
                Some(MinecraftLoginAlternative::Browser)
            }
            Some("device-code") if url.scheme() == "axolotl-auth" => {
                Some(MinecraftLoginAlternative::DeviceCode)
            }
            _ => None,
        };

        if let Some(alternative) = alternative {
            let _ = alternative_tx.send(alternative);
            return false;
        }

        true
    })
    .build()?;

    window.request_user_attention(Some(UserAttentionType::Critical))?;

    while (Utc::now() - start) < Duration::minutes(10) {
        tokio::select! {
            Some(alternative) = alternative_rx.recv() => {
                window.close()?;
                return match alternative {
                    MinecraftLoginAlternative::Browser => browser_login(app).await,
                    MinecraftLoginAlternative::DeviceCode => {
                        app.emit("minecraft-device-login-requested", ())?;
                        Ok(None)
                    }
                };
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
        }

        if window.title().is_err() {
            // user closed window, cancelling flow
            return Ok(None);
        }

        let callback_url = window.url()?;
        if callback_url
            .as_str()
            .starts_with("https://login.live.com/oauth20_desktop.srf")
            && let Some((_, code)) =
                callback_url.query_pairs().find(|x| x.0 == "code")
        {
            let state = callback_url
                .query_pairs()
                .find(|x| x.0 == "state")
                .map(|(_, state)| state.into_owned())
                .ok_or_else(|| {
                    theseus::ErrorKind::InputError(
                        "Microsoft sign-in response did not include state"
                            .into(),
                    )
                    .as_error()
                })?;
            window.close()?;
            let val = minecraft_auth::finish_login(&code, &state, flow).await?;

            notify_anti_piracy_status(&app).await;

            return Ok(Some(val));
        }
    }

    window.close()?;
    Ok(None)
}

#[tauri::command]
pub async fn browser_login<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<Credentials>> {
    let (listen_socket_tx, listen_socket) = oneshot::channel();
    let callback_address =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 53682);
    let auth_code = tokio::spawn(oauth_utils::auth_code_reply::listen_fixed(
        callback_address,
        listen_socket_tx,
    ));
    listen_socket.await.unwrap()?;

    let flow = minecraft_auth::begin_browser_login().await?;
    if let Err(error) =
        app.opener().open_url(&flow.auth_request_uri, None::<&str>)
    {
        oauth_utils::auth_code_reply::stop_listeners();
        return Err(crate::api::TheseusSerializableError::Theseus(
            theseus::ErrorKind::OtherError(format!(
                "Failed to open browser sign-in: {error}"
            ))
            .into(),
        ));
    }

    let auth_code =
        tokio::time::timeout(StdDuration::from_secs(10 * 60), auth_code)
            .await
            .map_err(|_| {
                oauth_utils::auth_code_reply::stop_listeners();
                theseus::ErrorKind::OtherError(
                    "Browser sign-in timed out".into(),
                )
                .as_error()
            })?;
    let auth_code = auth_code.map_err(|error| {
        theseus::ErrorKind::OtherError(format!(
            "Browser sign-in listener stopped unexpectedly: {error}"
        ))
        .as_error()
    })?;
    let Some(reply) = auth_code? else {
        return Ok(None);
    };

    let state = reply.state.ok_or_else(|| {
        theseus::ErrorKind::InputError(
            "Microsoft sign-in response did not include state".into(),
        )
        .as_error()
    })?;
    let credentials =
        minecraft_auth::finish_login(&reply.code, &state, flow).await?;
    notify_anti_piracy_status(&app).await;
    if let Some(main_window) = app.get_webview_window("main") {
        main_window.set_focus().ok();
    }

    Ok(Some(credentials))
}

#[tauri::command]
pub async fn begin_device_login()
-> Result<minecraft_auth::MinecraftDeviceLoginFlow> {
    Ok(minecraft_auth::begin_device_login().await?)
}

#[tauri::command]
pub async fn poll_device_login<R: Runtime>(
    app: tauri::AppHandle<R>,
    device_code: String,
) -> Result<minecraft_auth::MinecraftDeviceLoginPoll> {
    let result = minecraft_auth::poll_device_login(&device_code).await?;
    if matches!(
        result,
        minecraft_auth::MinecraftDeviceLoginPoll::Complete { .. }
    ) {
        notify_anti_piracy_status(&app).await;
    }
    Ok(result)
}

#[tauri::command]
pub async fn begin_yggdrasil_login(
    api_root: String,
    login: String,
    password: String,
) -> Result<minecraft_auth::YggdrasilLoginResult> {
    Ok(
        minecraft_auth::begin_yggdrasil_login(&api_root, &login, &password)
            .await?,
    )
}

#[tauri::command]
pub async fn finish_yggdrasil_login(
    flow_id: uuid::Uuid,
    profile_id: uuid::Uuid,
) -> Result<Credentials> {
    Ok(minecraft_auth::finish_yggdrasil_login(flow_id, profile_id).await?)
}

const YGGDRASIL_SAVED_LOGINS_KEY: &str = "yggdrasil-saved-logins";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SavedYggdrasilLogin {
    pub api_root: String,
    pub login: String,
}

#[tauri::command]
pub fn list_yggdrasil_saved_logins() -> Result<Vec<SavedYggdrasilLogin>> {
    read_yggdrasil_saved_logins()
}

#[tauri::command]
pub fn get_yggdrasil_password(
    api_root: String,
    login: String,
) -> Result<Option<String>> {
    let entry = yggdrasil_password_entry(&api_root, &login)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_error(error)),
    }
}

#[tauri::command]
pub fn set_yggdrasil_password(
    api_root: String,
    login: String,
    password: String,
) -> Result<()> {
    if password.is_empty() {
        return delete_yggdrasil_password(api_root, login);
    }
    let saved_login = normalize_saved_yggdrasil_login(&api_root, &login)?;
    yggdrasil_password_entry(&saved_login.api_root, &saved_login.login)?
        .set_password(&password)
        .map_err(keyring_error)?;

    let mut saved_logins = read_yggdrasil_saved_logins()?;
    upsert_saved_yggdrasil_login(&mut saved_logins, saved_login);
    write_yggdrasil_saved_logins(&saved_logins)
}

#[tauri::command]
pub fn delete_yggdrasil_password(
    api_root: String,
    login: String,
) -> Result<()> {
    let saved_login = normalize_saved_yggdrasil_login(&api_root, &login)?;
    match yggdrasil_password_entry(&saved_login.api_root, &saved_login.login)?
        .delete_credential()
    {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(error) => return Err(keyring_error(error)),
    }

    let mut saved_logins = read_yggdrasil_saved_logins()?;
    remove_saved_yggdrasil_login(&mut saved_logins, &saved_login);
    write_yggdrasil_saved_logins(&saved_logins)
}

fn yggdrasil_password_entry(
    api_root: &str,
    login: &str,
) -> Result<keyring::Entry> {
    let login = login.trim();
    if login.is_empty() {
        return Err(theseus::ErrorKind::InputError(
            "The Yggdrasil account name cannot be empty".to_string(),
        )
        .as_error()
        .into());
    }
    let api_root = minecraft_auth::normalize_yggdrasil_api_root(api_root)?;
    keyring::Entry::new(
        theseus::brand::BUNDLE_IDENTIFIER,
        &format!("{api_root}\n{login}"),
    )
    .map_err(keyring_error)
}

fn normalize_saved_yggdrasil_login(
    api_root: &str,
    login: &str,
) -> Result<SavedYggdrasilLogin> {
    let login = login.trim();
    if login.is_empty() {
        return Err(theseus::ErrorKind::InputError(
            "The Yggdrasil account name cannot be empty".to_string(),
        )
        .as_error()
        .into());
    }
    Ok(SavedYggdrasilLogin {
        api_root: minecraft_auth::normalize_yggdrasil_api_root(api_root)?,
        login: login.to_string(),
    })
}

fn yggdrasil_saved_logins_entry() -> Result<keyring::Entry> {
    keyring::Entry::new(
        theseus::brand::BUNDLE_IDENTIFIER,
        YGGDRASIL_SAVED_LOGINS_KEY,
    )
    .map_err(keyring_error)
}

fn read_yggdrasil_saved_logins() -> Result<Vec<SavedYggdrasilLogin>> {
    match yggdrasil_saved_logins_entry()?.get_password() {
        Ok(saved_logins) => match serde_json::from_str(&saved_logins) {
            Ok(saved_logins) => Ok(saved_logins),
            Err(error) => {
                tracing::warn!(
                    "Ignoring an invalid saved Yggdrasil login index: {error}"
                );
                Ok(Vec::new())
            }
        },
        Err(keyring::Error::NoEntry) => Ok(Vec::new()),
        Err(error) => Err(keyring_error(error)),
    }
}

fn write_yggdrasil_saved_logins(
    saved_logins: &[SavedYggdrasilLogin],
) -> Result<()> {
    let entry = yggdrasil_saved_logins_entry()?;
    if saved_logins.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error(error)),
        };
    }

    let saved_logins =
        serde_json::to_string(saved_logins).map_err(|error| {
            theseus::ErrorKind::OtherError(format!(
                "Unable to serialize saved Yggdrasil logins: {error}"
            ))
            .as_error()
        })?;
    entry.set_password(&saved_logins).map_err(keyring_error)
}

fn upsert_saved_yggdrasil_login(
    saved_logins: &mut Vec<SavedYggdrasilLogin>,
    saved_login: SavedYggdrasilLogin,
) {
    saved_logins.retain(|entry| entry != &saved_login);
    saved_logins.push(saved_login);
    saved_logins.sort_by(|left, right| {
        left.login
            .cmp(&right.login)
            .then_with(|| left.api_root.cmp(&right.api_root))
    });
}

fn remove_saved_yggdrasil_login(
    saved_logins: &mut Vec<SavedYggdrasilLogin>,
    saved_login: &SavedYggdrasilLogin,
) {
    saved_logins.retain(|entry| entry != saved_login);
}

fn keyring_error(
    error: keyring::Error,
) -> crate::api::TheseusSerializableError {
    theseus::ErrorKind::OtherError(format!(
        "Unable to access the system credential store: {error}"
    ))
    .as_error()
    .into()
}

fn parse_custom_uuid(uuid: Option<String>) -> Result<Option<uuid::Uuid>> {
    let Some(uuid) = uuid else {
        return Ok(None);
    };
    let uuid = uuid.trim().replace('-', "");
    if uuid.len() != 32
        || !uuid.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err(theseus::ErrorKind::InputError(
            "Custom UUID must be 32 hexadecimal characters; hyphens are optional"
                .to_string(),
        )
        .as_error()
        .into());
    }

    Ok(Some(uuid::Uuid::parse_str(&uuid).map_err(|_| {
        theseus::ErrorKind::InputError("Invalid custom UUID".to_string())
            .as_error()
    })?))
}

#[tauri::command]
pub async fn add_offline_user(
    username: String,
    uuid: Option<String>,
) -> Result<Credentials> {
    Ok(
        minecraft_auth::add_offline_user(&username, parse_custom_uuid(uuid)?)
            .await?,
    )
}

#[tauri::command]
pub async fn remove_user(user: String) -> Result<()> {
    Ok(minecraft_auth::remove_user(&user).await?)
}

#[tauri::command]
pub async fn get_default_user(offline_mode: bool) -> Result<Option<String>> {
    Ok(minecraft_auth::get_default_user(offline_mode).await?)
}

#[tauri::command]
pub async fn set_default_user(user: String) -> Result<()> {
    Ok(minecraft_auth::set_default_user(&user).await?)
}

/// Get a copy of the list of all user credentials
#[tauri::command]
pub async fn get_users(
    offline_mode: bool,
    cached_only: Option<bool>,
) -> Result<Vec<minecraft_auth::MinecraftUser>> {
    Ok(minecraft_auth::users_with_cache(
        offline_mode,
        cached_only.unwrap_or(false),
    )
    .await?)
}

#[tauri::command]
pub async fn get_anti_piracy_status() -> theseus::anti_piracy::Status {
    theseus::anti_piracy::status().await
}

#[tauri::command]
pub async fn clear_official_login_marker<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<theseus::anti_piracy::Status> {
    theseus::anti_piracy::clear_official_login().await?;
    let status = theseus::anti_piracy::status().await;
    if let Err(error) = app.emit("anti-piracy-status-changed", status) {
        tracing::warn!(%error, "Could not notify frontend of cleared official Minecraft login");
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_login(api_root: &str, login: &str) -> SavedYggdrasilLogin {
        SavedYggdrasilLogin {
            api_root: api_root.to_string(),
            login: login.to_string(),
        }
    }

    #[test]
    fn saved_login_index_upserts_and_sorts_entries() {
        let mut saved_logins =
            vec![saved_login("https://example.com", "second")];
        upsert_saved_yggdrasil_login(
            &mut saved_logins,
            saved_login("https://example.com", "first"),
        );
        upsert_saved_yggdrasil_login(
            &mut saved_logins,
            saved_login("https://example.com", "second"),
        );

        assert_eq!(saved_logins.len(), 2);
        assert_eq!(saved_logins[0].login, "first");
        assert_eq!(saved_logins[1].login, "second");
    }

    #[test]
    fn saved_login_index_removes_only_matching_entry() {
        let removed =
            saved_login("https://first.example", "player@example.com");
        let retained =
            saved_login("https://second.example", "player@example.com");
        let mut saved_logins = vec![removed.clone(), retained.clone()];

        remove_saved_yggdrasil_login(&mut saved_logins, &removed);

        assert_eq!(saved_logins, vec![retained]);
    }

    #[test]
    fn parses_custom_offline_uuid() {
        let expected =
            uuid::Uuid::parse_str("b50ad385-829d-3141-a216-7e7d7539ca7f")
                .unwrap();

        assert_eq!(
            parse_custom_uuid(Some(
                "b50ad385829d3141a2167e7d7539ca7f".to_string()
            ))
            .unwrap(),
            Some(expected)
        );
        assert_eq!(
            parse_custom_uuid(Some(
                "B50AD385-829D-3141-A216-7E7D7539CA7F".to_string()
            ))
            .unwrap(),
            Some(expected)
        );
        assert_eq!(parse_custom_uuid(None).unwrap(), None);
        assert!(parse_custom_uuid(Some("not-a-uuid".to_string())).is_err());
        assert!(
            parse_custom_uuid(Some(
                "b50ad385829d3141a2167e7d7539ca7".to_string()
            ))
            .is_err()
        );
    }
}
