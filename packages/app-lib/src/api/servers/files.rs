//! Reading, writing, and downloading files inside a server's directory.

use std::path::{Path, PathBuf};

use std::future::Future;
use std::pin::Pin;

use tokio_util::sync::CancellationToken;

use crate::State;
use crate::util::fetch::{
    ContentValidation, DownloadRequest, FetchProgressFn, Integrity,
    ResourceClass, download_to_path,
};

use crate::Result;
use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::util::io::{self, IOError};

use super::manifest::server_path;

pub(super) use crate::util::io::safe_relative_join;

pub async fn read_file(server_id: &str, file: &str) -> Result<String> {
    let path = resolve_server_file(server_id, file).await?;
    let bytes = io::read(&path).await?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text)
}

pub async fn write_file(
    server_id: &str,
    file: &str,
    contents: &str,
) -> Result<()> {
    let path = resolve_server_file(server_id, file).await?;
    io::write(&path, contents).await?;
    Ok(())
}

pub async fn download_file(
    server_id: &str,
    url: &str,
    filename: &str,
    expected_sha1: Option<String>,
) -> Result<()> {
    let dir = server_path(server_id).await?;
    download_to_dir(server_id, &dir, url, filename, expected_sha1).await
}

/// Downloads a URL into a nested, relative path inside the server directory
/// (for example `mods/sodium.jar`), creating parent directories as needed.
/// The relative path is validated against traversal; a plain filename behaves
/// identically to [`download_file`].
pub(super) async fn download_to_dir(
    server_id: &str,
    dir: &Path,
    url: &str,
    rel_path: &str,
    expected_sha1: Option<String>,
) -> Result<()> {
    let state = State::get().await?;
    let integrity = Integrity {
        sha1: expected_sha1,
        content: if rel_path.ends_with(".jar") {
            ContentValidation::Jar
        } else {
            ContentValidation::None
        },
        ..Integrity::default()
    };
    let request = DownloadRequest::new(url, ResourceClass::Loader)
        .with_integrity(integrity);
    download_server_file(server_id, &state, dir, rel_path, request, None, None)
        .await
}

pub(super) async fn download_server_file(
    server_id: &str,
    state: &State,
    dir: &Path,
    rel_path: &str,
    mut request: DownloadRequest,
    cancellation: Option<&CancellationToken>,
    console_label: Option<&str>,
) -> Result<()> {
    let destination = safe_relative_join(dir, rel_path)?;
    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }
    request.cancellation = cancellation.cloned();
    let label = console_label.map(str::to_string);
    let mut last_progress = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap();
    let mut progress: Box<FetchProgressFn<'_>> = Box::new(
        move |downloaded,
              total|
              -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            let log = if last_progress.elapsed()
                >= std::time::Duration::from_secs(1)
            {
                last_progress = std::time::Instant::now();
                label.as_ref().map(|label| {
                    if total > 0 {
                        format!(
                            "Downloading {label}: {:.1} / {:.1} MiB",
                            downloaded as f64 / 1048576.0,
                            total as f64 / 1048576.0
                        )
                    } else {
                        format!(
                            "Downloading {label}: {:.1} MiB",
                            downloaded as f64 / 1048576.0
                        )
                    }
                })
            } else {
                None
            };
            Box::pin(async move {
                emit_server(
                    server_id,
                    ServerPayloadType::DownloadProgress {
                        downloaded,
                        total: (total > 0).then_some(total),
                    },
                )
                .await
                .ok();
                if let Some(line) = log {
                    super::lifecycle::log_server_step(server_id, &line).await;
                }
                Ok(())
            })
        },
    );
    download_to_path(
        request,
        &destination,
        &state.download_semaphore,
        &state.pool,
        Some(progress.as_mut()),
    )
    .await?;
    let size = tokio::fs::metadata(&destination)
        .await
        .map_err(|e| IOError::with_path(e, &destination))?
        .len();
    emit_server(
        server_id,
        ServerPayloadType::DownloadProgress {
            downloaded: size,
            total: Some(size),
        },
    )
    .await
    .ok();
    Ok(())
}

async fn resolve_server_file(server_id: &str, file: &str) -> Result<PathBuf> {
    let dir = server_path(server_id).await?;
    safe_relative_join(&dir, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_relative_join_rejects_unsafe_paths() {
        let dir = Path::new("/tmp/servers/a");
        for path in [
            "../secret",
            "mods/../../secret",
            r"..\secret",
            r"mods/..\../secret",
            "/etc/passwd",
            "C:/windows/system32",
            r"C:\windows\system32",
            "C:secret",
            r"\\server\share\secret",
            r"\secret",
            r"\\?\C:\secret",
            "a//b",
            "a/./b",
            ".",
            "",
            "mods/.. /secret",
            "mods/secret.",
            "mods/secret ",
            "mods/mod.jar:payload",
            "NUL/secret",
            "mods/CON.jar",
            "mods/mod\0.jar",
        ] {
            assert!(safe_relative_join(dir, path).is_err(), "{path:?}");
        }
    }

    #[test]
    fn safe_relative_join_allows_nested_paths() {
        let dir = Path::new("/tmp/servers/a");
        assert_eq!(
            safe_relative_join(dir, "mods/sodium.jar").unwrap(),
            dir.join("mods/sodium.jar")
        );
        assert_eq!(
            safe_relative_join(dir, "config/foo/bar.toml").unwrap(),
            dir.join("config/foo/bar.toml")
        );
        assert!(safe_relative_join(dir, "mods/../secret").is_err());
        assert!(safe_relative_join(dir, "../secret").is_err());
        assert!(safe_relative_join(dir, "/etc/passwd").is_err());
        assert!(safe_relative_join(dir, "C:/windows/system32").is_err());
        assert!(safe_relative_join(dir, "mods//double.jar").is_err());
        assert!(safe_relative_join(dir, "mods/./dot.jar").is_err());
        assert!(safe_relative_join(dir, "").is_err());
    }
}
