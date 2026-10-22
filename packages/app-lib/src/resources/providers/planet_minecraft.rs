use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::*;

pub struct PlanetMinecraftProvider {
    transport: Arc<dyn ProtocolTransport>,
}

impl PlanetMinecraftProvider {
    pub fn new(transport: Arc<dyn ProtocolTransport>) -> Self {
        Self { transport }
    }
    async fn project(
        &self,
        id: &SourceProjectId,
    ) -> ResourceResult<RemoteProject> {
        let project: RemoteProject = serde_json::from_value(
            self.transport
                .request(
                    &SourceId::planet_minecraft(),
                    ProtocolRequest::get(format!(
                        "/projects/{}",
                        urlencoding::encode(id.as_str())
                    )),
                )
                .await?,
        )?;
        if project.id != id.as_str() {
            return Err(ResourceError::InvalidResponse(
                "Planet Minecraft project mismatch".into(),
            ));
        }
        Ok(project)
    }
    async fn fetch(
        &self,
        query: &ProviderQuery,
    ) -> ResourceResult<ProviderValue> {
        match query {
            ProviderQuery::Project { id, .. } => Ok(ProviderValue::Project(self.project(id).await?.document()?)),
            ProviderQuery::Description { id, .. } => {
                let project = self.project(id).await?;
                let body = project.description.ok_or_else(|| ResourceError::Unsupported("connector returned no project description".into()))?;
                Ok(ProviderValue::Description(ResourceText { source: SourceId::planet_minecraft(), format: TextFormat::Plain, body }))
            }
            ProviderQuery::Release { project_id, release_id, .. } => {
                let id = project_id.as_ref().ok_or_else(|| ResourceError::Unsupported("Planet Minecraft requires project-qualified releases".into()))?;
                let project = self.project(id).await?;
                let reference = source_project(SourceId::planet_minecraft(), project.id)?;
                project.versions.into_iter().find(|version| version.id == release_id.as_str()).map(|version| version.document(reference).map(ProviderValue::Release)).unwrap_or(Ok(ProviderValue::NotFound))
            }
            ProviderQuery::Releases { project_id, target, cursor, page_size, .. } => {
                if !target.loaders.is_empty() || !target.channels.is_empty() { return Err(ResourceError::Unsupported("connector has no loader or channel metadata".into())); }
                let project = self.project(project_id).await?;
                let reference = source_project(SourceId::planet_minecraft(), project.id)?;
                let releases = project.versions.into_iter().map(|version| version.document(reference.clone()).map(|document| document.release)).collect::<ResourceResult<Vec<_>>>()?;
                let releases = releases.into_iter().filter(|release| target.game_versions.is_empty() || release.compatibility.game_versions.iter().any(|version| target.game_versions.contains(version))).collect();
                Ok(ProviderValue::Releases(slice_page(releases, offset(cursor)?, *page_size)?))
            }
            ProviderQuery::Search { query, cursor } => {
                validate_search(query)?;
                let mut request = ProtocolRequest::get("/projects");
                request.query.push(("query".into(), query.text.clone()));
                if let Some(version) = query.target.game_versions.first() { request.query.push(("game_version".into(), version.clone())); }
                let projects: Vec<RemoteProject> = serde_json::from_value(self.transport.request(&SourceId::planet_minecraft(), request).await?)?;
                let entries = projects.into_iter().map(|project| project.document().map(|document| document.metadata)).collect::<ResourceResult<Vec<_>>>()?;
                if (!query.types.is_empty() || query.filter.as_ref().is_some_and(has_type_filter)) && entries.iter().any(|entry| entry.types.is_empty()) {
                    return Err(ResourceError::Unsupported("connector returned untyped projects; the requested content type cannot be verified".into()));
                }
                let entries = entries.into_iter().filter(|entry| {
                    (query.types.is_empty() || query.types.iter().any(|kind| entry.types.contains(kind)))
                        && query.filter.as_ref().is_none_or(|filter| matches_type_filter(filter, &entry.types))
                }).collect();
                Ok(ProviderValue::Search(slice_page(entries, offset(cursor)?, query.page_size)?))
            }
            _ => Err(ResourceError::Unsupported("connector does not expose identification, dependency or changelog endpoints".into())),
        }
    }
}

#[async_trait]
impl ResourceProvider for PlanetMinecraftProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: SourceId::planet_minecraft(),
            available: connector_namespace().is_some(),
            types: vec![
                ResourceType::Mod,
                ResourceType::ResourcePack,
                ResourceType::DataPack,
                ResourceType::Shader,
                ResourceType::World,
            ],
            capabilities: vec![
                SourceCapability::Project,
                SourceCapability::Description,
                SourceCapability::Search,
                SourceCapability::Releases,
                SourceCapability::Download,
            ],
            supported_search_sorts: vec![ResourceSearchSort::Relevance],
        }
    }
    fn namespace(&self) -> String {
        connector_namespace()
            .unwrap_or_else(|| "planet-minecraft-unconfigured".into())
    }
    fn batch_limit(&self, _: &ProviderQuery) -> usize {
        1
    }
    fn validate_query(&self, query: &ProviderQuery) -> ResourceResult<()> {
        if let ProviderQuery::Search { query, .. } = query {
            validate_search(query)?;
        }
        Ok(())
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        let mut result = vec![];
        for query in queries {
            result.push(match self.fetch(query).await {
                Err(ResourceError::Network { status: Some(404) }) => {
                    Ok(ProviderValue::NotFound)
                }
                value => value,
            });
        }
        result
    }
}

fn validate_search(query: &ResourceSearchQuery) -> ResourceResult<()> {
    if query.sort != ResourceSearchSort::Relevance
        || !query.categories.is_empty()
        || !query.target.loaders.is_empty()
        || query.target.game_versions.len() > 1
    {
        return Err(ResourceError::Unsupported("connector search exposes keyword, declared content types and one game version only".into()));
    }
    if let Some(filter) = &query.filter {
        validate_filter(filter, &query.target.game_versions, 0)?;
    }
    Ok(())
}

fn validate_filter(
    filter: &ResourceSearchExpression,
    versions: &[String],
    depth: usize,
) -> ResourceResult<()> {
    if depth > 16 {
        return Err(ResourceError::InvalidRequest(
            "search filter nesting exceeded".into(),
        ));
    }
    match filter {
        ResourceSearchExpression::All { terms }
        | ResourceSearchExpression::Any { terms } => {
            for term in terms {
                validate_filter(term, versions, depth + 1)?;
            }
            Ok(())
        }
        ResourceSearchExpression::Predicate { field, .. }
            if matches!(field.as_str(), "type" | "allTypes") =>
        {
            Ok(())
        }
        ResourceSearchExpression::Predicate {
            field,
            values,
            excluded: false,
        } if field == "gameVersion"
            && values.len() == 1
            && values == versions =>
        {
            Ok(())
        }
        _ => Err(ResourceError::Unsupported(
            "connector cannot express these search filters".into(),
        )),
    }
}

fn has_type_filter(filter: &ResourceSearchExpression) -> bool {
    match filter {
        ResourceSearchExpression::All { terms }
        | ResourceSearchExpression::Any { terms } => {
            terms.iter().any(has_type_filter)
        }
        ResourceSearchExpression::Predicate { field, .. } => {
            matches!(field.as_str(), "type" | "allTypes")
        }
    }
}

fn matches_type_filter(
    filter: &ResourceSearchExpression,
    types: &[ResourceType],
) -> bool {
    match filter {
        ResourceSearchExpression::All { terms } => {
            terms.iter().all(|term| matches_type_filter(term, types))
        }
        ResourceSearchExpression::Any { terms } => {
            terms.iter().any(|term| matches_type_filter(term, types))
        }
        ResourceSearchExpression::Predicate {
            field,
            values,
            excluded,
        } if matches!(field.as_str(), "type" | "allTypes") => {
            let matched = types.iter().any(|kind| {
                serde_json::to_value(kind)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_lowercase))
                    .is_some_and(|kind| {
                        values.iter().any(|value| value.to_lowercase() == kind)
                    })
            });
            matched != *excluded
        }
        _ => true,
    }
}

fn connector_namespace() -> Option<String> {
    let url = crate::api::planet_minecraft::connector_base_url().ok()?;
    let parsed = url::Url::parse(&url).ok()?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    Some(format!(
        "planet-minecraft:{:x}",
        Sha256::digest(url.as_bytes())
    ))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteProject {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    id: String,
    title: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    project_type: Option<String>,
    #[serde(default)]
    versions: Vec<RemoteRelease>,
}

impl RemoteProject {
    fn document(self) -> ResourceResult<ProjectDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        let types = match self.project_type.as_deref() {
            Some("mod") => vec![ResourceType::Mod],
            Some("resourcepack") => vec![ResourceType::ResourcePack],
            Some("datapack") => vec![ResourceType::DataPack],
            Some("world") => vec![ResourceType::World],
            Some("shader") => vec![ResourceType::Shader],
            _ => vec![],
        };
        Ok(ProjectDocument {
            protocol,
            metadata: SourceProjectMetadata {
                environments: vec![],
                categories: vec![],
                game_versions: vec![],
                loaders: vec![],
                gallery: vec![],
                project: source_project(SourceId::planet_minecraft(), self.id)?,
                slug: None,
                title: self.title,
                summary: self.summary.unwrap_or_default(),
                icon_url: None,
                types,
                authors: vec![],
                license: None,
                downloads: None,
                followers: None,
                created_at: None,
                project_published_at: None,
                updated_at: None,
                first_published_at: None,
                latest_published_at: None,
            },
            description: ResourceText {
                source: SourceId::planet_minecraft(),
                format: TextFormat::Plain,
                body: self.description.unwrap_or_default(),
            },
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteRelease {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    id: String,
    name: String,
    #[serde(default)]
    game_versions: Vec<String>,
    download: RemoteDownload,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDownload {
    page_url: String,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    direct_url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

impl RemoteRelease {
    fn document(
        self,
        project: SourceProjectRef,
    ) -> ResourceResult<ReleaseDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        let reference = release_ref(project, self.id)?;
        let file_hashes = hashes(
            self.download
                .sha256
                .into_iter()
                .filter(|hash| !hash.is_empty())
                .map(|hash| (HashAlgorithm::Sha256, hash)),
        )?;
        let urls = if file_hashes.is_empty() {
            vec![]
        } else {
            self.download
                .direct_url
                .into_iter()
                .filter(|url| url.starts_with("https://"))
                .collect()
        };
        let artifact = ResourceArtifact {
            reference: SourceArtifactRef {
                release: reference.clone(),
                artifact_id: SourceArtifactId::new("download")?,
            },
            file_name: self.download.file_name,
            role: ArtifactRole::Primary,
            available: None,
            integrity: ArtifactIntegrity {
                size: None,
                hashes: file_hashes,
            },
            acquisition: acquisition(urls, Some(self.download.page_url)),
            dependencies: vec![],
        };
        Ok(ReleaseDocument {
            protocol,
            release: ResourceRelease {
                reference,
                version: self.name,
                channel: ReleaseChannel::Unknown,
                published_at: None,
                compatibility: ReleaseCompatibility {
                    game_versions: self.game_versions,
                    loaders: vec![],
                },
                artifacts: vec![artifact],
                equivalent_releases: vec![],
            },
            changelog: None,
        })
    }
}
