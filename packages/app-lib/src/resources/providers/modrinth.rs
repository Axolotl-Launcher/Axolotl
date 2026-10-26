use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

use super::*;

pub struct ModrinthProvider {
    transport: Arc<dyn ProtocolTransport>,
}

impl ModrinthProvider {
    pub fn new(transport: Arc<dyn ProtocolTransport>) -> Self {
        Self { transport }
    }

    async fn request(&self, request: ProtocolRequest) -> ResourceResult<Value> {
        self.transport.request(&SourceId::modrinth(), request).await
    }

    async fn fetch_batch(
        &self,
        queries: &[ProviderQuery],
    ) -> ResourceResult<Vec<ResourceResult<ProviderValue>>> {
        if queries.iter().all(|query| {
            matches!(query, ProviderQuery::ReleaseProtocol { version, .. } if version == "v3")
        }) {
            let ids: Vec<_> = queries.iter().filter_map(|query| {
                if let ProviderQuery::ReleaseProtocol { release_id, .. } = query {
                    Some(release_id.as_str())
                } else {
                    None
                }
            }).collect();
            let mut request = ProtocolRequest::get("/v3/versions");
            request.query.push(("ids".into(), serde_json::to_string(&ids)?));
            let values: Vec<Value> = serde_json::from_value(self.request(request).await?)?;
            let mut protocols = HashMap::new();
            for value in values {
                let id = value["id"].as_str().ok_or_else(|| ResourceError::InvalidResponse("release protocol ID missing".into()))?.to_owned();
                if !ids.contains(&id.as_str()) || protocols.insert(id, protocol_projection(value)).is_some() {
                    return Err(ResourceError::InvalidResponse("unexpected or duplicate release protocol".into()));
                }
            }
            return Ok(ids.into_iter().map(|id| {
                Ok(protocols.get(id).cloned().map(ProviderValue::Protocol).unwrap_or(ProviderValue::NotFound))
            }).collect());
        }
        if queries.iter().all(|query| matches!(query, ProviderQuery::ProjectProtocol { version, .. } if version == "v3")) {
            let locators: Vec<_> = queries.iter().filter_map(|query| if let ProviderQuery::ProjectProtocol { locator, .. } = query { Some(locator.as_str()) } else { None }).collect();
            let mut request = ProtocolRequest::get("/v3/projects");
            request.query.push(("ids".into(), serde_json::to_string(&locators)?));
            let values: Vec<Value> = serde_json::from_value(self.request(request).await?)?;
            return Ok(locators.iter().map(|locator| values.iter().find(|value| value["id"].as_str() == Some(locator) || value["slug"].as_str().is_some_and(|slug| slug.eq_ignore_ascii_case(locator))).map(|value| protocol_document(value.clone()).map(ProviderValue::Project)).unwrap_or(Ok(ProviderValue::NotFound))).collect());
        }
        if queries
            .iter()
            .all(|query| matches!(query, ProviderQuery::Project { .. }))
        {
            let ids: Vec<_> = queries
                .iter()
                .filter_map(|query| {
                    if let ProviderQuery::Project { id, .. } = query {
                        Some(id.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            let mut request = ProtocolRequest::get("/projects");
            request
                .query
                .push(("ids".into(), serde_json::to_string(&ids)?));
            let projects: Vec<RemoteProject> =
                serde_json::from_value(self.request(request).await?)?;
            let mut teams: Vec<_> = projects
                .iter()
                .filter_map(|project| project.team.clone())
                .collect();
            teams.sort();
            teams.dedup();
            let memberships: ResourceResult<HashMap<String, Vec<String>>> =
                if teams.is_empty() {
                    Ok(HashMap::new())
                } else {
                    let mut request = ProtocolRequest::get("/teams");
                    request
                        .query
                        .push(("ids".into(), serde_json::to_string(&teams)?));
                    match self.request(request).await {
                        Ok(value) => {
                            serde_json::from_value::<Vec<Vec<Value>>>(value)
                                .map_err(Into::into)
                                .map(|groups| {
                                    let mut authors =
                                        HashMap::<String, Vec<String>>::new();
                                    for member in groups.into_iter().flatten() {
                                        if member["accepted"] == true
                                            && let (Some(team), Some(name)) = (
                                                member["team_id"].as_str(),
                                                member["user"]["username"]
                                                    .as_str(),
                                            )
                                        {
                                            authors
                                                .entry(team.into())
                                                .or_default()
                                                .push(name.into());
                                        }
                                    }
                                    authors
                                })
                        }
                        Err(error) => Err(error),
                    }
                };
            let mut values = HashMap::new();
            for project in projects {
                let id = project.id.clone();
                if !ids.contains(&id.as_str()) {
                    return Err(ResourceError::InvalidResponse(
                        "unexpected project in batch".into(),
                    ));
                }
                let authors = match &project.team {
                    Some(team) => memberships
                        .as_ref()
                        .map(|members| {
                            members.get(team).cloned().unwrap_or_default()
                        })
                        .map_err(Clone::clone),
                    None => Ok(vec![]),
                };
                let document = authors.and_then(|authors| {
                    project.document().map(|mut document| {
                        document.metadata.authors = authors;
                        ProviderValue::Project(document)
                    })
                });
                if values.insert(id, document).is_some() {
                    return Err(ResourceError::InvalidResponse(
                        "duplicate project in batch".into(),
                    ));
                }
            }
            return Ok(ids
                .into_iter()
                .map(|id| {
                    values
                        .get(id)
                        .cloned()
                        .unwrap_or(Ok(ProviderValue::NotFound))
                })
                .collect());
        }
        if queries
            .iter()
            .all(|query| matches!(query, ProviderQuery::Release { .. }))
        {
            let ids: Vec<_> = queries
                .iter()
                .filter_map(|query| {
                    if let ProviderQuery::Release { release_id, .. } = query {
                        Some(release_id.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            let mut request = ProtocolRequest::get("/versions");
            request
                .query
                .push(("ids".into(), serde_json::to_string(&ids)?));
            let releases: Vec<RemoteRelease> =
                serde_json::from_value(self.request(request).await?)?;
            let mut values = HashMap::new();
            for release in releases {
                let id = release.id.clone();
                if !ids.contains(&id.as_str()) {
                    return Err(ResourceError::InvalidResponse(
                        "unexpected release in batch".into(),
                    ));
                }
                if values.insert(id, release.document()?).is_some() {
                    return Err(ResourceError::InvalidResponse(
                        "duplicate release in batch".into(),
                    ));
                }
            }
            return Ok(ids
                .into_iter()
                .map(|id| {
                    Ok(values
                        .get(id)
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
            let mut by_algorithm: HashMap<&str, Vec<(usize, String)>> =
                HashMap::new();
            let mut result = queries
                .iter()
                .map(|query| {
                    Ok(
                        if matches!(query, ProviderQuery::IdentifyExport { .. })
                        {
                            ProviderValue::Identified(vec![])
                        } else {
                            ProviderValue::Matches(vec![])
                        },
                    )
                })
                .collect::<Vec<_>>();
            for (index, query) in queries.iter().enumerate() {
                let (ProviderQuery::Identify { integrity, .. }
                | ProviderQuery::IdentifyExport { integrity, .. }) = query
                else {
                    continue;
                };
                let hash = integrity
                    .hashes
                    .iter()
                    .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
                    .or_else(|| {
                        integrity.hashes.iter().find(|hash| {
                            hash.algorithm == HashAlgorithm::Sha512
                        })
                    });
                if let Some(hash) = hash {
                    hashes([(hash.algorithm, hash.value.clone())])?;
                    by_algorithm
                        .entry(if hash.algorithm == HashAlgorithm::Sha1 {
                            "sha1"
                        } else {
                            "sha512"
                        })
                        .or_default()
                        .push((index, hash.value.clone()));
                } else {
                    result[index] = Err(ResourceError::Unsupported(
                        "Modrinth identification requires SHA-1 or SHA-512"
                            .into(),
                    ));
                }
            }
            for (algorithm, inputs) in by_algorithm {
                let hashes: Vec<_> =
                    inputs.iter().map(|(_, hash)| hash).collect();
                let response: HashMap<String, RemoteRelease> =
                    serde_json::from_value(
                        self.request(ProtocolRequest::post(
                            "/version_files",
                            json!({"hashes": hashes, "algorithm": algorithm}),
                        ))
                        .await?,
                    )?;
                let mut documents = HashMap::new();
                for (hash, release) in response {
                    documents
                        .insert(hash.to_ascii_lowercase(), release.document()?);
                }
                for (index, hash) in inputs {
                    if let Some(document) = documents.get(&hash) {
                        let matching: Vec<_> = document
                            .release
                            .artifacts
                            .iter()
                            .filter(|artifact| {
                                artifact.integrity.hashes.iter().any(
                                    |candidate| {
                                        candidate.value == hash
                                            && candidate.algorithm
                                                == if algorithm == "sha1" {
                                                    HashAlgorithm::Sha1
                                                } else {
                                                    HashAlgorithm::Sha512
                                                }
                                    },
                                )
                            })
                            .cloned()
                            .collect();
                        if matching.is_empty() {
                            result[index] = Err(ResourceError::InvalidResponse("hash lookup contradicted its artifact metadata".into()));
                            continue;
                        }
                        result[index] = Ok(
                            if matches!(
                                queries[index],
                                ProviderQuery::IdentifyExport { .. }
                            ) {
                                ProviderValue::Identified(vec![
                                    document.clone(),
                                ])
                            } else {
                                ProviderValue::Matches(matching)
                            },
                        );
                    }
                }
            }
            return Ok(result);
        }
        let mut results = Vec::new();
        for query in queries {
            results.push(self.fetch_one(query).await);
        }
        Ok(results)
    }

    async fn fetch_one(
        &self,
        query: &ProviderQuery,
    ) -> ResourceResult<ProviderValue> {
        match query {
            ProviderQuery::ReleasesProtocol { project_id } => {
                let mut request = ProtocolRequest::get(format!(
                    "/project/{}/version",
                    urlencoding::encode(project_id.as_str())
                ));
                request
                    .query
                    .push(("include_changelog".into(), "false".into()));
                let releases: Vec<RemoteRelease> =
                    serde_json::from_value(self.request(request).await?)?;
                let mut protocols = Vec::with_capacity(releases.len());
                for release in releases {
                    if release.project_id != project_id.as_str() {
                        return Err(ResourceError::InvalidResponse(
                            "release project mismatch".into(),
                        ));
                    }
                    let document = release.document()?;
                    protocols.push(document.protocol.ok_or_else(|| {
                        ResourceError::InvalidResponse(
                            "release protocol missing".into(),
                        )
                    })?);
                }
                Ok(ProviderValue::Protocol(Value::Array(protocols)))
            }
            ProviderQuery::SearchProtocol { query, version } => {
                if !matches!(version.as_str(), "v2" | "v3") {
                    return Err(ResourceError::Unsupported(
                        "search protocol version".into(),
                    ));
                }
                let mut request = ProtocolRequest::get(if version == "v3" {
                    "/v3/search"
                } else {
                    "/search"
                });
                request.query = url::form_urlencoded::parse(
                    query.trim_start_matches('?').as_bytes(),
                )
                .into_owned()
                .collect();
                let value = self.request(request).await?;
                if !value["hits"].is_array() || !value["total_hits"].is_u64() {
                    return Err(ResourceError::InvalidResponse(
                        "invalid search protocol response".into(),
                    ));
                }
                Ok(ProviderValue::Protocol(protocol_projection(value)))
            }
            ProviderQuery::Locate { locator } => {
                SourceProjectId::new(locator.clone())?;
                let project: RemoteProject = serde_json::from_value(
                    self.request(ProtocolRequest::get(format!(
                        "/project/{}",
                        urlencoding::encode(locator)
                    )))
                    .await?,
                )?;
                Ok(ProviderValue::Project(project.document()?))
            }
            ProviderQuery::Description { id, .. } => {
                let project: RemoteProject = serde_json::from_value(
                    self.request(ProtocolRequest::get(format!(
                        "/project/{}",
                        urlencoding::encode(id.as_str())
                    )))
                    .await?,
                )?;
                if project.id != id.as_str() {
                    return Err(ResourceError::InvalidResponse(
                        "project mismatch".into(),
                    ));
                }
                Ok(ProviderValue::Description(project.document()?.description))
            }
            ProviderQuery::Changelog {
                project_id,
                release_id,
                ..
            } => {
                let release: RemoteRelease = serde_json::from_value(
                    self.request(ProtocolRequest::get(format!(
                        "/version/{}",
                        urlencoding::encode(release_id.as_str())
                    )))
                    .await?,
                )?;
                if release.project_id != project_id.as_str()
                    || release.id != release_id.as_str()
                {
                    return Err(ResourceError::InvalidResponse(
                        "release mismatch".into(),
                    ));
                }
                Ok(ProviderValue::Changelog(ResourceText {
                    source: SourceId::modrinth(),
                    format: TextFormat::Markdown,
                    body: release.changelog.unwrap_or_default(),
                }))
            }
            ProviderQuery::Releases {
                project_id,
                target,
                cursor,
                page_size,
                ..
            } => {
                let mut request = ProtocolRequest::get(format!(
                    "/project/{}/version",
                    urlencoding::encode(project_id.as_str())
                ));
                request
                    .query
                    .push(("include_changelog".into(), "false".into()));
                if !target.game_versions.is_empty() {
                    request.query.push((
                        "game_versions".into(),
                        serde_json::to_string(&target.game_versions)?,
                    ));
                }
                if !target.loaders.is_empty() {
                    request.query.push((
                        "loaders".into(),
                        serde_json::to_string(&target.loaders)?,
                    ));
                }
                let releases: Vec<RemoteRelease> =
                    serde_json::from_value(self.request(request).await?)?;
                let releases = releases
                    .into_iter()
                    .map(|release| {
                        release.document().map(|document| document.release)
                    })
                    .collect::<ResourceResult<Vec<_>>>()?;
                let releases = releases
                    .into_iter()
                    .filter(|release| {
                        target.channels.is_empty()
                            || target.channels.contains(&release.channel)
                    })
                    .collect();
                if *page_size == 0 {
                    return Ok(ProviderValue::Releases(SourcePage {
                        entries: releases,
                        cursor: None,
                        total: ResourceTotal {
                            value: None,
                            precision: TotalPrecision::Unknown,
                        },
                        window_exhausted: false,
                    }));
                }
                Ok(ProviderValue::Releases(slice_page(
                    releases,
                    offset(cursor)?,
                    *page_size,
                )?))
            }
            ProviderQuery::Search { query, cursor } => {
                if query.types.contains(&ResourceType::World) {
                    return Err(ResourceError::Unsupported(
                        "Modrinth world search".into(),
                    ));
                }
                let mut facets: Vec<Vec<String>> = vec![];
                if !query.types.is_empty() {
                    facets.push(
                        query
                            .types
                            .iter()
                            .map(|kind| {
                                format!("project_type:{}", type_name(*kind))
                            })
                            .collect(),
                    );
                }
                if !query.target.game_versions.is_empty() {
                    facets.push(
                        query
                            .target
                            .game_versions
                            .iter()
                            .map(|version| format!("versions:{version}"))
                            .collect(),
                    );
                }
                if !query.target.loaders.is_empty() {
                    facets.push(
                        query
                            .target
                            .loaders
                            .iter()
                            .map(|loader| format!("categories:{loader}"))
                            .collect(),
                    );
                }
                facets.extend(
                    query
                        .categories
                        .iter()
                        .map(|category| vec![format!("categories:{category}")]),
                );
                if query.types.iter().all(|kind| {
                    matches!(
                        kind,
                        ResourceType::Plugin | ResourceType::DataPack
                    )
                }) && !query.types.is_empty()
                {
                    facets.push(
                        query
                            .types
                            .iter()
                            .flat_map(|kind| match kind {
                                ResourceType::DataPack => {
                                    vec!["categories:datapack".into()]
                                }
                                _ => [
                                    "paper",
                                    "spigot",
                                    "bukkit",
                                    "purpur",
                                    "sponge",
                                    "folia",
                                    "velocity",
                                    "waterfall",
                                    "bungeecord",
                                ]
                                .map(|loader| format!("categories:{loader}"))
                                .to_vec(),
                            })
                            .collect(),
                    );
                }
                let index = match query.sort { ResourceSearchSort::Relevance => "relevance", ResourceSearchSort::Downloads => "downloads", ResourceSearchSort::Followers => "follows", ResourceSearchSort::ProjectUpdated => "updated", ResourceSearchSort::ProjectPublished => "newest", _ => return Err(ResourceError::Unsupported("Modrinth search exposes publication dates, not project creation or exact release-date ordering".into())) };
                let start = offset(cursor)?;
                if !(1..=100).contains(&query.page_size) {
                    return Err(ResourceError::InvalidRequest(
                        "invalid page size".into(),
                    ));
                }
                let mut request =
                    ProtocolRequest::get(if query.filter.is_some() {
                        "/v3/search"
                    } else {
                        "/search"
                    });
                request.query = vec![
                    ("query".into(), query.text.clone()),
                    ("limit".into(), query.page_size.to_string()),
                    ("offset".into(), start.to_string()),
                    ("index".into(), index.into()),
                    ("facets".into(), serde_json::to_string(&facets)?),
                ];
                if let Some(expression) = &query.filter {
                    let expression = search_filters::for_source(
                        expression,
                        &SourceId::modrinth(),
                    )?;
                    let mut terms = vec![expression];
                    for category in &query.categories {
                        terms.push(ResourceSearchExpression::Predicate {
                            field: "category".into(),
                            values: vec![category.clone()],
                            excluded: false,
                        });
                    }
                    if !query.types.is_empty() {
                        terms.push(ResourceSearchExpression::Predicate {
                            field: "type".into(),
                            values: query
                                .types
                                .iter()
                                .map(|kind| {
                                    match kind {
                                        ResourceType::Plugin => "plugin",
                                        ResourceType::DataPack => "datapack",
                                        _ => type_name(*kind),
                                    }
                                    .to_string()
                                })
                                .collect(),
                            excluded: false,
                        });
                    }
                    if !query.target.game_versions.is_empty() {
                        terms.push(ResourceSearchExpression::Predicate {
                            field: "gameVersion".into(),
                            values: query.target.game_versions.clone(),
                            excluded: false,
                        });
                    }
                    if !query.target.loaders.is_empty() {
                        terms.push(ResourceSearchExpression::Predicate {
                            field: "loader".into(),
                            values: query.target.loaders.clone(),
                            excluded: false,
                        });
                    }
                    request.query.retain(|(key, _)| key != "facets");
                    request.query.push((
                        "new_filters".into(),
                        search_filters::modrinth(
                            &ResourceSearchExpression::All { terms },
                            0,
                        )?,
                    ));
                }
                let result: SearchResponse =
                    serde_json::from_value(self.request(request).await?)?;
                let count = result.hits.len();
                let entries = result
                    .hits
                    .into_iter()
                    .map(|hit| hit.metadata())
                    .collect::<ResourceResult<Vec<_>>>()?;
                Ok(ProviderValue::Search(SourcePage {
                    entries,
                    cursor: (count > 0
                        && start.saturating_add(count) < result.total_hits)
                        .then(|| (start + count).to_string()),
                    total: ResourceTotal {
                        value: Some(result.total_hits as u64),
                        precision: TotalPrecision::Exact,
                    },
                    window_exhausted: false,
                }))
            }
            _ => {
                Err(ResourceError::Unsupported("invalid Modrinth batch".into()))
            }
        }
    }
}

#[async_trait]
impl ResourceProvider for ModrinthProvider {
    fn local_release_pagination(&self) -> bool {
        true
    }
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: SourceId::modrinth(),
            available: true,
            types: vec![
                ResourceType::Mod,
                ResourceType::Plugin,
                ResourceType::DataPack,
                ResourceType::ResourcePack,
                ResourceType::Shader,
                ResourceType::ModPack,
            ],
            capabilities: vec![
                SourceCapability::Search,
                SourceCapability::Project,
                SourceCapability::Description,
                SourceCapability::Releases,
                SourceCapability::IdentifyHash,
                SourceCapability::Dependencies,
                SourceCapability::Download,
                SourceCapability::Changelog,
            ],
            supported_search_sorts: vec![
                ResourceSearchSort::Relevance,
                ResourceSearchSort::Downloads,
                ResourceSearchSort::Followers,
                ResourceSearchSort::ProjectPublished,
                ResourceSearchSort::ProjectUpdated,
            ],
        }
    }
    fn namespace(&self) -> String {
        "modrinth-v2-dates".into()
    }
    fn batch_limit(&self, query: &ProviderQuery) -> usize {
        if matches!(
            query,
            ProviderQuery::ProjectProtocol { .. }
                | ProviderQuery::ReleaseProtocol { .. }
        ) {
            return 100;
        }
        if query.batch_group() < 3 { 100 } else { 1 }
    }
    fn validate_query(&self, query: &ProviderQuery) -> ResourceResult<()> {
        let valid_id = |id: &str| -> ResourceResult<()> {
            if id.len() == 8
                && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                Ok(())
            } else {
                Err(ResourceError::InvalidRequest(
                    "expected a stable Modrinth base62 ID".into(),
                ))
            }
        };
        match query {
            ProviderQuery::ReleaseProtocol {
                release_id,
                version,
            } => {
                valid_id(release_id.as_str())?;
                if version != "v3" {
                    return Err(ResourceError::Unsupported(
                        "release protocol version".into(),
                    ));
                }
            }
            ProviderQuery::ReleasesProtocol { project_id } => {
                valid_id(project_id.as_str())?
            }
            ProviderQuery::ProjectProtocol { locator, version } => {
                SourceProjectId::new(locator.clone())?;
                if version != "v3" {
                    return Err(ResourceError::Unsupported(
                        "unknown Modrinth project protocol".into(),
                    ));
                }
            }
            ProviderQuery::Project { id, .. }
            | ProviderQuery::Description { id, .. } => valid_id(id.as_str())?,
            ProviderQuery::Release {
                project_id,
                release_id,
                ..
            } => {
                if let Some(id) = project_id {
                    valid_id(id.as_str())?;
                }
                valid_id(release_id.as_str())?;
            }
            ProviderQuery::Releases { project_id, .. } => {
                valid_id(project_id.as_str())?
            }
            ProviderQuery::Changelog {
                project_id,
                release_id,
                ..
            } => {
                valid_id(project_id.as_str())?;
                valid_id(release_id.as_str())?;
            }
            ProviderQuery::Identify { integrity, .. }
            | ProviderQuery::IdentifyExport { integrity, .. } => {
                hashes(
                    integrity
                        .hashes
                        .iter()
                        .map(|hash| (hash.algorithm, hash.value.clone())),
                )?;
            }
            _ => {}
        }
        Ok(())
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        match self.fetch_batch(queries).await {
            Ok(values) => values,
            Err(ResourceError::Network { status: Some(404) }) => {
                vec![Ok(ProviderValue::NotFound); queries.len()]
            }
            Err(error) => vec![Err(error); queries.len()],
        }
    }
}

fn protocol_document(raw: Value) -> ResourceResult<ProjectDocument> {
    let mut normalized = raw.clone();
    normalized["title"] = json!(
        raw["name"]
            .as_str()
            .or_else(|| raw["title"].as_str())
            .or_else(|| raw["id"].as_str())
            .unwrap_or_default()
    );
    normalized["description"] =
        json!(raw["summary"].as_str().unwrap_or_default());
    normalized["body"] = json!(
        raw["description"]
            .as_str()
            .or_else(|| raw["body"].as_str())
            .unwrap_or_default()
    );
    normalized["project_type"] = raw["project_types"]
        .as_array()
        .and_then(|types| types.first())
        .cloned()
        .unwrap_or(json!("unknown"));
    let mut document =
        serde_json::from_value::<RemoteProject>(normalized)?.document()?;
    document.protocol = Some(protocol_projection(raw));
    Ok(document)
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RemoteProject {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    id: String,
    #[serde(default)]
    team: Option<String>,
    slug: Option<String>,
    project_type: String,
    title: String,
    description: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    published: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    approved: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    updated: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    followers: Option<u64>,
    #[serde(default)]
    license: Option<Value>,
    #[serde(default)]
    loaders: Vec<String>,
}

impl RemoteProject {
    pub(crate) fn document(self) -> ResourceResult<ProjectDocument> {
        let mut protocol = protocol_projection(serde_json::to_value(&self)?);
        // The legacy project API represents an unknown team as an empty ID.
        if self.team.is_none() {
            protocol["team"] = json!("");
        }
        // Redacted links cannot satisfy the legacy protocol's required URL.
        if let Some(links) = protocol["donation_urls"].as_array_mut() {
            links.retain(|link| {
                link["url"]
                    .as_str()
                    .is_some_and(|url| !url.trim().is_empty())
            });
        }
        let protocol = Some(protocol);
        let types = resource_types(&self.project_type, &self.loaders);
        Ok(ProjectDocument {
            protocol,
            metadata: SourceProjectMetadata {
                environments: vec![],
                categories: self
                    .extra
                    .get("categories")
                    .and_then(|value| {
                        serde_json::from_value(value.clone()).ok()
                    })
                    .unwrap_or_default(),
                game_versions: self
                    .extra
                    .get("game_versions")
                    .and_then(|value| {
                        serde_json::from_value(value.clone()).ok()
                    })
                    .unwrap_or_default(),
                loaders: self.loaders.clone(),
                gallery: self
                    .extra
                    .get("gallery")
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
                project: source_project(SourceId::modrinth(), self.id)?,
                slug: self.slug,
                title: self.title,
                summary: self.description,
                icon_url: self.icon_url,
                types,
                authors: vec![],
                license: self.license.and_then(|license| {
                    license["id"].as_str().map(str::to_owned)
                }),
                downloads: self.downloads,
                followers: self.followers,
                created_at: self.published,
                project_published_at: self.approved,
                updated_at: self.updated,
                first_published_at: None,
                latest_published_at: None,
            },
            description: ResourceText {
                source: SourceId::modrinth(),
                format: TextFormat::Markdown,
                body: self.body,
            },
        })
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RemoteRelease {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    id: String,
    project_id: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date_published: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    changelog: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    files: Vec<RemoteFile>,
    #[serde(default)]
    dependencies: Vec<RemoteDependency>,
}

#[derive(Deserialize, Serialize)]
struct RemoteFile {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    #[serde(default)]
    id: Option<String>,
    filename: String,
    #[serde(default)]
    url: Option<String>,
    size: u64,
    hashes: HashMap<String, String>,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    file_type: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct RemoteDependency {
    project_id: Option<String>,
    version_id: Option<String>,
    file_name: Option<String>,
    dependency_type: ResourceDependencyKind,
}

impl RemoteRelease {
    pub(crate) fn document(self) -> ResourceResult<ReleaseDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        let reference = release_ref(
            source_project(SourceId::modrinth(), self.project_id)?,
            self.id,
        )?;
        let mut dependencies = vec![];
        for dependency in self.dependencies {
            let target = match (
                dependency.project_id,
                dependency.version_id,
                dependency.file_name,
            ) {
                (Some(project), Some(release), _) => {
                    DependencyTarget::Release {
                        reference: release_ref(
                            source_project(SourceId::modrinth(), project)?,
                            release,
                        )?,
                    }
                }
                (None, Some(release), _) => DependencyTarget::SourceRelease {
                    source: SourceId::modrinth(),
                    release_id: SourceReleaseId::new(release)?,
                },
                (Some(project), None, _) => DependencyTarget::Resource {
                    reference: ResourceRef::Source {
                        project: source_project(SourceId::modrinth(), project)?,
                    },
                },
                (None, None, Some(name)) => DependencyTarget::FileName { name },
                (None, None, None)
                    if dependency.dependency_type
                        == ResourceDependencyKind::Embedded =>
                {
                    continue;
                }
                _ => {
                    return Err(ResourceError::InvalidResponse(
                        "dependency has no target".into(),
                    ));
                }
            };
            dependencies.push(ArtifactDependency {
                target,
                kind: dependency.dependency_type,
                version_constraint: None,
            });
        }
        let mut artifacts = vec![];
        let mut seen = std::collections::HashSet::new();
        for file in self.files {
            let id = SourceArtifactId::new(
                file.id.unwrap_or_else(|| file.filename.clone()),
            )?;
            if !seen.insert(id.clone()) {
                return Err(ResourceError::InvalidResponse(
                    "duplicate release file".into(),
                ));
            }
            let file_hashes =
                hashes(file.hashes.into_iter().filter_map(|(name, hash)| {
                    match name.as_str() {
                        "sha1" => Some((HashAlgorithm::Sha1, hash)),
                        "sha512" => Some((HashAlgorithm::Sha512, hash)),
                        _ => None,
                    }
                }))?;
            let attachment = matches!(
                file.file_type.as_deref(),
                Some("sources-jar" | "dev-jar" | "javadoc-jar" | "signature")
            );
            artifacts.push(ResourceArtifact {
                reference: SourceArtifactRef {
                    release: reference.clone(),
                    artifact_id: id,
                },
                file_name: Some(file.filename),
                role: if file.primary && !attachment {
                    ArtifactRole::Primary
                } else {
                    ArtifactRole::Attachment
                },
                available: None,
                integrity: ArtifactIntegrity {
                    size: Some(file.size),
                    hashes: file_hashes,
                },
                acquisition: acquisition(file.url.into_iter().collect(), None),
                dependencies: dependencies.clone(),
            });
        }
        Ok(ReleaseDocument {
            protocol,
            release: ResourceRelease {
                reference,
                version: self.version_number,
                channel: match self.version_type.as_str() {
                    "release" => ReleaseChannel::Release,
                    "beta" => ReleaseChannel::Beta,
                    "alpha" => ReleaseChannel::Alpha,
                    _ => ReleaseChannel::Unknown,
                },
                published_at: self.date_published,
                compatibility: ReleaseCompatibility {
                    game_versions: self.game_versions,
                    loaders: self.loaders,
                },
                artifacts,
                equivalent_releases: vec![],
            },
            changelog: self.changelog.map(|body| ResourceText {
                source: SourceId::modrinth(),
                format: TextFormat::Markdown,
                body,
            }),
        })
    }
}

#[derive(Deserialize, Serialize)]
struct SearchResponse {
    hits: Vec<SearchHit>,
    total_hits: usize,
}

#[derive(Deserialize, Serialize)]
struct SearchHit {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    project_id: String,
    slug: String,
    #[serde(alias = "name")]
    title: String,
    #[serde(alias = "primary_project_type", default)]
    project_type: String,
    #[serde(default)]
    #[serde(alias = "summary")]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    follows: Option<u64>,
    #[serde(default)]
    date_created: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    date_modified: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    categories: Vec<String>,
}

impl SearchHit {
    fn metadata(self) -> ResourceResult<SourceProjectMetadata> {
        Ok(SourceProjectMetadata {
            environments: self
                .extra
                .get("environment")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            categories: self.categories.clone(),
            game_versions: self
                .extra
                .get("game_versions")
                .or_else(|| self.extra.get("versions"))
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            loaders: self
                .extra
                .get("loaders")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_else(|| {
                    self.categories
                        .iter()
                        .filter(|value| {
                            matches!(
                                value.as_str(),
                                "fabric"
                                    | "forge"
                                    | "quilt"
                                    | "neoforge"
                                    | "paper"
                                    | "bukkit"
                                    | "spigot"
                                    | "datapack"
                                    | "optifine"
                                    | "iris"
                            )
                        })
                        .cloned()
                        .collect()
                }),
            gallery: self
                .extra
                .get("gallery")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            project: source_project(SourceId::modrinth(), self.project_id)?,
            slug: Some(self.slug),
            title: self.title,
            summary: self.description,
            icon_url: self.icon_url,
            types: self
                .extra
                .get("all_project_types")
                .or_else(|| self.extra.get("project_types"))
                .and_then(Value::as_array)
                .map(|types| {
                    types
                        .iter()
                        .filter_map(Value::as_str)
                        .flat_map(|kind| resource_types(kind, &self.categories))
                        .collect()
                })
                .unwrap_or_else(|| {
                    resource_types(&self.project_type, &self.categories)
                }),
            authors: self.author.into_iter().collect(),
            license: self
                .extra
                .get("license")
                .and_then(Value::as_str)
                .map(str::to_owned),
            downloads: self.downloads,
            followers: self.follows,
            created_at: None,
            project_published_at: self.date_created,
            updated_at: self.date_modified,
            first_published_at: None,
            latest_published_at: None,
        })
    }
}

fn type_name(kind: ResourceType) -> &'static str {
    match kind {
        ResourceType::Mod | ResourceType::Plugin | ResourceType::DataPack => {
            "mod"
        }
        ResourceType::ResourcePack => "resourcepack",
        ResourceType::Shader => "shader",
        ResourceType::ModPack => "modpack",
        ResourceType::World => "world",
    }
}

fn resource_types(kind: &str, loaders: &[String]) -> Vec<ResourceType> {
    if kind == "mod" {
        let mut types = vec![];
        if loaders.iter().any(|loader| loader == "datapack") {
            types.push(ResourceType::DataPack);
        }
        if loaders.iter().any(|loader| {
            matches!(
                loader.as_str(),
                "paper"
                    | "spigot"
                    | "bukkit"
                    | "purpur"
                    | "sponge"
                    | "folia"
                    | "velocity"
                    | "waterfall"
                    | "bungeecord"
            )
        }) {
            types.push(ResourceType::Plugin);
        }
        if types.is_empty()
            || loaders.iter().any(|loader| {
                matches!(
                    loader.as_str(),
                    "fabric"
                        | "forge"
                        | "neoforge"
                        | "quilt"
                        | "liteloader"
                        | "rift"
                )
            })
        {
            types.push(ResourceType::Mod);
        }
        types
    } else {
        match kind {
            "plugin" => vec![ResourceType::Plugin],
            "datapack" => vec![ResourceType::DataPack],
            "resourcepack" => vec![ResourceType::ResourcePack],
            "shader" => vec![ResourceType::Shader],
            "modpack" => vec![ResourceType::ModPack],
            _ => vec![],
        }
    }
}
