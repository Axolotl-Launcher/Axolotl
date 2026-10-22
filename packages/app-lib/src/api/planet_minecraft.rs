use crate::State;
use crate::util::fetch::INSECURE_REQWEST_CLIENT;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CONNECTOR_URL_ENV: &str = "AXOLOTL_PLANET_MINECRAFT_CONNECTOR_URL";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetMinecraftProject {
    pub id: String,
    pub title: String,
    pub page_url: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub versions: Vec<PlanetMinecraftVersion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetMinecraftVersion {
    pub id: String,
    pub name: String,
    pub game_versions: Vec<String>,
    pub download: PlanetMinecraftDownload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetMinecraftDownload {
    pub page_url: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub direct_url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PlanetMinecraftInstallRoute {
    Automatic {
        direct_url: String,
        sha256: String,
        file_name: Option<String>,
    },
    Manual {
        page_url: String,
        file_name: Option<String>,
    },
}

impl PlanetMinecraftDownload {
    pub fn install_route(&self) -> PlanetMinecraftInstallRoute {
        match (
            self.direct_url.as_deref().filter(|url| {
                !url.trim().is_empty() && validated_download_url(url).is_ok()
            }),
            self.sha256
                .as_deref()
                .filter(|hash| !hash.trim().is_empty()),
        ) {
            (Some(direct_url), Some(sha256)) => {
                PlanetMinecraftInstallRoute::Automatic {
                    direct_url: direct_url.to_string(),
                    sha256: sha256.to_string(),
                    file_name: self.file_name.clone(),
                }
            }
            _ => PlanetMinecraftInstallRoute::Manual {
                page_url: self.page_url.clone(),
                file_name: self.file_name.clone(),
            },
        }
    }
}

pub fn connector_base_url() -> crate::Result<String> {
    let value = std::env::var(CONNECTOR_URL_ENV).map_err(|_| {
        crate::ErrorKind::InputError(
            "Planet Minecraft connector is not configured".to_string(),
        )
    })?;
    let url = reqwest::Url::parse(&value).map_err(|_| {
        crate::ErrorKind::InputError(
            "Planet Minecraft connector URL is invalid".to_string(),
        )
    })?;
    if url.scheme() != "https" {
        return Err(crate::ErrorKind::InputError(
            "Planet Minecraft connector must use HTTPS".to_string(),
        )
        .into());
    }
    Ok(value.trim_end_matches('/').to_string())
}

pub async fn search_projects(
    query: &str,
    game_version: Option<&str>,
) -> crate::Result<Vec<PlanetMinecraftProject>> {
    let mut url = format!(
        "{}/projects?query={}",
        connector_base_url()?,
        urlencoding::encode(query)
    );
    if let Some(game_version) = game_version.filter(|value| !value.is_empty()) {
        url.push_str(&format!(
            "&game_version={}",
            urlencoding::encode(game_version)
        ));
    }
    connector_get(&url).await
}

pub async fn get_project(id: &str) -> crate::Result<PlanetMinecraftProject> {
    connector_get(&format!(
        "{}/projects/{}",
        connector_base_url()?,
        urlencoding::encode(id)
    ))
    .await
}

pub async fn download_verified_file(
    direct_url: &str,
    expected_sha256: &str,
) -> crate::Result<Bytes> {
    let url = validated_download_url(direct_url)?;
    if expected_sha256.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Planet Minecraft downloads require a SHA-256".to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    let _permit = state.api_semaphore.0.acquire().await?;
    let bytes = INSECURE_REQWEST_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Planet Minecraft returned an empty file".to_string(),
        )
        .into());
    }
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(crate::ErrorKind::OtherError(
            "Planet Minecraft SHA-256 mismatch".to_string(),
        )
        .into());
    }
    Ok(bytes)
}

fn validated_download_url(value: &str) -> crate::Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| {
        crate::ErrorKind::InputError(
            "Planet Minecraft download URL is invalid".to_string(),
        )
    })?;
    if url.scheme() != "https" {
        return Err(crate::ErrorKind::InputError(
            "Planet Minecraft download URL must use HTTPS".to_string(),
        )
        .into());
    }
    Ok(url)
}

pub(crate) async fn connector_get<T>(url: &str) -> crate::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let state = State::get().await?;
    let _permit = state.api_semaphore.0.acquire().await?;
    Ok(INSECURE_REQWEST_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_without_a_hash_remain_manual() {
        let download = PlanetMinecraftDownload {
            page_url: "https://www.planetminecraft.com/mod/example".to_string(),
            file_name: Some("example.jar".to_string()),
            direct_url: Some("https://host.example/example.jar".to_string()),
            sha256: None,
        };
        assert!(matches!(
            download.install_route(),
            PlanetMinecraftInstallRoute::Manual { .. }
        ));
    }

    #[test]
    fn direct_downloads_require_https() {
        assert!(validated_download_url("http://example.com/file.jar").is_err());
        assert!(validated_download_url("https://example.com/file.jar").is_ok());
    }

    #[test]
    fn non_https_direct_downloads_fall_back_to_manual_import() {
        let download = PlanetMinecraftDownload {
            page_url: "https://www.planetminecraft.com/mod/example".to_string(),
            file_name: Some("example.jar".to_string()),
            direct_url: Some("http://host.example/example.jar".to_string()),
            sha256: Some("abc".to_string()),
        };
        assert!(matches!(
            download.install_route(),
            PlanetMinecraftInstallRoute::Manual { .. }
        ));
    }
}
