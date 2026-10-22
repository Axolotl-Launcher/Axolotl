use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;

const PROJECT_FIELDS: &str = "{uuid,slug,name,description,authors{name},mod_versions{uuid,name,description,game_versions{name},files{uuid,name,sha256,page_url,redirect_url,direct_url,archive_url}}}";
const MATCH_FIELDS: &str = "{uuid,name,sha256,page_url,redirect_url,direct_url,archive_url,mod_version{uuid,name,game_versions{name},mod{uuid,slug}}}";

pub struct McArchiveProvider {
    transport: Arc<dyn ProtocolTransport>,
}

impl McArchiveProvider {
    pub fn new(transport: Arc<dyn ProtocolTransport>) -> Self {
        Self { transport }
    }
    async fn request(
        &self,
        mut request: ProtocolRequest,
        fields: &str,
    ) -> ResourceResult<Value> {
        request.fields = Some(fields.into());
        self.transport
            .request(&SourceId::mcarchive(), request)
            .await
    }

    async fn project(
        &self,
        id: &SourceProjectId,
        slug: &Option<String>,
    ) -> ResourceResult<RemoteProject> {
        let slug = slug.as_ref().filter(|slug| !slug.is_empty()).ok_or_else(|| ResourceError::Unsupported("MCArchive requires the project's slug locator alongside its stable UUID".into()))?;
        let project: RemoteProject = serde_json::from_value(
            self.request(
                ProtocolRequest::get(format!(
                    "/mods/by_slug/{}",
                    urlencoding::encode(slug)
                )),
                PROJECT_FIELDS,
            )
            .await?,
        )?;
        if project.uuid != id.as_str() {
            return Err(ResourceError::InvalidResponse(
                "MCArchive project UUID mismatch".into(),
            ));
        }
        Ok(project)
    }

    async fn fetch(
        &self,
        query: &ProviderQuery,
    ) -> ResourceResult<ProviderValue> {
        match query {
            ProviderQuery::ProjectProtocol { .. }
            | ProviderQuery::ReleaseProtocol { .. }
            | ProviderQuery::ReleasesProtocol { .. }
            | ProviderQuery::SearchProtocol { .. } => {
                Err(ResourceError::Unsupported(
                    "MCArchive has no alternative project protocol".into(),
                ))
            }
            ProviderQuery::Locate { locator } => {
                SourceProjectId::new(locator.clone())?;
                let project: RemoteProject = serde_json::from_value(
                    self.request(
                        ProtocolRequest::get(format!(
                            "/mods/by_slug/{}",
                            urlencoding::encode(locator)
                        )),
                        PROJECT_FIELDS,
                    )
                    .await?,
                )?;
                Ok(ProviderValue::Project(project.document()?))
            }
            ProviderQuery::Project { id, slug } => Ok(ProviderValue::Project(
                self.project(id, slug).await?.document()?,
            )),
            ProviderQuery::Description { id, slug } => {
                Ok(ProviderValue::Description(
                    self.project(id, slug).await?.document()?.description,
                ))
            }
            ProviderQuery::Release {
                project_id,
                release_id,
                slug,
            } => {
                let id = project_id.as_ref().ok_or_else(|| {
                    ResourceError::Unsupported(
                        "MCArchive release lookup requires its project UUID"
                            .into(),
                    )
                })?;
                let project = self.project(id, slug).await?;
                let reference =
                    source_project(SourceId::mcarchive(), project.uuid)?;
                let release = project
                    .mod_versions
                    .into_iter()
                    .find(|version| version.uuid == release_id.as_str());
                release
                    .map(|release| {
                        release.document(reference).map(ProviderValue::Release)
                    })
                    .unwrap_or(Ok(ProviderValue::NotFound))
            }
            ProviderQuery::Releases {
                project_id,
                slug,
                target,
                cursor,
                page_size,
            } => {
                if !target.loaders.is_empty() || !target.channels.is_empty() {
                    return Err(ResourceError::Unsupported("MCArchive does not publish loader or release-channel metadata".into()));
                }
                let project = self.project(project_id, slug).await?;
                let reference =
                    source_project(SourceId::mcarchive(), project.uuid)?;
                let releases = project
                    .mod_versions
                    .into_iter()
                    .map(|release| {
                        release
                            .document(reference.clone())
                            .map(|document| document.release)
                    })
                    .collect::<ResourceResult<Vec<_>>>()?;
                let releases = releases
                    .into_iter()
                    .filter(|release| {
                        target.game_versions.is_empty()
                            || release.compatibility.game_versions.iter().any(
                                |version| {
                                    target.game_versions.contains(version)
                                },
                            )
                    })
                    .collect();
                Ok(ProviderValue::Releases(slice_page(
                    releases,
                    offset(cursor)?,
                    *page_size,
                )?))
            }
            ProviderQuery::Search { query, cursor } => {
                validate_archive_search(query)?;
                let query = search_filters::merge_positive_filters(
                    query,
                    &SourceId::mcarchive(),
                )?;
                validate_archive_search(&query)?;
                let mut request = ProtocolRequest::get("/mods/");
                request.query.push(("keyword".into(), query.text.clone()));
                if let Some(version) = query.target.game_versions.first() {
                    request
                        .query
                        .push(("game_version".into(), version.clone()));
                }
                let response = self
                    .request(
                        request,
                        "{uuid,slug,name,description,authors{name}}",
                    )
                    .await?;
                let projects: Vec<RemoteProject> = collection(response)?;
                let entries = projects
                    .into_iter()
                    .map(|project| {
                        project.document().map(|document| document.metadata)
                    })
                    .collect::<ResourceResult<Vec<_>>>()?;
                Ok(ProviderValue::Search(slice_page(
                    entries,
                    offset(cursor)?,
                    query.page_size,
                )?))
            }
            ProviderQuery::Identify { integrity, .. } => {
                let hash = integrity
                    .hashes
                    .iter()
                    .find(|hash| hash.algorithm == HashAlgorithm::Sha256)
                    .ok_or_else(|| {
                        ResourceError::Unsupported(
                            "MCArchive identification requires SHA-256".into(),
                        )
                    })?;
                hashes([(hash.algorithm, hash.value.clone())])?;
                let response = self
                    .request(
                        ProtocolRequest::get(format!(
                            "/files/by_hash/sha256/{}",
                            hash.value
                        )),
                        MATCH_FIELDS,
                    )
                    .await?;
                let files: Vec<RemoteMatchedFile> = if response.is_null() {
                    vec![]
                } else if response.is_object() && response.get("uuid").is_some()
                {
                    vec![serde_json::from_value(response)?]
                } else {
                    collection(response)?
                };
                let mut artifacts = vec![];
                for file in files {
                    let reference = release_ref(
                        source_project(
                            SourceId::mcarchive(),
                            file.mod_version.mod_.uuid,
                        )?,
                        file.mod_version.uuid,
                    )?;
                    artifacts.push(
                        file.file.artifact(reference, ArtifactRole::Unknown)?,
                    );
                }
                Ok(ProviderValue::Matches(artifacts))
            }
            ProviderQuery::Changelog { .. }
            | ProviderQuery::IdentifyExport { .. } => {
                Err(ResourceError::Unsupported(
                    "MCArchive release descriptions are not changelogs".into(),
                ))
            }
        }
    }
}

fn validate_archive_search(query: &ResourceSearchQuery) -> ResourceResult<()> {
    if let Some(expression) = &query.filter {
        validate_archive_filter(expression)?;
    }
    if query.sort != ResourceSearchSort::Relevance
        || query.types.iter().any(|kind| *kind != ResourceType::Mod)
        || query.target.game_versions.len() > 1
        || !query.target.loaders.is_empty()
        || !query.categories.is_empty()
    {
        return Err(ResourceError::Unsupported(
            "MCArchive search supports keyword and one game version".into(),
        ));
    }
    Ok(())
}

fn validate_archive_filter(
    expression: &ResourceSearchExpression,
) -> ResourceResult<()> {
    match expression {
        ResourceSearchExpression::All { terms } => {
            for term in terms {
                validate_archive_filter(term)?;
            }
            Ok(())
        }
        ResourceSearchExpression::Predicate {
            field,
            excluded: false,
            values,
        } if (field == "type" && values == &["mod"])
            || (field == "gameVersion" && values.len() == 1) =>
        {
            Ok(())
        }
        ResourceSearchExpression::Predicate {
            field,
            values,
            excluded: true,
        } if matches!(field.as_str(), "type" | "allTypes")
            && !values.iter().any(|value| value == "mod") =>
        {
            Ok(())
        }
        _ => Err(ResourceError::Unsupported(
            "MCArchive cannot express these search filters".into(),
        )),
    }
}

#[async_trait]
impl ResourceProvider for McArchiveProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource {
            id: SourceId::mcarchive(),
            available: true,
            types: vec![ResourceType::Mod],
            capabilities: vec![
                SourceCapability::Search,
                SourceCapability::Project,
                SourceCapability::Description,
                SourceCapability::Releases,
                SourceCapability::IdentifyHash,
                SourceCapability::Download,
            ],
            supported_search_sorts: vec![ResourceSearchSort::Relevance],
        }
    }
    fn namespace(&self) -> String {
        "mcarchive-v1".into()
    }
    fn batch_limit(&self, _: &ProviderQuery) -> usize {
        1
    }
    fn validate_query(&self, query: &ProviderQuery) -> ResourceResult<()> {
        if let ProviderQuery::Search { query, .. } = query {
            validate_archive_search(query)?;
        }
        Ok(())
    }
    async fn fetch_many(
        &self,
        queries: &[ProviderQuery],
    ) -> Vec<ResourceResult<ProviderValue>> {
        let mut results = vec![];
        for query in queries {
            results.push(match self.fetch(query).await {
                Err(ResourceError::Network { status: Some(404) }) => {
                    Ok(ProviderValue::NotFound)
                }
                value => value,
            });
        }
        results
    }
}

#[derive(Deserialize, Serialize)]
struct RemoteProject {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    uuid: String,
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authors: Vec<Value>,
    #[serde(default, alias = "modVersions")]
    mod_versions: Vec<RemoteRelease>,
}

impl RemoteProject {
    fn document(self) -> ResourceResult<ProjectDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        Ok(ProjectDocument {
            protocol,
            metadata: SourceProjectMetadata {
                environments: vec![],
                categories: vec![],
                game_versions: vec![],
                loaders: vec![],
                gallery: vec![],
                project: source_project(SourceId::mcarchive(), self.uuid)?,
                slug: Some(self.slug),
                title: self.name,
                summary: self.description.clone(),
                icon_url: None,
                types: vec![ResourceType::Mod],
                authors: self
                    .authors
                    .into_iter()
                    .filter_map(|author| {
                        author["name"].as_str().map(str::to_owned)
                    })
                    .collect(),
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
                source: SourceId::mcarchive(),
                format: TextFormat::Plain,
                body: self.description,
            },
        })
    }
}

#[derive(Deserialize, Serialize)]
struct RemoteRelease {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    uuid: String,
    name: String,
    #[serde(default, alias = "gameVersions")]
    game_versions: Vec<Value>,
    #[serde(default)]
    files: Vec<RemoteFile>,
}

impl RemoteRelease {
    fn document(
        self,
        project: SourceProjectRef,
    ) -> ResourceResult<ReleaseDocument> {
        let protocol = Some(protocol_projection(serde_json::to_value(&self)?));
        let reference = release_ref(project, self.uuid)?;
        let role = if self.files.len() == 1 {
            ArtifactRole::Primary
        } else {
            ArtifactRole::Unknown
        };
        let artifacts = self
            .files
            .into_iter()
            .map(|file| file.artifact(reference.clone(), role))
            .collect::<ResourceResult<Vec<_>>>()?;
        Ok(ReleaseDocument {
            protocol,
            release: ResourceRelease {
                reference,
                version: self.name,
                channel: ReleaseChannel::Unknown,
                published_at: None,
                compatibility: ReleaseCompatibility {
                    game_versions: self
                        .game_versions
                        .into_iter()
                        .filter_map(|version| {
                            version["name"].as_str().map(str::to_owned)
                        })
                        .collect(),
                    loaders: vec![],
                },
                artifacts,
                equivalent_releases: vec![],
            },
            changelog: None,
        })
    }
}

#[derive(Deserialize, Serialize)]
struct RemoteFile {
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
    uuid: String,
    name: String,
    sha256: Option<String>,
    #[serde(default, alias = "archiveUrl")]
    archive_url: Option<String>,
    #[serde(default, alias = "directUrl")]
    direct_url: Option<String>,
    #[serde(default, alias = "pageUrl")]
    page_url: Option<String>,
    #[serde(default, alias = "redirectUrl")]
    redirect_url: Option<String>,
}

impl RemoteFile {
    fn artifact(
        self,
        release: SourceReleaseRef,
        role: ArtifactRole,
    ) -> ResourceResult<ResourceArtifact> {
        let file_hashes = hashes(
            self.sha256
                .into_iter()
                .filter(|hash| !hash.is_empty())
                .map(|hash| (HashAlgorithm::Sha256, hash)),
        )?;
        let urls = if file_hashes.is_empty() {
            vec![]
        } else {
            self.archive_url
                .filter(|url| !url.trim().is_empty())
                .or(self.direct_url.filter(|url| !url.trim().is_empty()))
                .into_iter()
                .filter(|url| !url.is_empty())
                .collect()
        };
        Ok(ResourceArtifact {
            reference: SourceArtifactRef {
                release,
                artifact_id: SourceArtifactId::new(self.uuid)?,
            },
            file_name: Some(self.name),
            role,
            available: None,
            integrity: ArtifactIntegrity {
                size: None,
                hashes: file_hashes,
            },
            acquisition: acquisition(
                urls,
                self.page_url
                    .filter(|url| !url.is_empty())
                    .or(self.redirect_url),
            ),
            dependencies: vec![],
        })
    }
}

#[derive(Deserialize, Serialize)]
struct RemoteMatchedFile {
    #[serde(flatten)]
    file: RemoteFile,
    mod_version: MatchedRelease,
}

#[derive(Deserialize, Serialize)]
struct MatchedRelease {
    uuid: String,
    #[serde(rename = "mod")]
    mod_: MatchedProject,
}

#[derive(Deserialize, Serialize)]
struct MatchedProject {
    uuid: String,
}

fn collection<T: for<'de> Deserialize<'de>>(
    value: Value,
) -> ResourceResult<Vec<T>> {
    let items = if value.is_array() {
        value
    } else {
        value
            .get("results")
            .or_else(|| value.get("data"))
            .cloned()
            .ok_or_else(|| {
                ResourceError::InvalidResponse(
                    "unexpected MCArchive collection".into(),
                )
            })?
    };
    Ok(serde_json::from_value(items)?)
}

#[cfg(test)]
mod search_filter_tests {
    use super::*;

    fn predicate(
        field: &str,
        values: &[&str],
        excluded: bool,
    ) -> ResourceSearchExpression {
        ResourceSearchExpression::Predicate {
            field: field.into(),
            values: values.iter().map(|value| (*value).into()).collect(),
            excluded,
        }
    }

    #[test]
    fn default_mod_browse_exclusions_do_not_reject_archive_mods() {
        let filter = ResourceSearchExpression::All {
            terms: vec![
                predicate("type", &["mod"], false),
                predicate(
                    "allTypes",
                    &["modpack", "resourcepack", "datapack", "shader"],
                    true,
                ),
                predicate("gameVersion", &["1.7.10"], false),
            ],
        };
        assert!(validate_archive_filter(&filter).is_ok());
    }

    #[test]
    fn unsupported_filters_are_never_silently_broadened() {
        for filter in [
            predicate("allTypes", &["mod"], true),
            predicate("loader", &["forge"], false),
            predicate("gameVersion", &["1.7.10", "1.12.2"], false),
            predicate("environment", &["client_only"], false),
        ] {
            assert!(validate_archive_filter(&filter).is_err());
        }
    }
}
