use eyre::{Context, bail};
use flate2::read::GzDecoder;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use self::binary::{
    cleanup_legacy_versions, find_terracotta_executable,
    install_terracotta_binary, is_terracotta_executable,
    resolve_installed_terracotta_binary_path, resolve_terracotta_binary_path,
    terracotta_binary_name, terracotta_binary_path,
    validate_terracotta_version, versioned_terracotta_binary_name,
};
pub use self::binary::{terracotta_download_urls, terracotta_platform_key};
use self::lan::MinecraftLanAnnouncer;
use super::data::Credentials;

mod binary;
mod lan;

const MAX_TERRACOTTA_ARCHIVE_SIZE: u64 = 256 * 1024 * 1024;
const MAX_TERRACOTTA_DIAGNOSTIC_LINES: usize = 2_000;
const MAX_CONSECUTIVE_POLL_FAILURES: u8 = 3;
const TERRACOTTA_REQUEST_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(10);

static TERRACOTTA_STATE: LazyLock<Mutex<TerracottaState>> =
    LazyLock::new(|| Mutex::new(TerracottaState::default()));

static TERRACOTTA_RUNTIME: LazyLock<Mutex<TerracottaRuntime>> =
    LazyLock::new(|| Mutex::new(TerracottaRuntime::default()));

static TERRACOTTA_OPERATION: LazyLock<Mutex<()>> =
    LazyLock::new(|| Mutex::new(()));

static TERRACOTTA_DIAGNOSTIC_OUTPUT: LazyLock<
    std::sync::Mutex<VecDeque<String>>,
> = LazyLock::new(|| std::sync::Mutex::new(VecDeque::new()));

fn record_terracotta_output(pid: u32, stream: &str, text: &str) {
    let line = format!(
        "{} pid={pid} {stream}: {text}",
        chrono::Local::now().to_rfc3339()
    );
    if let Ok(mut output) = TERRACOTTA_DIAGNOSTIC_OUTPUT.lock() {
        output.push_back(line);
        while output.len() > MAX_TERRACOTTA_DIAGNOSTIC_LINES {
            output.pop_front();
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TerracottaState {
    pub status: TerracottaStatus,
    pub http_port: Option<u16>,
    pub room_code: Option<String>,
    pub server_port: Option<u16>,
    pub players: Vec<PlayerInfo>,
    pub download_progress: Option<u8>,
    pub download_stage: Option<TerracottaDownloadStage>,
    pub binary_installed: bool,
    pub installed_version: Option<String>,
    pub error_type: Option<TerracottaErrorType>,
    pub error_message: Option<String>,
    pub profile_index: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TerracottaStatus {
    #[default]
    Idle,
    Starting,
    Downloading,
    Waiting,
    HostScanning,
    HostStarting,
    HostReady,
    GuestConnecting,
    GuestStarting,
    GuestReady,
    Error,
    Fatal,
}

impl TerracottaStatus {
    fn from_api(value: &str) -> Option<Self> {
        match value {
            "idle" => Some(Self::Idle),
            "starting" => Some(Self::Starting),
            "waiting" => Some(Self::Waiting),
            "host-scanning" => Some(Self::HostScanning),
            "host-starting" => Some(Self::HostStarting),
            "host-ok" => Some(Self::HostReady),
            "guest-connecting" => Some(Self::GuestConnecting),
            "guest-starting" => Some(Self::GuestStarting),
            "guest-ok" => Some(Self::GuestReady),
            "exception" => Some(Self::Error),
            "fatal" => Some(Self::Fatal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerracottaDownloadStage {
    Preparing,
    Downloading,
    Verifying,
    Extracting,
    Installing,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerracottaErrorType {
    Os,
    Network,
    Install,
    Terracotta,
    Unknown,
}

impl From<i32> for TerracottaErrorType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Os,
            1 => Self::Network,
            2 => Self::Install,
            3 => Self::Terracotta,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub machine_id: String,
    pub name: String,
    pub vendor: String,
    pub kind: TerracottaPlayerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TerracottaPlayerKind {
    Host,
    Guest,
    Unknown,
}

impl From<&str> for TerracottaPlayerKind {
    fn from(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "HOST" => Self::Host,
            "GUEST" => Self::Guest,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerracottaMeta {
    pub version: String,
    pub compile_timestamp: String,
    pub easytier_version: String,
    pub yggdrasil_port: u16,
    pub target_tuple: String,
    pub target_os: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TerracottaUpdate {
    pub installed_version: Option<String>,
    pub latest_version: String,
    pub update_available: bool,
}

#[derive(Deserialize, Serialize)]
struct InstalledTerracottaInfo {
    version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TerracottaApiState {
    state: String,
    #[serde(default)]
    index: Option<u32>,
    #[serde(default)]
    room: Option<String>,
    #[serde(default)]
    profile_index: Option<u32>,
    #[serde(default)]
    profiles: Option<Vec<TerracottaApiProfile>>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    r#type: Option<i32>,
    #[serde(default)]
    difficulty: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TerracottaApiProfile {
    machine_id: String,
    name: String,
    vendor: String,
    kind: String,
}

struct TerracottaProcess {
    child: Child,
    output: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Default)]
struct TerracottaRuntime {
    process: Option<TerracottaProcess>,
    state_poller: Option<JoinHandle<()>>,
    lan_announcer: MinecraftLanAnnouncer,
}

impl Drop for TerracottaProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn capture_terracotta_output<R>(
    reader: R,
    stream: &'static str,
    pid: u32,
    output: Arc<Mutex<VecDeque<String>>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(text)) = lines.next_line().await {
            tracing::info!(pid, stream, "terracotta process: {text}");
            record_terracotta_output(pid, stream, &text);
            let mut captured = output.lock().await;
            captured.push_back(format!("{stream}: {text}"));
            if captured.len() > 50 {
                captured.pop_front();
            }
        }
    });
}

async fn take_terminated_terracotta_process() -> eyre::Result<
    Option<(std::process::ExitStatus, Arc<Mutex<VecDeque<String>>>)>,
> {
    let mut runtime = TERRACOTTA_RUNTIME.lock().await;
    let Some(process) = runtime.process.as_mut() else {
        return Ok(None);
    };
    let Some(status) = process
        .child
        .try_wait()
        .wrap_err("failed to inspect terracotta process")?
    else {
        return Ok(None);
    };
    let output = process.output.clone();
    runtime.process.take();
    Ok(Some((status, output)))
}

async fn format_terracotta_exit(
    status: std::process::ExitStatus,
    output: Arc<Mutex<VecDeque<String>>>,
) -> String {
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    let captured = output.lock().await;
    let details = captured.iter().cloned().collect::<Vec<_>>().join("\n");
    if details.is_empty() {
        format!("terracotta exited before startup completed ({status})")
    } else {
        format!(
            "terracotta exited before startup completed ({status}):\n{details}"
        )
    }
}

#[cfg(test)]
fn reported_terracotta_port(output: &str) -> Option<u16> {
    output.lines().find_map(|line| {
        let (_, port) = line.rsplit_once("port = ")?;
        port.trim().parse().ok()
    })
}

#[cfg(test)]
fn terracotta_startup_arguments(target_os: &str) -> &'static [&'static str] {
    if target_os == "macos" {
        &["--daemon"]
    } else {
        &[]
    }
}

async fn get_latest_terracotta_version() -> eyre::Result<String> {
    #[derive(Deserialize)]
    struct ReleaseInfo {
        tag_name: String,
    }

    let endpoints = [
        "https://gitee.com/api/v5/repos/burningtnt/Terracotta/releases/latest",
        "https://api.github.com/repos/burningtnt/Terracotta/releases/latest",
    ];
    let mut failures = Vec::new();

    for endpoint in endpoints {
        let response = crate::util::fetch::INSECURE_REQWEST_CLIENT
            .get(endpoint)
            .header("Accept", "application/json")
            .header("User-Agent", crate::launcher_user_agent())
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                match response.json::<ReleaseInfo>().await {
                    Ok(info) => {
                        return Ok(info
                            .tag_name
                            .trim_start_matches('v')
                            .to_string());
                    }
                    Err(error) => failures.push(format!("{endpoint}: {error}")),
                }
            }
            Ok(response) => {
                failures.push(format!("{endpoint}: HTTP {}", response.status()))
            }
            Err(error) => failures.push(format!("{endpoint}: {error}")),
        }
    }

    bail!(
        "failed to fetch latest terracotta release info: {}",
        failures.join("; ")
    )
}

fn terracotta_version_file() -> PathBuf {
    terracotta_binary_path().with_file_name("terracotta-version.json")
}

fn installed_terracotta_version() -> Option<String> {
    if !is_binary_installed() {
        return None;
    }

    let bytes = std::fs::read(terracotta_version_file()).ok()?;
    let info =
        serde_json::from_slice::<InstalledTerracottaInfo>(&bytes).ok()?;
    validate_terracotta_version(&info.version).ok()?;
    Some(info.version)
}

async fn persist_installed_terracotta_version(
    version: &str,
) -> eyre::Result<()> {
    let version_file = terracotta_version_file();
    let temporary_file =
        version_file.with_file_name("terracotta-version.json.tmp");
    let contents = serde_json::to_vec(&InstalledTerracottaInfo {
        version: version.to_string(),
    })?;

    tokio::fs::write(&temporary_file, contents)
        .await
        .wrap_err("failed to write terracotta version metadata")?;
    tokio::fs::rename(&temporary_file, &version_file)
        .await
        .wrap_err("failed to install terracotta version metadata")?;
    Ok(())
}

fn update_is_available(
    installed_version: Option<&str>,
    latest_version: &str,
) -> bool {
    installed_version != Some(latest_version)
}

pub async fn check_for_update() -> eyre::Result<TerracottaUpdate> {
    let latest_version = get_latest_terracotta_version().await?;
    let installed_version = installed_terracotta_version();
    Ok(TerracottaUpdate {
        update_available: update_is_available(
            installed_version.as_deref(),
            &latest_version,
        ),
        installed_version,
        latest_version,
    })
}

async fn download_terracotta_inner(
    version: Option<String>,
) -> eyre::Result<()> {
    info!(requested_version = ?version, "starting terracotta download");
    {
        let mut state = TERRACOTTA_STATE.lock().await;
        state.status = TerracottaStatus::Downloading;
        state.download_progress = Some(0);
        state.download_stage = Some(TerracottaDownloadStage::Preparing);
        state.error_type = None;
        state.error_message = None;
    }

    let version = match version {
        Some(v) => v,
        None => get_latest_terracotta_version().await?,
    };
    validate_terracotta_version(&version)?;

    let platform = terracotta_platform_key();
    info!(version, platform, "resolved terracotta download");
    if platform == "unsupported" {
        bail!(
            "no terracotta binary available for {}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }

    let urls = terracotta_download_urls(&version, platform);
    let mut archive_data: Option<Vec<u8>> = None;

    for url in &urls {
        info!("attempting to download terracotta from {url}");

        {
            let mut state = TERRACOTTA_STATE.lock().await;
            state.download_stage = Some(TerracottaDownloadStage::Downloading);
        }

        match crate::util::fetch::INSECURE_REQWEST_CLIENT
            .get(url)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let total_size = response.content_length().unwrap_or(0);
                if total_size > MAX_TERRACOTTA_ARCHIVE_SIZE {
                    warn!(
                        "terracotta archive from {url} is too large: {total_size} bytes"
                    );
                    continue;
                }
                let mut downloaded: u64 = 0;
                let mut stream = response.bytes_stream();
                let mut data = Vec::with_capacity(total_size as usize);
                let mut hasher = Sha512::new();
                let mut stream_error = None;

                while let Some(chunk) = stream.next().await {
                    let chunk = match chunk {
                        Ok(chunk) => chunk,
                        Err(error) => {
                            stream_error = Some(error.to_string());
                            break;
                        }
                    };
                    hasher.update(&chunk);
                    data.extend_from_slice(&chunk);
                    downloaded += chunk.len() as u64;
                    if downloaded > MAX_TERRACOTTA_ARCHIVE_SIZE {
                        stream_error = Some(format!(
                            "archive exceeds {MAX_TERRACOTTA_ARCHIVE_SIZE} bytes"
                        ));
                        break;
                    }

                    if total_size > 0 {
                        let pct = ((downloaded as f64 / total_size as f64)
                            * 100.0)
                            .clamp(0.0, 100.0)
                            as u8;
                        let mut state = TERRACOTTA_STATE.lock().await;
                        state.download_progress = Some(pct);
                    }
                }
                if let Some(error) = stream_error {
                    warn!("download from {url} failed: {error}");
                    continue;
                }

                info!(
                    url,
                    downloaded_bytes = downloaded,
                    "downloaded terracotta archive"
                );

                let computed_hash = format!("{:x}", hasher.finalize());

                {
                    let mut state = TERRACOTTA_STATE.lock().await;
                    state.download_stage =
                        Some(TerracottaDownloadStage::Verifying);
                }

                let hash_url = format!("{url}.sha512");
                match crate::util::fetch::INSECURE_REQWEST_CLIENT
                    .get(&hash_url)
                    .send()
                    .await
                {
                    Ok(hash_resp) if hash_resp.status().is_success() => {
                        let checksum = hash_resp
                            .text()
                            .await
                            .unwrap_or_default()
                            .trim()
                            .to_string();
                        let expected_hash = checksum.split_whitespace().next();
                        if expected_hash.is_none_or(|hash| {
                            hash.len() != 128
                                || !hash.chars().all(|character| {
                                    character.is_ascii_hexdigit()
                                })
                                || !computed_hash.eq_ignore_ascii_case(hash)
                        }) {
                            warn!(
                                "SHA-512 mismatch for terracotta archive from {url}: \
expected {expected_hash:?}, computed {computed_hash}"
                            );
                            continue;
                        }
                        info!(
                            "SHA-512 verification passed for terracotta archive"
                        );
                    }
                    _ => {
                        warn!(
                            "no SHA-512 checksum available at {hash_url}, \
skipping verification"
                        );
                    }
                }

                archive_data = Some(data);
                break;
            }
            Ok(response) => {
                warn!(
                    "download from {url} returned HTTP {}",
                    response.status()
                );
            }
            Err(e) => {
                warn!("download from {url} failed: {e}");
            }
        }
    }

    let archive_data = archive_data.ok_or_else(|| {
        eyre::eyre!("all download sources failed for terracotta v{version}")
    })?;

    {
        let mut state = TERRACOTTA_STATE.lock().await;
        state.download_stage = Some(TerracottaDownloadStage::Extracting);
        state.download_progress = None;
    }

    let target_dir = terracotta_binary_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("terracotta"));

    tokio::fs::create_dir_all(&target_dir)
        .await
        .wrap_err("failed to create terracotta directory")?;

    let staging_dir = tempfile::Builder::new()
        .prefix("terracotta-install-")
        .tempdir_in(&target_dir)
        .wrap_err(
            "failed to create terracotta installation staging directory",
        )?;
    let staging_path = staging_dir.path().to_path_buf();
    let archive_data_clone = archive_data;

    tokio::task::spawn_blocking(move || -> eyre::Result<()> {
        let decoder = GzDecoder::new(&archive_data_clone[..]);
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(&staging_path)
            .wrap_err("failed to extract terracotta archive")?;
        Ok(())
    })
    .await??;

    {
        let mut state = TERRACOTTA_STATE.lock().await;
        state.download_stage = Some(TerracottaDownloadStage::Installing);
    }

    let expected_path = target_dir.join(terracotta_binary_name());
    let versioned_name = versioned_terracotta_binary_name(&version, platform);
    let candidate =
        find_terracotta_executable(staging_dir.path(), Some(&versioned_name))
            .ok_or_else(|| {
            eyre::eyre!(
                "no valid terracotta executable found in downloaded archive"
            )
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&candidate)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&candidate, permissions)?;
    }

    install_terracotta_binary(&candidate, &expected_path).await?;
    if !is_terracotta_executable(&expected_path) {
        bail!("installed terracotta binary failed executable validation");
    }
    persist_installed_terracotta_version(&version).await?;
    info!(
        "installed {} as {}",
        candidate.display(),
        expected_path.display()
    );

    cleanup_legacy_versions(&version).await?;

    info!(
        "terracotta v{version} installed to {}",
        target_dir.display()
    );

    let mut state = TERRACOTTA_STATE.lock().await;
    state.download_progress = Some(100);
    state.download_stage = Some(TerracottaDownloadStage::Complete);
    drop(state);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let mut state = TERRACOTTA_STATE.lock().await;
    state.status = TerracottaStatus::Idle;
    state.download_progress = None;
    state.download_stage = None;
    Ok(())
}

async fn run_terracotta_download(version: Option<String>) -> eyre::Result<()> {
    let result = download_terracotta_inner(version).await;
    if let Err(error) = &result {
        warn!("terracotta download failed: {error:#}");
        let mut state = TERRACOTTA_STATE.lock().await;
        state.status = TerracottaStatus::Error;
        state.download_progress = None;
        state.download_stage = None;
        state.error_type = Some(TerracottaErrorType::Install);
        state.error_message = Some(format!("{error:#}"));
    }
    result
}

pub async fn download_terracotta(version: Option<String>) -> eyre::Result<()> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    if TERRACOTTA_STATE.lock().await.http_port.is_some() {
        bail!(
            "cannot replace terracotta while the multiplayer service is running"
        );
    }
    run_terracotta_download(version).await
}

pub async fn update_terracotta() -> eyre::Result<TerracottaUpdate> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    if TERRACOTTA_STATE.lock().await.http_port.is_some() {
        bail!(
            "cannot update terracotta while the multiplayer service is running"
        );
    }

    let latest_version = get_latest_terracotta_version().await?;
    let installed_version = installed_terracotta_version();
    if update_is_available(installed_version.as_deref(), &latest_version) {
        run_terracotta_download(Some(latest_version.clone())).await?;
    }

    Ok(TerracottaUpdate {
        installed_version: installed_terracotta_version(),
        latest_version,
        update_available: false,
    })
}

fn terracotta_client() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(2))
            .timeout(TERRACOTTA_REQUEST_TIMEOUT)
            .build()
            .expect("terracotta client should build")
    });
    &CLIENT
}

async fn terracotta_get<T: serde::de::DeserializeOwned>(
    port: u16,
    path: &str,
) -> eyre::Result<T> {
    let client = terracotta_client();
    let url = format!("http://127.0.0.1:{port}{path}");
    let resp = client.get(&url).send().await.wrap_err_with(|| {
        format!("failed to connect to terracotta at {url}")
    })?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .wrap_err("failed to read terracotta response")?;
    if !status.is_success() {
        bail!(
            "terracotta returned {} for {}: {}",
            status.as_u16(),
            path,
            body
        );
    }
    let parsed: T = serde_json::from_str(&body).wrap_err_with(|| {
        format!("failed to parse terracotta response for {path}: {body}")
    })?;
    Ok(parsed)
}

fn terracotta_server_port(url: Option<&str>) -> Option<u16> {
    url?.parse::<std::net::SocketAddr>()
        .ok()
        .map(|url| url.port())
}

async fn sync_minecraft_lan_announcer(port: Option<u16>) {
    TERRACOTTA_RUNTIME.lock().await.lan_announcer.sync(port);
}

async fn spawn_terracotta_state_poller(port: u16) {
    let mut runtime = TERRACOTTA_RUNTIME.lock().await;
    if let Some(task) = runtime.state_poller.take() {
        task.abort();
    }
    runtime.state_poller = Some(tokio::spawn(poll_terracotta_state(port)));
}

async fn poll_terracotta_state(port: u16) {
    let mut last_index: u32 = 0;
    let mut consecutive_failures = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match terracotta_get::<TerracottaApiState>(port, "/state").await {
            Ok(api_state) => {
                consecutive_failures = 0;
                let new_index = api_state.index.unwrap_or(0);
                if new_index > 0 && new_index <= last_index {
                    continue;
                }
                last_index = new_index;
                let server_port = if api_state.state == "guest-ok" {
                    terracotta_server_port(api_state.url.as_deref())
                } else {
                    None
                };

                let mut state = TERRACOTTA_STATE.lock().await;
                let previous_status = state.status;
                state.http_port = Some(port);
                state.room_code = api_state.room.clone();
                state.server_port = server_port;
                state.profile_index = api_state.profile_index;

                state.status = TerracottaStatus::from_api(&api_state.state)
                    .unwrap_or_else(|| {
                        warn!("unknown terracotta state: {}", api_state.state);
                        TerracottaStatus::Error
                    });
                if state.status != previous_status {
                    info!(
                        ?previous_status,
                        status = ?state.status,
                        player_count = api_state
                            .profiles
                            .as_ref()
                            .map_or(state.players.len(), Vec::len),
                        has_room_code = api_state.room.is_some(),
                        "terracotta state changed"
                    );
                }

                if TerracottaStatus::from_api(&api_state.state).is_none() {
                    state.error_type = Some(TerracottaErrorType::Terracotta);
                    state.error_message = Some(format!(
                        "unsupported terracotta state: {}",
                        api_state.state
                    ));
                } else if state.status == TerracottaStatus::Fatal {
                    state.error_type =
                        api_state.r#type.map(TerracottaErrorType::from);
                    state.error_message = api_state.url.clone();
                } else if state.status != TerracottaStatus::Error {
                    state.error_type = None;
                    state.error_message = None;
                }

                if let Some(profiles) = api_state.profiles {
                    state.players = profiles
                        .into_iter()
                        .map(|p| PlayerInfo {
                            machine_id: p.machine_id,
                            name: p.name,
                            vendor: p.vendor,
                            kind: TerracottaPlayerKind::from(p.kind.as_str()),
                        })
                        .collect();
                }
                drop(state);
                sync_minecraft_lan_announcer(server_port).await;
            }
            Err(e) => {
                warn!("failed to poll terracotta state: {e:#}");
                consecutive_failures += 1;
                if consecutive_failures < MAX_CONSECUTIVE_POLL_FAILURES {
                    continue;
                }
                let mut state = TERRACOTTA_STATE.lock().await;
                if state.status != TerracottaStatus::Idle {
                    state.status = TerracottaStatus::Error;
                }
                state.http_port = None;
                state.server_port = None;
                drop(state);
                let mut runtime = TERRACOTTA_RUNTIME.lock().await;
                if let Some(mut process) = runtime.process.take() {
                    process.child.start_kill().ok();
                }
                runtime.lan_announcer.sync(None);
                break;
            }
        }
    }
}

fn is_binary_installed() -> bool {
    is_terracotta_executable(&resolve_installed_terracotta_binary_path())
}

pub async fn get_state() -> TerracottaState {
    let mut state = TERRACOTTA_STATE.lock().await.clone();
    state.binary_installed = is_binary_installed();
    state.installed_version = installed_terracotta_version();
    state
}

pub async fn get_meta() -> eyre::Result<TerracottaMeta> {
    let state = TERRACOTTA_STATE.lock().await;
    let port = state
        .http_port
        .ok_or_else(|| eyre::eyre!("terracotta is not running"))?;
    drop(state);
    terracotta_get::<TerracottaMeta>(port, "/meta").await
}

pub async fn get_player_name() -> String {
    match crate::State::get_if_initialized() {
        Some(state_ref) => match Credentials::get_active(&state_ref.data).await
        {
            Ok(Some(creds)) => creds.offline_profile.name,
            _ => "Anonymous".to_string(),
        },
        None => "Anonymous".to_string(),
    }
}

#[derive(Deserialize)]
struct TerracottaPortInfo {
    port: u16,
}

fn terracotta_port_file() -> PathBuf {
    std::env::temp_dir()
        .join(format!("terracotta_port_{}.json", std::process::id()))
}

pub async fn start_terracotta(
    binary_path: Option<String>,
    auto_download: bool,
) -> eyre::Result<()> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    info!(
        custom_binary = binary_path.is_some(),
        auto_download,
        platform = terracotta_platform_key(),
        "starting terracotta multiplayer service"
    );
    let custom_bin_path = binary_path.map(PathBuf::from);
    let mut final_path = custom_bin_path.as_deref().map_or_else(
        resolve_installed_terracotta_binary_path,
        resolve_terracotta_binary_path,
    );

    if !is_terracotta_executable(&final_path) && auto_download {
        info!("terracotta binary not found, attempting auto-download");
        run_terracotta_download(None).await?;
        final_path = custom_bin_path.as_deref().map_or_else(
            resolve_installed_terracotta_binary_path,
            resolve_terracotta_binary_path,
        );
    }

    if !is_terracotta_executable(&final_path) {
        bail!(
            "valid terracotta executable not found at {} (platform: {}, expected name: {})",
            final_path.display(),
            terracotta_platform_key(),
            terracotta_binary_name()
        );
    }
    info!(binary = %final_path.display(), "using terracotta executable");

    let mut runtime = TERRACOTTA_RUNTIME.lock().await;
    if let Some(process) = runtime.process.as_mut() {
        if process
            .child
            .try_wait()
            .wrap_err("failed to inspect existing terracotta process")?
            .is_none()
        {
            bail!("terracotta is already running");
        }
        warn!("discarding exited terracotta process before restart");
        runtime.process.take();
    }

    let port_file = terracotta_port_file();
    let _ = std::fs::remove_file(&port_file);

    let is_macos = cfg!(target_os = "macos");
    let mut command = Command::new(&final_path);
    if is_macos {
        command.arg("--daemon");
    } else {
        command.arg("--hmcl").arg(&port_file);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .wrap_err_with(|| {
            let mode = if is_macos { "daemon" } else { "--hmcl" };
            format!(
                "failed to start terracotta {mode} at {}",
                final_path.display()
            )
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let pid = child.id().unwrap_or(0);
    let output = Arc::new(Mutex::new(VecDeque::new()));

    if let Some(stdout) = stdout {
        capture_terracotta_output(stdout, "stdout", pid, output.clone());
    }
    if let Some(stderr) = stderr {
        capture_terracotta_output(stderr, "stderr", pid, output.clone());
    }

    let pid = child.id().unwrap_or(0);
    info!("started terracotta (pid {pid})");

    runtime.process = Some(TerracottaProcess {
        child,
        output: output.clone(),
    });
    drop(runtime);

    {
        let mut state = TERRACOTTA_STATE.lock().await;
        state.status = TerracottaStatus::Starting;
        state.http_port = None;
        state.room_code = None;
        state.server_port = None;
        state.players.clear();
        state.error_type = None;
        state.error_message = None;
        state.profile_index = None;
    }

    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 30;
    let mut hmcl_helper: Option<Child> = None;
    let port = loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        attempts += 1;
        if attempts > MAX_ATTEMPTS {
            let output = {
                let mut runtime = TERRACOTTA_RUNTIME.lock().await;
                runtime.process.take().map(|mut p| {
                    let _ = p.child.start_kill();
                    p.output.clone()
                })
            };
            let details = match output {
                Some(out) => {
                    tokio::time::sleep(std::time::Duration::from_millis(25))
                        .await;
                    out.lock()
                        .await
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                None => String::new(),
            };
            let msg = if details.is_empty() {
                "timed out waiting for terracotta to start".to_string()
            } else {
                format!("timed out waiting for terracotta to start:\n{details}")
            };
            let mut state = TERRACOTTA_STATE.lock().await;
            state.status = TerracottaStatus::Error;
            state.error_type = Some(TerracottaErrorType::Terracotta);
            state.error_message = Some(msg.clone());
            bail!(msg);
        }

        match std::fs::read_to_string(&port_file) {
            Ok(contents) => {
                match serde_json::from_str::<TerracottaPortInfo>(&contents) {
                    Ok(info) => {
                        let _ = std::fs::remove_file(&port_file);
                        break info.port;
                    }
                    Err(e) => {
                        warn!("failed to parse terracotta port file: {}", e);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if is_macos {
                    let helper_exited = match hmcl_helper.as_mut() {
                        Some(helper) => helper
                            .try_wait()
                            .wrap_err(
                                "failed to inspect terracotta helper process",
                            )?
                            .is_some(),
                        None => true,
                    };
                    if helper_exited {
                        hmcl_helper = Some(
                            Command::new(&final_path)
                                .arg("--hmcl")
                                .arg(&port_file)
                                .stdin(Stdio::null())
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .kill_on_drop(true)
                                .spawn()
                                .wrap_err_with(|| {
                                    format!(
                                        "failed to start terracotta --hmcl helper at {}",
                                        final_path.display()
                                    )
                                })?,
                        );
                    }
                }

                if !is_macos
                    && let Some((status, output)) =
                        take_terminated_terracotta_process().await?
                {
                    let message = format_terracotta_exit(status, output).await;
                    // On Windows the `--hmcl` wrapper exits right after
                    // the delegate writes the port file, so give the file
                    // one final check before treating the exit as a
                    // failure.
                    if let Ok(contents) = std::fs::read_to_string(&port_file)
                        && let Ok(info) = serde_json::from_str::<
                            TerracottaPortInfo,
                        >(&contents)
                    {
                        let _ = std::fs::remove_file(&port_file);
                        break info.port;
                    }
                    let msg = format!(
                        "terracotta exited before writing port file: {message}"
                    );
                    let mut state = TERRACOTTA_STATE.lock().await;
                    state.status = TerracottaStatus::Error;
                    state.error_type = Some(TerracottaErrorType::Terracotta);
                    state.error_message = Some(msg.clone());
                    bail!(msg);
                }
            }
            Err(e) => {
                warn!("failed to read terracotta port file: {}", e);
            }
        }
    };

    info!("terracotta started on port {}", port);
    spawn_terracotta_state_poller(port).await;
    let mut state = TERRACOTTA_STATE.lock().await;
    state.http_port = Some(port);
    state.status = TerracottaStatus::Idle;
    Ok(())
}

pub async fn stop_terracotta() -> eyre::Result<()> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    let mut runtime = TERRACOTTA_RUNTIME.lock().await;
    if let Some(task) = runtime.state_poller.take() {
        task.abort();
    }
    if let Some(mut process) = runtime.process.take() {
        process.child.start_kill().ok();
        info!("stopped terracotta");
    }
    runtime.lan_announcer.sync(None);
    drop(runtime);

    // On Windows, `--hmcl` spawns a detached delegate and the tracked child
    // exits as soon as the port file appears, so killing the child may not
    // stop the actual server. Ask it to shut down over HTTP as a fallback.
    let port = {
        let state = TERRACOTTA_STATE.lock().await;
        state.http_port.or_else(|| {
            std::fs::read_to_string(terracotta_port_file())
                .ok()
                .and_then(|contents| {
                    serde_json::from_str::<TerracottaPortInfo>(&contents).ok()
                })
                .map(|info| info.port)
        })
    };
    if let Some(port) = port {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            terracotta_client()
                .get(format!("http://127.0.0.1:{port}/panic?peaceful=true"))
                .send(),
        )
        .await;
    }

    let mut state = TERRACOTTA_STATE.lock().await;
    *state = TerracottaState::default();
    state.binary_installed = is_binary_installed();
    drop(state);
    super::multiplayer::release_provider(
        super::multiplayer::MultiplayerProvider::Terracotta,
    )
    .await;
    Ok(())
}

fn build_room_url(
    port: u16,
    path: &str,
    room: &str,
    player: &str,
    nodes: &[String],
) -> String {
    let mut url = format!(
        "http://127.0.0.1:{port}{path}?room={}&player={}",
        urlencoding::encode(room),
        urlencoding::encode(player),
    );
    for node in nodes {
        url.push_str("&public_nodes=");
        url.push_str(&urlencoding::encode(node));
    }
    url
}

pub(crate) fn validate_public_nodes(nodes: &[String]) -> eyre::Result<()> {
    for node in nodes {
        let parsed = url::Url::parse(node).wrap_err_with(|| {
            format!("invalid terracotta public node: {node}")
        })?;
        if !matches!(
            parsed.scheme(),
            "http" | "https" | "tcp" | "tls" | "udp" | "ws" | "wss"
        ) || parsed.host_str().is_none()
        {
            bail!("unsupported terracotta public node: {node}");
        }
    }
    Ok(())
}

async fn configured_public_nodes() -> eyre::Result<Vec<String>> {
    let settings = super::settings::get().await?;
    validate_public_nodes(&settings.terracotta_public_nodes)?;
    Ok(settings.terracotta_public_nodes)
}

pub async fn start_hosting(
    room_code: Option<String>,
    player_name: String,
) -> eyre::Result<()> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    let player_name = player_name.trim();
    if player_name.is_empty() {
        bail!("player name cannot be empty");
    }
    let state = TERRACOTTA_STATE.lock().await;
    let port = state
        .http_port
        .ok_or_else(|| eyre::eyre!("terracotta is not running"))?;
    drop(state);

    let room_param = room_code.as_deref().unwrap_or("");
    let nodes = configured_public_nodes().await?;
    info!(
        port,
        custom_room_code = room_code.is_some(),
        "requesting terracotta host session"
    );

    let client = terracotta_client();
    let url = build_room_url(
        port,
        "/state/scanning",
        room_param,
        player_name,
        &nodes,
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .wrap_err("failed to send hosting request to terracotta")?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!(
            "terracotta hosting failed with status {}: {body}",
            status.as_u16()
        );
    }
    info!(port, "terracotta accepted host session request");
    Ok(())
}

pub async fn start_joining(
    room_code: String,
    player_name: String,
) -> eyre::Result<()> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    let player_name = player_name.trim();
    if player_name.is_empty() {
        bail!("player name cannot be empty");
    }
    let room_code = parse_room_code(&room_code)?;
    let state = TERRACOTTA_STATE.lock().await;
    let port = state
        .http_port
        .ok_or_else(|| eyre::eyre!("terracotta is not running"))?;
    drop(state);

    let nodes = configured_public_nodes().await?;
    info!(port, "requesting terracotta guest session");

    let client = terracotta_client();
    let url = build_room_url(
        port,
        "/state/guesting",
        &room_code,
        player_name,
        &nodes,
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .wrap_err("failed to send joining request to terracotta")?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!(
            "terracotta joining failed with status {}: {body}",
            status.as_u16()
        );
    }
    info!(port, "terracotta accepted guest session request");
    Ok(())
}

pub async fn reset_state() -> eyre::Result<()> {
    let _operation = TERRACOTTA_OPERATION.lock().await;
    let state = TERRACOTTA_STATE.lock().await;
    let port = state.http_port;
    drop(state);
    info!(?port, "resetting terracotta multiplayer state");

    if let Some(port) = port {
        let client = terracotta_client();
        let url = format!("http://127.0.0.1:{port}/state/ide");
        let response = client.get(&url).send().await.wrap_err_with(|| {
            format!("failed to connect to terracotta at {url}")
        })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!(
                "terracotta reset failed with status {}: {body}",
                status.as_u16()
            );
        }
    }

    let mut state = TERRACOTTA_STATE.lock().await;
    state.status = TerracottaStatus::Idle;
    state.room_code = None;
    state.server_port = None;
    state.players.clear();
    state.error_type = None;
    state.error_message = None;
    state.profile_index = None;
    drop(state);
    sync_minecraft_lan_announcer(None).await;
    info!("terracotta multiplayer state reset");
    Ok(())
}

pub fn parse_room_code(code: &str) -> eyre::Result<String> {
    if code.starts_with("U/") || code.starts_with("u/") {
        let inner = &code[2..];
        if inner.len() == 19 && inner.chars().filter(|&c| c == '-').count() == 3
        {
            let segments: Vec<&str> = inner.split('-').collect();
            if segments.len() == 4
                && segments.iter().all(|s| s.len() == 4)
                && segments
                    .iter()
                    .all(|s| s.chars().all(|c| c.is_ascii_alphanumeric()))
            {
                return Ok(format!("U/{inner}"));
            }
        }
    }
    bail!(
        "invalid room code format: {code}. Expected format: U/XXXX-XXXX-XXXX-XXXX"
    )
}

pub async fn get_logs() -> eyre::Result<String> {
    let state = TERRACOTTA_STATE.lock().await;
    let port = state
        .http_port
        .ok_or_else(|| eyre::eyre!("terracotta is not running"))?;
    drop(state);

    let client = terracotta_client();
    let url = format!("http://127.0.0.1:{port}/log?fetch=");
    let resp = client
        .get(&url)
        .send()
        .await
        .wrap_err("failed to fetch terracotta logs")?;
    let body = resp
        .text()
        .await
        .wrap_err("failed to read terracotta logs")?;
    Ok(body)
}

pub async fn get_diagnostic_report() -> eyre::Result<String> {
    let state = get_state().await;
    let state = serde_json::to_string_pretty(&state)
        .wrap_err("failed to serialize terracotta state")?;
    let output = TERRACOTTA_DIAGNOSTIC_OUTPUT
        .lock()
        .map(|output| output.iter().cloned().collect::<Vec<_>>().join("\n"))
        .unwrap_or_else(|_| "diagnostic output lock was poisoned".to_string());
    let output = if output.is_empty() {
        "No Terracotta process output was captured.".to_string()
    } else {
        output
    };

    Ok(format!(
        "Terracotta multiplayer diagnostics\nPlatform: {}\nBinary: {}\n\nState:\n{}\n\nTerracotta process output:\n{}\n",
        terracotta_platform_key(),
        resolve_installed_terracotta_binary_path().display(),
        state,
        output
    ))
}

#[cfg(all(
    test,
    any(
        target_os = "macos",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "windows"
    )
))]
mod tests {
    use super::*;
    use std::path::Path;

    fn executable_magic() -> [u8; 4] {
        #[cfg(target_os = "macos")]
        {
            [0xcf, 0xfa, 0xed, 0xfe]
        }
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            [0x7f, b'E', b'L', b'F']
        }
        #[cfg(target_os = "windows")]
        {
            [b'M', b'Z', 0, 0]
        }
    }

    #[test]
    fn resolver_ignores_installer_package_and_finds_executable() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().join(terracotta_binary_name());
        std::fs::write(&canonical, b"xar!").unwrap();

        let versioned = temp.path().join(versioned_terracotta_binary_name(
            "0.4.2",
            terracotta_platform_key(),
        ));
        std::fs::write(&versioned, executable_magic()).unwrap();

        assert!(!is_terracotta_executable(&canonical));
        assert!(is_terracotta_executable(&versioned));
        assert_eq!(resolve_terracotta_binary_path(&canonical), versioned);
    }

    #[cfg(unix)]
    #[test]
    fn executable_validation_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let link = temp.path().join(terracotta_binary_name());
        std::fs::write(&target, executable_magic()).unwrap();
        symlink(&target, &link).unwrap();

        assert!(!is_terracotta_executable(&link));
    }

    #[test]
    fn executable_search_prefers_requested_version() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("terracotta-0.4.1-test");
        let preferred = temp.path().join("terracotta-0.4.2-test");
        std::fs::write(&old, executable_magic()).unwrap();
        std::fs::write(&preferred, executable_magic()).unwrap();

        assert_eq!(
            find_terracotta_executable(
                temp.path(),
                Some("terracotta-0.4.2-test")
            ),
            Some(preferred)
        );
    }

    #[tokio::test]
    async fn binary_install_replaces_existing_file_and_removes_backup() {
        let temp = tempfile::tempdir().unwrap();
        let candidate = temp.path().join("candidate");
        let destination = temp.path().join(terracotta_binary_name());
        std::fs::write(&candidate, b"new").unwrap();
        std::fs::write(&destination, b"old").unwrap();

        install_terracotta_binary(&candidate, &destination)
            .await
            .unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
        assert!(
            !destination
                .with_file_name(format!("{}.old", terracotta_binary_name()))
                .exists()
        );
    }

    #[tokio::test]
    async fn binary_install_restores_existing_file_when_replacement_fails() {
        let temp = tempfile::tempdir().unwrap();
        let candidate = temp.path().join("missing");
        let destination = temp.path().join(terracotta_binary_name());
        std::fs::write(&destination, b"old").unwrap();

        assert!(
            install_terracotta_binary(&candidate, &destination)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"old");
    }

    #[test]
    fn parses_port_reported_by_an_existing_daemon() {
        let output =
            "stdout: [Lock]: Successfully join the global mutex, port = 57924";
        assert_eq!(reported_terracotta_port(output), Some(57924));
    }

    #[test]
    fn only_macos_uses_the_daemon_startup_argument() {
        assert_eq!(terracotta_startup_arguments("macos"), &["--daemon"]);
        assert!(terracotta_startup_arguments("windows").is_empty());
        assert!(terracotta_startup_arguments("linux").is_empty());
        assert!(terracotta_startup_arguments("freebsd").is_empty());
    }

    #[test]
    fn builds_the_binary_path_below_the_launcher_data_directory() {
        let root = Path::new("launcher-data");
        assert_eq!(
            binary::terracotta_binary_path_in(root),
            root.join("terracotta").join(terracotta_binary_name())
        );
    }

    #[test]
    fn installed_binary_resolution_falls_back_to_the_legacy_location() {
        let temp = tempfile::tempdir().unwrap();
        let installed = temp
            .path()
            .join("launcher-data")
            .join(terracotta_binary_name());
        let legacy = temp
            .path()
            .join("application")
            .join(terracotta_binary_name());
        std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, executable_magic()).unwrap();

        assert_eq!(
            binary::resolve_installed_terracotta_binary_path_from(
                &installed, &legacy
            ),
            legacy
        );
    }

    #[test]
    fn installed_binary_resolution_keeps_the_writable_path_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let installed = temp
            .path()
            .join("launcher-data")
            .join(terracotta_binary_name());
        let legacy = temp
            .path()
            .join("application")
            .join(terracotta_binary_name());

        assert_eq!(
            binary::resolve_installed_terracotta_binary_path_from(
                &installed, &legacy
            ),
            installed
        );
    }

    #[test]
    fn parses_guest_server_port() {
        assert_eq!(
            terracotta_server_port(Some("127.0.0.1:38449")),
            Some(38449)
        );
        assert_eq!(terracotta_server_port(Some("invalid")), None);
        assert_eq!(terracotta_server_port(None), None);
    }

    #[test]
    fn validates_release_versions_used_in_paths_and_urls() {
        assert!(validate_terracotta_version("0.4.2").is_ok());
        assert!(validate_terracotta_version("0.4.2-rc.1").is_ok());
        assert!(validate_terracotta_version("").is_err());
        assert!(validate_terracotta_version("../terracotta").is_err());
        assert!(validate_terracotta_version("0.4.2/other").is_err());
    }

    #[test]
    fn flags_unknown_or_different_installed_versions_for_update() {
        assert!(update_is_available(None, "0.4.2"));
        assert!(update_is_available(Some("0.4.1"), "0.4.2"));
        assert!(!update_is_available(Some("0.4.2"), "0.4.2"));
    }

    #[test]
    fn cleanup_preserves_version_metadata() {
        assert!(binary::should_cleanup_terracotta_file(
            "terracotta-0.4.1-windows-x86_64.exe"
        ));
        assert!(!binary::should_cleanup_terracotta_file(
            "terracotta-version.json"
        ));
        assert!(!binary::should_cleanup_terracotta_file(
            "terracotta-version.json.tmp"
        ));
    }

    #[test]
    fn provides_independent_download_sources() {
        let urls = terracotta_download_urls("0.4.2", "macos-arm64");
        assert_eq!(urls.len(), 2);
        assert!(urls.iter().any(|url| url.contains("gitee.com")));
        assert!(urls.iter().any(|url| url.contains("github.com")));
    }

    #[test]
    fn maps_protocol_states_without_silently_accepting_unknown_values() {
        assert_eq!(
            TerracottaStatus::from_api("guest-ok"),
            Some(TerracottaStatus::GuestReady)
        );
        assert_eq!(TerracottaStatus::from_api("future-state"), None);
    }

    #[test]
    fn validates_room_codes_at_the_backend_boundary() {
        assert_eq!(
            parse_room_code("u/ABCD-EFGH-IJKL-MNOP").unwrap(),
            "U/ABCD-EFGH-IJKL-MNOP"
        );
        assert!(parse_room_code("ABCD-EFGH-IJKL-MNOP").is_err());
        assert!(parse_room_code("U/ABCD-EFGH-IJKL-MNO!").is_err());
    }

    #[test]
    fn builds_room_urls_with_repeated_public_node_parameters() {
        let nodes = vec![
            "wss://center.node.1tmc.top".to_string(),
            "tcp://example.com:11010".to_string(),
        ];
        let url = build_room_url(
            12345,
            "/state/scanning",
            "U/ABCD-EFGH-IJKL-MNOP",
            "Player One",
            &nodes,
        );

        assert_eq!(
            url,
            "http://127.0.0.1:12345/state/scanning?room=U%2FABCD-EFGH-IJKL-MNOP&player=Player%20One&public_nodes=wss%3A%2F%2Fcenter.node.1tmc.top&public_nodes=tcp%3A%2F%2Fexample.com%3A11010"
        );
        assert!(
            !build_room_url(12345, "/state/scanning", "", "Player", &[])
                .contains("public_nodes")
        );
    }

    #[test]
    fn validates_supported_public_node_urls() {
        assert!(
            validate_public_nodes(&[
                "wss://center.node.1tmc.top".to_string(),
                "udp://example.com:11010".to_string(),
            ])
            .is_ok()
        );
        assert!(validate_public_nodes(&[]).is_ok());
        assert!(
            validate_public_nodes(&["ftp://example.com".to_string()]).is_err()
        );
        assert!(validate_public_nodes(&["wss://".to_string()]).is_err());
    }

    #[test]
    fn serializes_error_types_for_the_frontend_contract() {
        assert_eq!(
            serde_json::to_string(&TerracottaErrorType::Terracotta).unwrap(),
            "\"terracotta\""
        );
        assert_eq!(
            serde_json::to_string(&TerracottaPlayerKind::Host).unwrap(),
            "\"HOST\""
        );
    }
}
