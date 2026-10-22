use crate::State;
use crate::util::fetch::INSECURE_REQWEST_CLIENT;
use serde::{Deserialize, Serialize};
use sha2::Digest;

const API_BASE_URL: &str = "https://mcarchive.net/api/v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McArchiveGameVersion {
    pub id: i64,
    pub name: String,
    #[serde(default, alias = "version_type")]
    pub version_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McArchiveFile {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default, alias = "archive_url")]
    pub archive_url: Option<String>,
    #[serde(default, alias = "direct_url")]
    pub direct_url: Option<String>,
    #[serde(default, alias = "redirect_url")]
    pub redirect_url: Option<String>,
    #[serde(default, alias = "page_url")]
    pub page_url: Option<String>,
}

impl McArchiveFile {
    pub fn download_url(&self) -> Option<&str> {
        self.archive_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .or_else(|| {
                self.direct_url
                    .as_deref()
                    .filter(|url| !url.trim().is_empty())
            })
    }

    pub fn needs_manual_download(&self) -> bool {
        !self.is_automatically_installable()
    }

    pub fn is_automatically_installable(&self) -> bool {
        self.download_url().is_some()
            && self
                .sha256
                .as_deref()
                .is_some_and(|hash| !hash.trim().is_empty())
    }

    pub fn manual_download_url(&self) -> Option<&str> {
        self.page_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .or_else(|| {
                self.redirect_url
                    .as_deref()
                    .filter(|url| !url.trim().is_empty())
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McArchiveModVersion {
    pub uuid: String,
    pub name: String,
    #[serde(default, alias = "game_versions")]
    pub game_versions: Vec<McArchiveGameVersion>,
    #[serde(default)]
    pub files: Vec<McArchiveFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McArchiveMod {
    #[serde(default)]
    pub uuid: String,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, alias = "website", alias = "page_url")]
    pub page_url: Option<String>,
    #[serde(default, alias = "mod_versions")]
    pub mod_versions: Vec<McArchiveModVersion>,
}

const SEARCH_FIELDS: &str = "{uuid,slug,name,description,website}";
const MOD_FIELDS: &str = "{uuid,slug,name,description,website,mod_versions{uuid,name,page_url,description,game_versions{id,name},files{uuid,name,sha256,description,page_url,redirect_url,direct_url,archive_url}}}";
const FILE_FIELDS: &str = "{uuid,name,sha256,description,page_url,redirect_url,direct_url,archive_url}";

fn normalize_mod_identity(item: &mut McArchiveMod) {
    if item.uuid.trim().is_empty() {
        item.uuid = item.slug.clone();
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum Collection<T> {
    Items(Vec<T>),
    Wrapped { results: Vec<T> },
    Data { data: Vec<T> },
}

impl<T> Collection<T> {
    fn into_inner(self) -> Vec<T> {
        match self {
            Self::Items(items) => items,
            Self::Wrapped { results } => results,
            Self::Data { data } => data,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum OneOrCollection<T> {
    One(T),
    Collection(Collection<T>),
}

impl<T> OneOrCollection<T> {
    fn into_first(self) -> Option<T> {
        match self {
            Self::One(item) => Some(item),
            Self::Collection(items) => items.into_inner().into_iter().next(),
        }
    }
}

pub async fn get_game_versions() -> crate::Result<Vec<McArchiveGameVersion>> {
    get_collection("/game_versions", "{id,name}").await
}

pub async fn search_mods(
    keyword: &str,
    game_version: Option<&str>,
) -> crate::Result<Vec<McArchiveMod>> {
    let keyword = keyword.trim();
    let mut url = format!(
        "{API_BASE_URL}/mods/?keyword={}",
        urlencoding::encode(keyword)
    );
    if let Some(game_version) = game_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        url.push_str("&game_version=");
        url.push_str(&urlencoding::encode(game_version));
    }
    let mut mods = get_json::<Collection<McArchiveMod>>(&url, SEARCH_FIELDS)
        .await
        .map(Collection::into_inner)?;
    for item in &mut mods {
        normalize_mod_identity(item);
    }
    Ok(mods)
}

pub async fn get_mod_by_slug(slug: &str) -> crate::Result<McArchiveMod> {
    get_json(
        &format!("{API_BASE_URL}/mods/by_slug/{}", urlencoding::encode(slug)),
        MOD_FIELDS,
    )
    .await
    .map(|mut item| {
        normalize_mod_identity(&mut item);
        item
    })
}

pub async fn get_file_by_filename(
    filename: &str,
) -> crate::Result<Option<McArchiveFile>> {
    get_optional(
        &format!(
            "{API_BASE_URL}/files/by_filename/{}",
            urlencoding::encode(filename)
        ),
        FILE_FIELDS,
    )
    .await
}

pub async fn get_file_by_sha256(
    sha256: &str,
) -> crate::Result<Option<McArchiveFile>> {
    get_optional(
        &format!(
            "{API_BASE_URL}/files/by_hash/sha256/{}",
            urlencoding::encode(sha256)
        ),
        FILE_FIELDS,
    )
    .await
}

pub async fn download_file(file: &McArchiveFile) -> crate::Result<Vec<u8>> {
    let url = file.download_url().ok_or_else(|| {
        crate::ErrorKind::InputError(
			"MCArchive does not expose a verifiable direct archive URL for this file"
				.to_string(),
		)
    })?;
    let state = State::get().await?;
    let _permit = state.download_semaphore.0.acquire().await?;
    let response = INSECURE_REQWEST_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?.to_vec();
    if bytes.is_empty() {
        return Err(crate::ErrorKind::OtherError(
            "MCArchive returned an empty file".to_string(),
        )
        .into());
    }
    let expected = file
        .sha256
        .as_deref()
        .filter(|hash| !hash.is_empty())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "MCArchive does not publish a SHA-256 for {}",
                file.name
            ))
        })?;
    let actual = format!("{:x}", sha2::Sha256::digest(&bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(crate::ErrorKind::OtherError(format!(
            "MCArchive SHA-256 mismatch for {}",
            file.name
        ))
        .into());
    }
    Ok(bytes)
}

async fn get_collection<T>(path: &str, fields: &str) -> crate::Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    get_json::<Collection<T>>(&format!("{API_BASE_URL}{path}"), fields)
        .await
        .map(Collection::into_inner)
}

async fn get_optional<T>(url: &str, fields: &str) -> crate::Result<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let state = State::get().await?;
    let _permit = state.api_semaphore.0.acquire().await?;
    let response = mcarchive_get(url).header("X-Fields", fields).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response.error_for_status()?;
    let value = response.json::<Option<OneOrCollection<T>>>().await?;
    Ok(value.and_then(OneOrCollection::into_first))
}

pub(crate) async fn get_json<T>(url: &str, fields: &str) -> crate::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let state = State::get().await?;
    let _permit = state.api_semaphore.0.acquire().await?;
    Ok(mcarchive_get(url)
        .header("X-Fields", fields)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn mcarchive_get(url: &str) -> reqwest::RequestBuilder {
    INSECURE_REQWEST_CLIENT
        .get(url)
        .version(reqwest::Version::HTTP_11)
        .header(reqwest::header::ACCEPT, "application/json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcarchive_uses_string_uuid_identifiers_and_archive_url() {
        let file: McArchiveFile = serde_json::from_value(serde_json::json!({
            "uuid": "46d5c61e-02b4-4ca0-8b4b-7c965d2931bc",
            "name": "ModLoader 1.6.2.zip",
            "sha256": "abc",
            "archive_url": "https://b2.mcarchive.net/file/modloader.zip",
            "direct_url": "",
            "redirect_url": "",
            "page_url": ""
        }))
        .unwrap();
        assert_eq!(file.uuid, "46d5c61e-02b4-4ca0-8b4b-7c965d2931bc");
        assert_eq!(
            file.download_url(),
            Some("https://b2.mcarchive.net/file/modloader.zip")
        );
    }

    #[test]
    fn missing_urls_require_manual_download() {
        let file = McArchiveFile {
            uuid: "file".to_string(),
            name: "mod.zip".to_string(),
            sha256: None,
            archive_url: Some(String::new()),
            direct_url: None,
            redirect_url: Some("https://example.invalid/download".to_string()),
            page_url: Some("https://mcarchive.net/mod".to_string()),
        };
        assert!(file.needs_manual_download());
        assert_eq!(
            file.manual_download_url(),
            Some("https://mcarchive.net/mod")
        );
    }

    #[test]
    fn hash_lookup_collections_return_the_first_matching_file() {
        let response: Option<OneOrCollection<McArchiveFile>> =
            serde_json::from_value(serde_json::json!([
                {
                    "uuid": "file-uuid",
                    "name": "mod.jar",
                    "sha256": "abc"
                }
            ]))
            .unwrap();
        assert_eq!(
            response
                .and_then(OneOrCollection::into_first)
                .map(|file| file.uuid),
            Some("file-uuid".to_string())
        );
    }

    #[test]
    fn mod_search_summaries_do_not_claim_to_include_versions() {
        let mod_: McArchiveMod = serde_json::from_value(serde_json::json!({
            "uuid": "mod-uuid",
            "slug": "modloader",
            "name": "ModLoader"
        }))
        .unwrap();

        assert_eq!(mod_.slug, "modloader");
        assert!(mod_.mod_versions.is_empty());
    }

    #[test]
    fn real_mcarchive_search_payload_deserializes_without_versions() {
        let payload = serde_json::json!([
            {
                "uuid": "ff38ccfe-24ca-4bc6-97ba-59d6b37b147a",
                "slug": "modloader",
                "name": "Modloader",
                "description": "",
                "website": ""
            },
            {
                "uuid": "7b2abb02-ba79-4363-abbf-268b2024488d",
                "slug": "modloadermp",
                "name": "ModLoaderMP"
            }
        ]);
        let mut mods: Vec<McArchiveMod> =
            serde_json::from_value::<Collection<McArchiveMod>>(payload)
                .unwrap()
                .into_inner();
        for item in &mut mods {
            normalize_mod_identity(item);
        }
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].slug, "modloader");
        assert_eq!(mods[0].uuid, "ff38ccfe-24ca-4bc6-97ba-59d6b37b147a");
        assert!(mods[0].mod_versions.is_empty());
        assert_eq!(mods[1].name, "ModLoaderMP");
    }

    #[test]
    fn real_mcarchive_mod_payload_deserializes_nested_versions_and_files() {
        let payload = serde_json::json!({
            "uuid": "ff38ccfe-24ca-4bc6-97ba-59d6b37b147a",
            "slug": "modloader",
            "name": "Modloader",
            "description": "",
            "website": "",
            "mod_versions": [{
                "uuid": "52fbdb8f-6ae7-4dd6-8b02-cbfc54b5ce3e",
                "name": "1.6.2",
                "page_url": "",
                "description": "",
                "game_versions": [{"id": 9, "name": "1.6.2"}],
                "files": [{
                    "uuid": "e7830701-aca7-4e9b-a5b2-94aacbeef293",
                    "name": "ModLoader 1.6.2.zip",
                    "sha256": "0b14f5e261c9862989aa74313b59188cce10bea6724bae31130ce1e8e6a1c060",
                    "description": "",
                    "page_url": "",
                    "redirect_url": "",
                    "direct_url": "",
                    "archive_url": "https://b2.mcarchive.net/file/mcarchive/example.zip"
                }]
            }]
        });
        let mut item: McArchiveMod = serde_json::from_value(payload).unwrap();
        normalize_mod_identity(&mut item);
        assert_eq!(item.mod_versions.len(), 1);
        assert_eq!(item.mod_versions[0].game_versions[0].name, "1.6.2");
        assert_eq!(
            item.mod_versions[0].files[0].download_url(),
            Some("https://b2.mcarchive.net/file/mcarchive/example.zip")
        );
    }

    #[test]
    fn missing_uuid_uses_slug_for_stable_search_identity() {
        let mut item: McArchiveMod =
            serde_json::from_value(serde_json::json!({
                "slug": "modloader",
                "name": "Modloader"
            }))
            .unwrap();
        normalize_mod_identity(&mut item);
        assert_eq!(item.uuid, "modloader");
    }
}
