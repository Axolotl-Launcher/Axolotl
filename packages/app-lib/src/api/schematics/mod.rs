//! Source-neutral schematic discovery and acquisition.

mod public;

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
pub(crate) const MAX_FILE_SIZE: usize = 128 * 1024 * 1024;

use crate::resources::*;
use crate::third_party_accounts::shimu;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Shimu,
    Reden,
    #[serde(rename = "buildschematics")]
    BuildSchematics,
}

impl Source {
    pub fn id(self) -> &'static str {
        match self {
            Self::Shimu => "shimu",
            Self::Reden => "reden",
            Self::BuildSchematics => "buildschematics",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub name: String,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub source: Source,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub author: String,
    pub cover: Option<String>,
    pub game_versions: Vec<String>,
    pub files: Vec<File>,
    pub files_resolved: bool,
    pub source_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    #[serde(flatten)]
    pub project: Project,
    pub description: String,
    pub description_format: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<Project>,
    pub page_count: u32,
    pub total: u64,
    pub failed_sources: Vec<Source>,
}

fn failure(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.to_owned()).into()
}

fn shimu_id(id: &str) -> crate::Result<u64> {
    id.parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| failure("Invalid Shimu schematic identity"))
}

fn from_shimu(project: shimu::SchematicPost) -> Project {
    Project {
        source: Source::Shimu,
        id: project.post_id.to_string(),
        source_url: format!("https://www.mcimg.com/post/{}", project.post_id),
        title: project.title,
        summary: project.summary,
        author: project.author,
        cover: project.cover,
        game_versions: project.game_versions,
        files: project
            .files
            .into_iter()
            .map(|file| File {
                name: file.name,
                size: file.size,
            })
            .collect(),
        files_resolved: true,
    }
}

pub async fn list(
    sources: Vec<Source>,
    page: u32,
    query: String,
    page_size: u32,
    sort: String,
) -> crate::Result<Page> {
    if page == 0
        || page > 10000
        || query.chars().count() > 200
        || ![5, 10, 15, 20, 50, 100].contains(&page_size)
        || sources.is_empty()
    {
        return Err(failure("Invalid schematic search"));
    }
    let mut seen = HashSet::new();
    let sources: Vec<_> = sources
        .into_iter()
        .filter(|source| seen.insert(*source))
        .collect();
    if sources.len() == 1 {
        return list_source(sources[0], page, query, page_size, sort).await;
    }
    if sort != "default" {
        return Err(failure("Unsupported combined schematic sort"));
    }
    let replies = join_all(sources.iter().map(|source| {
        list_source(
            *source,
            1,
            query.clone(),
            page_size,
            default_sort(*source).into(),
        )
    }))
    .await;
    let mut feeds = Vec::new();
    let mut failed_sources = Vec::new();
    let mut first_error = None;
    for (source, reply) in sources.into_iter().zip(replies) {
        match reply {
            Ok(reply) => feeds.push((source, reply)),
            Err(error) => {
                failed_sources.push(source);
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if feeds.is_empty() {
        return Err(first_error
            .unwrap_or_else(|| failure("No schematic sources are available")));
    }
    let total: u64 = feeds.iter().map(|(_, reply)| reply.total).sum();
    let start = (page as u64 - 1) * page_size as u64;
    let end = (start + page_size as u64).min(total);
    let mut consumed = vec![0u64; feeds.len()];
    let mut slots = Vec::new();
    let mut position = 0;
    // Interleave source-local order before pagination. Exhausted sources drop
    // out of the rotation, so the tail never develops empty pages or holes.
    while position < end {
        for (index, (_, reply)) in feeds.iter().enumerate() {
            if consumed[index] >= reply.total {
                continue;
            }
            if position >= start {
                slots.push((index, consumed[index]));
            }
            consumed[index] += 1;
            position += 1;
            if position >= end {
                break;
            }
        }
    }
    let mut needed = HashSet::new();
    for (index, offset) in &slots {
        needed.insert((*index, (*offset / page_size as u64 + 1) as u32));
    }
    let mut pages: HashMap<_, _> = feeds
        .iter()
        .enumerate()
        .map(|(index, (_, reply))| ((index, 1u32), reply.items.clone()))
        .collect();
    let requests = needed.into_iter().filter(|(_, page)| *page != 1).map(
        |(index, page)| {
            let source = feeds[index].0;
            let query = query.clone();
            async move {
                let reply = list_source(
                    source,
                    page,
                    query,
                    page_size,
                    default_sort(source).into(),
                )
                .await?;
                Ok::<_, crate::Error>(((index, page), reply.items))
            }
        },
    );
    for reply in join_all(requests).await {
        let (key, items) = reply?;
        pages.insert(key, items);
    }
    let mut items = Vec::with_capacity(slots.len());
    for (index, offset) in slots {
        let page = (offset / page_size as u64 + 1) as u32;
        let item = pages
            .get(&(index, page))
            .and_then(|items| items.get((offset % page_size as u64) as usize))
            .ok_or_else(|| {
                failure("The source changed during pagination. Try again")
            })?;
        items.push(item.clone());
    }
    Ok(Page {
        items,
        total,
        page_count: total.div_ceil(page_size as u64).max(1) as u32,
        failed_sources,
    })
}

fn default_sort(source: Source) -> &'static str {
    if source == Source::Shimu {
        "new"
    } else {
        "default"
    }
}

async fn list_source(
    source: Source,
    page: u32,
    query: String,
    page_size: u32,
    sort: String,
) -> crate::Result<Page> {
    if source == Source::Shimu {
        let page = shimu::list(page, query, page_size, sort).await?;
        return Ok(Page {
            page_count: page.page_count,
            total: page.total,
            failed_sources: vec![],
            items: page.items.into_iter().map(from_shimu).collect(),
        });
    }
    public::list(source, page, &query, page_size, &sort).await
}

pub async fn detail(source: Source, id: String) -> crate::Result<Detail> {
    if source == Source::Shimu {
        let detail = shimu::detail(shimu_id(&id)?).await?;
        return Ok(Detail {
            project: from_shimu(detail.project),
            description: detail.description,
            description_format: "html",
        });
    }
    Ok(public::detail(source, &id).await?.detail)
}

async fn acquire(
    source: Source,
    id: &str,
    file_name: &str,
) -> crate::Result<(String, Vec<u8>)> {
    let (name, bytes) = if source == Source::Shimu {
        (
            file_name.to_owned(),
            shimu::download(shimu_id(id)?, file_name).await?,
        )
    } else {
        public::download(source, id, file_name).await?
    };
    validate_schematic(&bytes)?;
    Ok((name, bytes))
}

pub async fn preview_file(
    source: Source,
    id: &str,
    file_name: &str,
) -> crate::Result<(tempfile::TempDir, std::path::PathBuf)> {
    let (name, bytes) = acquire(source, id, file_name).await?;
    let directory = tempfile::Builder::new()
        .prefix("axolotl-schematic-")
        .tempdir()?;
    let path = directory
        .path()
        .join(crate::util::io::sanitize_download_file_name(&name)?);
    tokio::fs::write(&path, bytes).await?;
    Ok((directory, path))
}

pub async fn install(
    instance_id: String,
    source: Source,
    id: String,
    file_name: String,
) -> crate::Result<String> {
    let (name, bytes) = acquire(source, &id, &file_name).await?;
    let hash = crate::util::fetch::sha1_async(bytes.clone().into()).await?;
    let artifact = ResourceArtifact {
        reference: SourceArtifactRef {
            release: SourceReleaseRef {
                project: SourceProjectRef {
                    source: SourceId::new(source.id())
                        .map_err(|_| failure("Invalid schematic source"))?,
                    project_id: SourceProjectId::new(id)
                        .map_err(|_| failure("Invalid schematic identity"))?,
                },
                release_id: SourceReleaseId::new(hash.clone())
                    .map_err(|_| failure("Invalid schematic hash"))?,
            },
            artifact_id: SourceArtifactId::new(name.clone())
                .map_err(|_| failure("Invalid schematic file"))?,
        },
        file_name: Some(name.clone()),
        role: ArtifactRole::Primary,
        available: Some(true),
        integrity: ArtifactIntegrity {
            size: Some(bytes.len() as u64),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: hash,
            }],
        },
        acquisition: ArtifactAcquisition::Unavailable,
        dependencies: vec![],
    };
    let state = crate::State::get().await?;
    let path = crate::state::instances::commands::install_schematic::install(
        &state,
        &instance_id,
        &name,
        &bytes,
        &artifact,
    )
    .await?;
    if let Err(error) =
        crate::instance::emit_content_changed(&instance_id).await
    {
        tracing::warn!(%instance_id, %error, "Installed schematic content notification failed");
    }
    Ok(path)
}

pub(crate) fn validate_schematic(bytes: &[u8]) -> crate::Result<()> {
    let mut decoded = Vec::new();
    if bytes.starts_with(&[0x1f, 0x8b]) {
        flate2::read::GzDecoder::new(bytes)
            .take(MAX_FILE_SIZE as u64 + 1)
            .read_to_end(&mut decoded)
            .map_err(|_| failure("Invalid compressed schematic"))?;
    } else {
        decoded.extend_from_slice(bytes);
    }
    if decoded.len() > MAX_FILE_SIZE {
        return Err(failure("The schematic is too large"));
    }
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut std::io::Cursor::new(decoded),
        quartz_nbt::io::Flavor::Uncompressed,
    )
    .map_err(|_| failure("The downloaded file is not a schematic"))?;
    let valid = (root.contains_key("Regions") && root.contains_key("Metadata"))
        || (root.contains_key("Width")
            && root.contains_key("Height")
            && root.contains_key("Length")
            && (root.contains_key("Blocks") || root.contains_key("BlockData")))
        || root.contains_key("Schematic")
        || (root.contains_key("blocks")
            && root.contains_key("palette")
            && root.contains_key("size"));
    if !valid {
        return Err(failure("The downloaded file is not a schematic"));
    }
    Ok(())
}
