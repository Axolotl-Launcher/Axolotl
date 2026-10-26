use crate::state::instances::DependencyRelation;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

use super::*;

pub struct CurseForgeProvider {
    transport: Arc<dyn ProtocolTransport>,
    download_restriction_bypass: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl CurseForgeProvider {
    pub fn new(transport: Arc<dyn ProtocolTransport>) -> Self {
        Self::with_download_restriction_policy(
            transport,
            Arc::new(|| {
                crate::State::get_if_initialized().is_some_and(|state| {
                    state.bypass_curseforge_download_restrictions()
                })
            }),
        )
    }

    /// Test constructor that pins the download policy instead of reading the
    /// process-wide launcher state, which other tests may initialise.
    #[cfg(test)]
    pub(crate) fn for_tests(transport: Arc<dyn ProtocolTransport>) -> Self {
        Self::with_download_restriction_policy(transport, Arc::new(|| false))
    }

    pub(crate) fn with_download_restriction_policy(
        transport: Arc<dyn ProtocolTransport>,
        download_restriction_bypass: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            transport,
            download_restriction_bypass,
        }
    }
    async fn request(&self, request: ProtocolRequest) -> ResourceResult<Value> {
        self.transport
            .request(&SourceId::curseforge(), request)
            .await
    }

    async fn fetch_batch(
        &self,
        queries: &[ProviderQuery],
        bypass_restrictions: bool,
    ) -> ResourceResult<Vec<ResourceResult<ProviderValue>>> {
        if queries
            .iter()
            .all(|query| matches!(query, ProviderQuery::Project { .. }))
        {
            let ids = queries
                .iter()
                .map(|query| {
                    if let ProviderQuery::Project { id, .. } = query {
                        positive_id(id.as_str())
                    } else {
                        unreachable!()
                    }
                })
                .collect::<ResourceResult<Vec<_>>>()?;
            let response = self
                .request(ProtocolRequest::post(
                    "/v1/mods",
                    json!({"modIds": ids, "filterPcOnly": true}),
                ))
                .await?;
            let projects: Vec<RemoteProject> =
                serde_json::from_value(response["data"].clone())?;
            let mut values = HashMap::new();
            for project in projects {
                let id = project.id;
                if !ids.contains(&id) {
                    return Err(ResourceError::InvalidResponse(
                        "unexpected project in batch".into(),
                    ));
                }
                if values.insert(id, project.document()?).is_some() {
                    return Err(ResourceError::InvalidResponse(
                        "duplicate project in batch".into(),
                    ));
                }
            }
            return Ok(ids
                .into_iter()
                .map(|id| {
                    Ok(values
                        .get(&id)
                        .cloned()
                        .map(ProviderValue::Project)
                        .unwrap_or(ProviderValue::NotFound))
                })
                .collect());
        }
        if queries
            .iter()
            .all(|query| matches!(query, ProviderQuery::Release { .. }))
        {
            let ids = queries
                .iter()
                .map(|query| {
                    if let ProviderQuery::Release {
                        release_id,
                        project_id,
                        ..
                    } = query
                    {
                        if let Some(id) = project_id {
                            positive_id(id.as_str())?;
                        }
                        positive_id(release_id.as_str())
                    } else {
                        unreachable!()
                    }
                })
                .collect::<ResourceResult<Vec<_>>>()?;
            let response = self
                .request(ProtocolRequest::post(
                    "/v1/mods/files",
                    json!({"fileIds": ids}),
                ))
                .await?;
            let files: Vec<RemoteFile> =
                serde_json::from_value(response["data"].clone())?;
            let mut values = HashMap::new();
            for file in files {
                let id = file.id;
                if !ids.contains(&id) {
                    return Err(ResourceError::InvalidResponse(
                        "unexpected file in batch".into(),
                    ));
                }
                if values
                    .insert(
                        id,
                        file.document_with_download_policy(
                            bypass_restrictions,
                        )?,
                    )
                    .is_some()
                {
                    return Err(ResourceError::InvalidResponse(
                        "duplicate file in batch".into(),
                    ));
                }
            }
            let mut documents: Vec<_> = values.into_values().collect();
            self.fill_manual_links(&mut documents).await?;
            let values: HashMap<_, _> = documents
                .into_iter()
                .map(|document| {
                    positive_id(document.release.reference.release_id.as_str())
                        .map(|id| (id, document))
                })
                .collect::<ResourceResult<_>>()?;
            return Ok(ids
                .into_iter()
                .map(|id| {
                    Ok(values
                        .get(&id)
                        .cloned()
                        .map(ProviderValue::Release)
                        .unwrap_or(ProviderValue::NotFound))
                })
                .collect());
        }
        if queries.iter().all(|query| {
            matches!(
                query,
                ProviderQuery::Identify { .. }
                    | ProviderQuery::IdentifyExport { .. }
            )
        }) {
            let fingerprints: Vec<_> = queries
                .iter()
                .filter_map(|query| {
                    if let ProviderQuery::Identify { fingerprint, .. }
                    | ProviderQuery::IdentifyExport {
                        fingerprint, ..
                    } = query
                    {
                        *fingerprint
                    } else {
                        None
                    }
                })
                .collect();
            if fingerprints.is_empty() {
                return Ok(vec![
                    Err(ResourceError::Unsupported(
                        "CurseForge identification requires local fingerprint"
                            .into()
                    ));
                    queries.len()
                ]);
            }
            let response = self
                .request(ProtocolRequest::post(
                    "/v1/fingerprints/432",
                    json!({"fingerprints": fingerprints}),
                ))
                .await?;
            let matches: FingerprintResponse =
                serde_json::from_value(response["data"].clone())?;
            let mut candidates: HashMap<u32, Vec<ReleaseDocument>> =
                HashMap::new();
            for matched in matches.exact_matches {
                let fingerprint = matched.file.file_fingerprint;
                if matched.id != matched.file.mod_id {
                    return Err(ResourceError::InvalidResponse(
                        "fingerprint project mismatch".into(),
                    ));
                }
                candidates.entry(fingerprint).or_default().push(
                    matched
                        .file
                        .document_with_download_policy(bypass_restrictions)?,
                );
            }
            return Ok(queries
                .iter()
                .map(|query| match query {
                    ProviderQuery::Identify {
                        fingerprint: Some(fingerprint),
                        ..
                    } => Ok(ProviderValue::Matches(
                        candidates
                            .get(fingerprint)
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .flat_map(|document| document.release.artifacts)
                            .collect(),
                    )),
                    ProviderQuery::IdentifyExport {
                        fingerprint: Some(fingerprint),
                        ..
                    } => Ok(ProviderValue::Identified(
                        candidates
                            .get(fingerprint)
                            .cloned()
                            .unwrap_or_default(),
                    )),
                    _ => Err(ResourceError::Unsupported(
                        "CurseForge identification requires local fingerprint"
                            .into(),
                    )),
                })
                .collect());
        }
        let mut results = vec![];
        for query in queries {
            results.push(self.fetch_one(query, bypass_restrictions).await);
        }
        Ok(results)
    }

    async fn fill_manual_links(
        &self,
        documents: &mut [ReleaseDocument],
    ) -> ResourceResult<()> {
        let mut ids: Vec<_> = documents
            .iter()
            .filter(|document| {
                document.release.artifacts.iter().any(|artifact| {
                    artifact.available != Some(false)
                        && matches!(
                            artifact.acquisition,
                            ArtifactAcquisition::Unavailable
                        )
                })
            })
            .map(|document| {
                positive_id(
                    document.release.reference.project.project_id.as_str(),
                )
            })
            .collect::<ResourceResult<_>>()?;
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Ok(());
        }
        let response = self
            .request(ProtocolRequest::post(
                "/v1/mods",
                json!({"modIds": ids, "filterPcOnly": true}),
            ))
            .await?;
        let projects: Vec<RemoteProject> =
            serde_json::from_value(response["data"].clone())?;
        let links: HashMap<_, _> = projects
            .into_iter()
            .filter_map(|project| {
                project
                    .links
                    .and_then(|links| {
                        links["websiteUrl"].as_str().map(str::to_owned)
                    })
                    .and_then(|link| public_url(&link))
                    .map(|link| (project.id.to_string(), link))
            })
            .collect();
        for document in documents {
            if let Some(link) = links
                .get(document.release.reference.project.project_id.as_str())
            {
                for artifact in &mut document.release.artifacts {
                    if artifact.available != Some(false)
                        && matches!(
                            artifact.acquisition,
                            ArtifactAcquisition::Unavailable
                        )
                    {
                        artifact.acquisition = acquisition(
                            vec![],
                            Some(format!(
                                "{}/files/{}",
                                link.trim_end_matches('/'),
                                document.release.reference.release_id.as_str()
                            )),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    async fn fetch_one(
        &self,
        query: &ProviderQuery,
        bypass_restrictions: bool,
    ) -> ResourceResult<ProviderValue> {
        match query {
            ProviderQuery::Description { id, .. } => {
                let id = positive_id(id.as_str())?;
                let response = self
                    .request(ProtocolRequest::get(format!(
                        "/v1/mods/{id}/description"
                    )))
                    .await?;
                let body = serde_json::from_value(response["data"].clone())?;
                Ok(ProviderValue::Description(ResourceText {
                    source: SourceId::curseforge(),
                    format: TextFormat::Html,
                    body,
                }))
            }
            ProviderQuery::Changelog {
                project_id,
                release_id,
                ..
            } => {
                let project = positive_id(project_id.as_str())?;
                let file = positive_id(release_id.as_str())?;
                let metadata = self
                    .request(ProtocolRequest::get(format!(
                        "/v1/mods/{project}/files/{file}"
                    )))
                    .await?;
                let metadata: RemoteFile =
                    serde_json::from_value(metadata["data"].clone())?;
                if metadata.mod_id != project || metadata.id != file {
                    return Err(ResourceError::InvalidResponse(
                        "changelog release does not belong to its project"
                            .into(),
                    ));
                }
                let response = self
                    .request(ProtocolRequest::get(format!(
                        "/v1/mods/{project}/files/{file}/changelog"
                    )))
                    .await?;
                Ok(ProviderValue::Changelog(ResourceText {
                    source: SourceId::curseforge(),
                    format: TextFormat::Html,
                    body: serde_json::from_value(response["data"].clone())?,
                }))
            }
            ProviderQuery::Releases {
                project_id,
                target,
                cursor,
                page_size,
                ..
            } => {
                let id = positive_id(project_id.as_str())?;
                let start = offset(cursor)?;
                let mut request =
                    ProtocolRequest::get(format!("/v1/mods/{id}/files"));
                request.query = pagination(start, *page_size)?;
                add_target(&mut request, target)?;
                let response = self.request(request).await?;
                let files: Vec<RemoteFile> =
                    serde_json::from_value(response["data"].clone())?;
                let count = files.len();
                let mut documents = files
                    .into_iter()
                    .map(|file| {
                        file.document_with_download_policy(bypass_restrictions)
                    })
                    .collect::<ResourceResult<Vec<_>>>()?;
                self.fill_manual_links(&mut documents).await?;
                let entries = documents
                    .into_iter()
                    .map(|document| document.release)
                    .filter(|release| {
                        target.channels.is_empty()
                            || target.channels.contains(&release.channel)
                    })
                    .collect();
                Ok(ProviderValue::Releases(page(
                    response, entries, start, count, *page_size,
                )?))
            }
            ProviderQuery::Search { query, cursor } => {
                if let Some(expression) = &query.filter {
                    search_filters::validate_curseforge(
                        &search_filters::for_source(
                            expression,
                            &SourceId::curseforge(),
                        )?,
                        0,
                    )?;
                }
                let query = search_filters::merge_positive_filters(
                    query,
                    &SourceId::curseforge(),
                )?;
                if query.types.len() != 1 {
                    return Err(ResourceError::Unsupported("CurseForge search requires one content class per request".into()));
                }
                let class = match query.types[0] {
                    ResourceType::Mod => 6,
                    ResourceType::Plugin => 5,
                    ResourceType::ResourcePack => 12,
                    ResourceType::Shader => 6552,
                    ResourceType::ModPack => 4471,
                    ResourceType::World => 17,
                    ResourceType::DataPack => 6945,
                };
                let sort = match query.sort {
                    ResourceSearchSort::Relevance => 1,
                    ResourceSearchSort::Downloads => 6,
                    ResourceSearchSort::ProjectUpdated => 3,
                    _ => return Err(ResourceError::Unsupported(
                        "CurseForge cannot provide requested sort semantics"
                            .into(),
                    )),
                };
                let start = offset(cursor)?;
                let mut request = ProtocolRequest::get("/v1/mods/search");
                request.query = pagination(start, query.page_size)?;
                request.query.extend([
                    ("gameId".into(), "432".into()),
                    ("classId".into(), class.to_string()),
                    ("searchFilter".into(), query.text.clone()),
                    ("sortField".into(), sort.to_string()),
                    ("sortOrder".into(), "desc".into()),
                ]);
                add_target(&mut request, &query.target)?;
                if query.categories.len() > 10 {
                    return Err(ResourceError::InvalidRequest(
                        "too many CurseForge categories".into(),
                    ));
                }
                if !query.categories.is_empty() {
                    let categories = query
                        .categories
                        .iter()
                        .map(|id| positive_id(id))
                        .collect::<ResourceResult<Vec<_>>>()?;
                    request.query.push((
                        "categoryIds".into(),
                        serde_json::to_string(&categories)?,
                    ));
                }
                let response = self.request(request).await?;
                let projects: Vec<RemoteProject> =
                    serde_json::from_value(response["data"].clone())?;
                let count = projects.len();
                let entries = projects
                    .into_iter()
                    .map(|project| {
                        project.document().map(|document| document.metadata)
                    })
                    .collect::<ResourceResult<Vec<_>>>()?;
                Ok(ProviderValue::Search(page(
                    response,
                    entries,
                    start,
                    count,
                    query.page_size,
                )?))
            }
            _ => Err(ResourceError::Unsupported(
                "invalid CurseForge batch".into(),
            )),
        }
    }
}

#[async_trait]
impl ResourceProvider for CurseForgeProvider {
    fn source(&self) -> ResourceSource {
        let capability = crate::api::curseforge::capability();
        ResourceSource {
            id: SourceId::curseforge(),
            available: capability.configured
                && matches!(
                    capability.status,
                    crate::api::curseforge::CurseForgeCapabilityStatus::Ready
                ),
            types: vec![
                ResourceType::Mod,
                ResourceType::Plugin,
                ResourceType::ResourcePack,
                ResourceType::Shader,
                ResourceType::ModPack,
                ResourceType::World,
                ResourceType::DataPack,
            ],
            capabilities: vec![
                SourceCapability::Search,
                SourceCapability::Project,
                SourceCapability::Description,
                SourceCapability::Releases,
                SourceCapability::IdentifyFingerprint,
                SourceCapability::Dependencies,
                SourceCapability::Download,
                SourceCapability::Changelog,
            ],
            supported_search_sorts: vec![
                ResourceSearchSort::Relevance,
                ResourceSearchSort::Downloads,
                ResourceSearchSort::ProjectUpdated,
            ],
        }
    }
    fn namespace(&self) -> String {
        if (self.download_restriction_bypass)() {
            "curseforge-v1-dates-auto-download".into()
        } else {
            "curseforge-v1-dates".into()
        }
    }
    fn batch_limit(&self, query: &ProviderQuery) -> usize {
        if query.batch_group() < 3 { 50 } else { 1 }
    }
    fn validate_query(&self, query: &ProviderQuery) -> ResourceResult<()> {
        match query {
            ProviderQuery::Project { id, .. }
            | ProviderQuery::Description { id, .. } => {
                positive_id(id.as_str())?;
            }
            ProviderQuery::Releases { project_id, .. } => {
                positive_id(project_id.as_str())?;
            }
            ProviderQuery::Release {
                project_id,
                release_id,
                ..
            } => {
                if let Some(id) = project_id {
                    positive_id(id.as_str())?;
                }
                positive_id(release_id.as_str())?;
            }
            ProviderQuery::Changelog {
                project_id,
                release_id,
                ..
            } => {
                positive_id(project_id.as_str())?;
                positive_id(release_id.as_str())?;
            }
            _ => {}
        }
        Ok(())
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        let bypass_restrictions = (self.download_restriction_bypass)();
        match self.fetch_batch(queries, bypass_restrictions).await {
            Ok(values) => values,
            Err(ResourceError::Network { status: Some(404) }) => {
                vec![Ok(ProviderValue::NotFound); queries.len()]
            }
            Err(error) => vec![Err(error); queries.len()],
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteProject {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    id: u32,
    name: String,
    slug: String,
    #[serde(default)]
    links: Option<Value>,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    class_id: Option<u32>,
    #[serde(default)]
    logo: Option<Value>,
    #[serde(default)]
    authors: Vec<Value>,
    #[serde(default)]
    download_count: Option<u64>,
    #[serde(default)]
    date_created: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    date_modified: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    date_released: Option<chrono::DateTime<chrono::Utc>>,
}

impl RemoteProject {
    pub(crate) fn document(self) -> ResourceResult<ProjectDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        positive_id(&self.id.to_string())?;
        let types = match self.class_id {
            Some(6) => vec![ResourceType::Mod],
            Some(5) => vec![ResourceType::Plugin],
            Some(12) => vec![ResourceType::ResourcePack],
            Some(6552) => vec![ResourceType::Shader],
            Some(4471) => vec![ResourceType::ModPack],
            Some(17) => vec![ResourceType::World],
            Some(6945) => vec![ResourceType::DataPack],
            _ => vec![],
        };
        Ok(ProjectDocument {
            protocol,
            metadata: SourceProjectMetadata {
                environments: vec![],
                categories: self
                    .extra
                    .get("categories")
                    .and_then(Value::as_array)
                    .map(|categories| {
                        categories
                            .iter()
                            .filter_map(|category| {
                                category["slug"].as_str().map(str::to_owned)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                game_versions: self
                    .extra
                    .get("latestFilesIndexes")
                    .and_then(Value::as_array)
                    .map(|indexes| {
                        indexes
                            .iter()
                            .filter_map(|index| {
                                index["gameVersion"].as_str().map(str::to_owned)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                loaders: vec![],
                gallery: self
                    .extra
                    .get("screenshots")
                    .and_then(Value::as_array)
                    .map(|images| {
                        images
                            .iter()
                            .filter_map(|image| {
                                image["url"].as_str().map(str::to_owned)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                project: source_project(
                    SourceId::curseforge(),
                    self.id.to_string(),
                )?,
                slug: Some(self.slug),
                title: self.name,
                summary: self.summary,
                icon_url: self
                    .logo
                    .and_then(|logo| logo["url"].as_str().map(str::to_owned)),
                types,
                authors: self
                    .authors
                    .into_iter()
                    .filter_map(|author| {
                        author["name"].as_str().map(str::to_owned)
                    })
                    .collect(),
                license: None,
                downloads: self.download_count,
                followers: None,
                created_at: self.date_created,
                project_published_at: None,
                updated_at: self.date_modified,
                first_published_at: None,
                latest_published_at: self.date_released,
            },
            description: ResourceText {
                source: SourceId::curseforge(),
                format: TextFormat::Html,
                body: String::new(),
            },
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteFile {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    id: u32,
    mod_id: u32,
    file_name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_date: Option<chrono::DateTime<chrono::Utc>>,
    file_length: u64,
    #[serde(default)]
    release_type: u32,
    #[serde(default)]
    hashes: Option<Vec<RemoteHash>>,
    #[serde(default)]
    game_versions: Option<Vec<String>>,
    #[serde(default)]
    dependencies: Option<Vec<RemoteDependency>>,
    #[serde(default)]
    file_fingerprint: u32,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    is_server_pack: Option<bool>,
    #[serde(default)]
    is_available: Option<bool>,
}

#[derive(Deserialize, Serialize)]
struct RemoteHash {
    algo: u32,
    value: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDependency {
    mod_id: u32,
    relation_type: u32,
}

/// CurseForge CDN layout for a file the API only exposes by id.
pub(crate) fn derived_download_url(
    file_id: u32,
    file_name: &str,
) -> ResourceResult<String> {
    positive_id(&file_id.to_string())?;
    crate::util::io::sanitize_download_file_name(file_name)
        .map_err(|error| ResourceError::InvalidResponse(error.to_string()))?;
    Ok(format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        file_id / 1000,
        file_id % 1000,
        urlencoding::encode(file_name)
    ))
}

impl RemoteFile {
    fn document_with_download_policy(
        self,
        bypass_restrictions: bool,
    ) -> ResourceResult<ReleaseDocument> {
        let mut document = self.document()?;
        if bypass_restrictions {
            for artifact in &mut document.release.artifacts {
                if artifact.available != Some(false)
                    && !matches!(
                        artifact.acquisition,
                        ArtifactAcquisition::PublicDownload { .. }
                    )
                    && let Some(filename) = &artifact.file_name
                {
                    let file_id =
                        positive_id(artifact.reference.artifact_id.as_str())?;
                    let url = derived_download_url(file_id, filename)?;
                    artifact.acquisition = acquisition(vec![url], None);
                }
            }
        }
        Ok(document)
    }

    pub(crate) fn document(self) -> ResourceResult<ReleaseDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        positive_id(&self.id.to_string())?;
        positive_id(&self.mod_id.to_string())?;
        let reference = release_ref(
            source_project(SourceId::curseforge(), self.mod_id.to_string())?,
            self.id.to_string(),
        )?;
        let file_hashes =
            hashes(self.hashes.unwrap_or_default().into_iter().filter_map(
                |hash| match hash.algo {
                    1 => Some((HashAlgorithm::Sha1, hash.value)),
                    2 => Some((HashAlgorithm::Md5, hash.value)),
                    _ => None,
                },
            ))?;
        let mut loaders = vec![];
        let mut game_versions = vec![];
        for version in self.game_versions.unwrap_or_default() {
            match version.to_ascii_lowercase().as_str() {
                "forge" | "fabric" | "quilt" | "neoforge" | "liteloader"
                | "rift" => loaders.push(version.to_ascii_lowercase()),
                "client" | "server" => {}
                _ if version
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_digit()) =>
                {
                    game_versions.push(version)
                }
                _ => {}
            }
        }
        let mut dependencies = vec![];
        for dependency in self.dependencies.unwrap_or_default() {
            let kind =
                match DependencyRelation::from_raw(dependency.relation_type) {
                    DependencyRelation::Embedded => {
                        ResourceDependencyKind::Embedded
                    }
                    DependencyRelation::Optional
                    | DependencyRelation::Tool
                    | DependencyRelation::Include => {
                        ResourceDependencyKind::Optional
                    }
                    DependencyRelation::Required => {
                        ResourceDependencyKind::Required
                    }
                    DependencyRelation::Incompatible => {
                        ResourceDependencyKind::Incompatible
                    }
                    DependencyRelation::Unknown(_) => {
                        return Err(ResourceError::InvalidResponse(
                            "unknown dependency relation".into(),
                        ));
                    }
                };
            positive_id(&dependency.mod_id.to_string())?;
            dependencies.push(ArtifactDependency {
                target: DependencyTarget::Resource {
                    reference: ResourceRef::Source {
                        project: source_project(
                            SourceId::curseforge(),
                            dependency.mod_id.to_string(),
                        )?,
                    },
                },
                kind,
                version_constraint: None,
            });
        }
        let urls = if self.is_available == Some(false) {
            vec![]
        } else {
            self.download_url.into_iter().collect()
        };
        let artifact = ResourceArtifact {
            reference: SourceArtifactRef {
                release: reference.clone(),
                artifact_id: SourceArtifactId::new(self.id.to_string())?,
            },
            file_name: Some(self.file_name),
            available: self.is_available,
            role: if self.is_server_pack == Some(true) {
                ArtifactRole::ServerPack
            } else {
                ArtifactRole::Primary
            },
            integrity: ArtifactIntegrity {
                size: Some(self.file_length),
                hashes: file_hashes,
            },
            acquisition: acquisition(urls, None),
            dependencies,
        };
        Ok(ReleaseDocument {
            protocol,
            release: ResourceRelease {
                reference,
                version: self.display_name,
                channel: match self.release_type {
                    1 => ReleaseChannel::Release,
                    2 => ReleaseChannel::Beta,
                    3 => ReleaseChannel::Alpha,
                    _ => ReleaseChannel::Unknown,
                },
                published_at: self.file_date,
                compatibility: ReleaseCompatibility {
                    game_versions,
                    loaders,
                },
                artifacts: vec![artifact],
                equivalent_releases: vec![],
            },
            changelog: None,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintResponse {
    #[serde(default)]
    exact_matches: Vec<FingerprintMatch>,
}

#[derive(Deserialize, Serialize)]
struct FingerprintMatch {
    id: u32,
    file: RemoteFile,
}

fn pagination(
    start: usize,
    size: u32,
) -> ResourceResult<Vec<(String, String)>> {
    if !(1..=50).contains(&size) || start >= 10000 {
        return Err(ResourceError::InvalidRequest(
            "CurseForge pagination window or page size exceeded".into(),
        ));
    }
    Ok(vec![
        ("index".into(), start.to_string()),
        (
            "pageSize".into(),
            size.min(10000 - start as u32).to_string(),
        ),
    ])
}

fn add_target(
    request: &mut ProtocolRequest,
    target: &ResourceTarget,
) -> ResourceResult<()> {
    if target.game_versions.len() > 1 || target.loaders.len() > 1 {
        return Err(ResourceError::Unsupported(
            "CurseForge supports one game version and loader per request"
                .into(),
        ));
    }
    if let Some(version) = target.game_versions.first() {
        request.query.push(("gameVersion".into(), version.clone()));
    }
    if let Some(loader) = target.loaders.first() {
        let value = match loader.as_str() {
            "forge" => 1,
            "liteloader" => 3,
            "fabric" => 4,
            "quilt" => 5,
            "neoforge" => 6,
            _ => {
                return Err(ResourceError::Unsupported(
                    "unknown CurseForge loader".into(),
                ));
            }
        };
        request
            .query
            .push(("modLoaderType".into(), value.to_string()));
    }
    Ok(())
}

fn page<T>(
    response: Value,
    entries: Vec<T>,
    start: usize,
    count: usize,
    size: u32,
) -> ResourceResult<SourcePage<T>> {
    if count > size as usize
        || response["pagination"]["index"]
            .as_u64()
            .is_some_and(|index| index != start as u64)
        || response["pagination"]["resultCount"]
            .as_u64()
            .is_some_and(|reported| reported != count as u64)
    {
        return Err(ResourceError::InvalidResponse(
            "source pagination does not match the requested page".into(),
        ));
    }
    let total = response["pagination"]["totalCount"].as_u64();
    let end = start.checked_add(count).ok_or_else(|| {
        ResourceError::InvalidResponse("pagination overflow".into())
    })?;
    let more = count > 0
        && total.map_or(count == size as usize, |total| (end as u64) < total);
    let exhausted = (count > 0 && end >= 10000)
        || (count == 0 && total.is_some_and(|total| total > start as u64));
    Ok(SourcePage {
        entries,
        cursor: (more && !exhausted).then(|| end.to_string()),
        total: ResourceTotal {
            value: total,
            precision: if total == Some(10000) {
                TotalPrecision::Estimate
            } else if total.is_some() {
                TotalPrecision::Exact
            } else {
                TotalPrecision::Unknown
            },
        },
        window_exhausted: exhausted,
    })
}
