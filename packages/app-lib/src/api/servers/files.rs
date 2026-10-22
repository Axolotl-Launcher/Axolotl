//! Reading, writing, and downloading files inside a server's directory.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use sha1_smol::Sha1;

use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::util::io::{self, IOError};
use crate::{ErrorKind, Result};

use super::manifest::server_path;

pub(super) use crate::util::io::safe_relative_join;

const DOWNLOAD_PROGRESS_STEP: u64 = 512 * 1024;

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
    let destination = safe_relative_join(dir, rel_path)?;
    let partial = destination.with_extension("part");

    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }

    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .build()
        .map_err(|e| ErrorKind::NetworkError(e.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())?;
    let total = response.content_length();
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::File::create(&partial)
        .await
        .map_err(|e| IOError::with_path(e, &partial))?;
    let mut hasher = Sha1::new();
    let mut downloaded: u64 = 0;
    let mut last_reported: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| IOError::with_path(e, &partial))?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        if downloaded - last_reported >= DOWNLOAD_PROGRESS_STEP {
            last_reported = downloaded;
            emit_server(
                server_id,
                ServerPayloadType::DownloadProgress { downloaded, total },
            )
            .await
            .ok();
        }
    }
    drop(file);

    if let Some(expected) = expected_sha1.as_deref() {
        let actual = hasher.digest().to_string();
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(ErrorKind::NetworkError(format!(
                "Download checksum mismatch for {rel_path}: expected {expected}, got {actual}"
            ))
            .as_error());
        }
    }

    tokio::fs::rename(&partial, &destination)
        .await
        .map_err(|e| IOError::with_path(e, &destination))?;
    emit_server(
        server_id,
        ServerPayloadType::DownloadProgress {
            downloaded,
            total: Some(downloaded.max(total.unwrap_or(0))),
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
