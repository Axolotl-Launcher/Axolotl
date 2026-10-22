use super::*;
use crate::mod_metadata::LocalModMetadata;

#[async_trait::async_trait]
pub(crate) trait ArtifactMetadataReader: Send + Sync {
    async fn read(
        &self,
        artifact: &ResourceArtifact,
        cancel: &CancellationToken,
    ) -> ResourceResult<Option<LocalModMetadata>>;
}

pub(crate) struct RuntimeArtifactMetadataReader;

#[async_trait::async_trait]
impl ArtifactMetadataReader for RuntimeArtifactMetadataReader {
    async fn read(
        &self,
        artifact: &ResourceArtifact,
        cancel: &CancellationToken,
    ) -> ResourceResult<Option<LocalModMetadata>> {
        let state = crate::State::get().await?;
        let _lease = crate::storage::usage::lease().await?;
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        if let Some(sha1) = artifact
            .integrity
            .hashes
            .iter()
            .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
            && let Some(size) = artifact.integrity.size
        {
            let cached: Vec<String> = sqlx::query_scalar(
                "SELECT local_mod_data FROM instance_files WHERE missing = 0 AND local_mod_data IS NOT NULL AND sha1 = ? COLLATE NOCASE AND size = ?"
            ).bind(&sha1.value).bind(size as i64).fetch_all(&state.pool).await.map_err(db)?;
            for json in cached {
                if let Ok(metadata) = serde_json::from_str(&json) {
                    return Ok(Some(metadata));
                }
            }
        }
        let ArtifactAcquisition::PublicDownload { urls } =
            &artifact.acquisition
        else {
            return Err(invalid(
                "a local dependency version cannot be verified without the exact download",
            ));
        };
        let Some(url) = urls.first() else {
            return Err(invalid("artifact has no download URL"));
        };
        let base = state.directories.caches_dir().join("resource-staging");
        tokio::fs::create_dir_all(&base)
            .await
            .map_err(|error| invalid(error.to_string()))?;
        let temporary = tempfile::Builder::new()
            .prefix("dependency-metadata-")
            .tempdir_in(base)
            .map_err(|error| invalid(error.to_string()))?;
        let path = temporary.path().join("candidate.jar");
        let class = match artifact.reference.release.project.source.as_str() {
            "modrinth" => crate::util::fetch::ResourceClass::Modrinth,
            "curseforge" => crate::util::fetch::ResourceClass::CurseForge,
            _ => crate::util::fetch::ResourceClass::Other,
        };
        let mut request = crate::util::fetch::DownloadRequest::new(url, class)
            .with_integrity(super::super::execution::integrity(
                artifact,
                ResourceType::Mod,
            ));
        request.candidate_urls = urls.iter().skip(1).cloned().collect();
        request.cancellation = Some(cancel.clone());
        crate::util::fetch::download_to_path(
            request,
            &path,
            &state.download_semaphore,
            &state.pool,
            None,
        )
        .await?;
        let bytes = bytes::Bytes::from(
            tokio::fs::read(path)
                .await
                .map_err(|error| invalid(error.to_string()))?,
        );
        tokio::task::spawn_blocking(move || {
            crate::mod_metadata::extract_mod_metadata(&bytes)
        })
        .await
        .map_err(|error| invalid(error.to_string()))
    }
}

impl ResourceService {
    pub(crate) async fn local_requirements_accept(
        &self,
        release: &ResourceRelease,
        requirements: &[requirements::RetainedRequirement],
        cancel: &CancellationToken,
    ) -> ResourceResult<bool> {
        let local: Vec<_> = requirements
            .iter()
            .filter(|requirement| requirement.local_mod_id.is_some())
            .collect();
        if local.is_empty() {
            return Ok(true);
        }
        let artifacts: Vec<_> = release
            .artifacts
            .iter()
            .filter(|artifact| artifact.role == ArtifactRole::Primary)
            .collect();
        if artifacts.len() != 1 {
            return Ok(false);
        }
        let artifact = artifacts[0];
        let cached = self
            .artifact_metadata
            .get(&artifact.reference)
            .filter(|cached| {
                identity::exact_integrity(&cached.0, &artifact.integrity)
            })
            .map(|cached| cached.1.clone());
        let metadata = if let Some(metadata) = cached {
            metadata
        } else {
            let Some(metadata) =
                self.artifact_metadata_reader.read(artifact, cancel).await?
            else {
                return Ok(false);
            };
            if self.artifact_metadata.len() >= 256 {
                self.artifact_metadata.clear();
            }
            self.artifact_metadata.insert(
                artifact.reference.clone(),
                (artifact.integrity.clone(), metadata.clone()),
            );
            metadata
        };
        Ok(local.iter().all(|requirement| {
            requirement.local_mod_id.as_ref() == Some(&metadata.mod_id)
                && requirement.version_constraint.as_ref().is_none_or(
                    |constraint| {
                        metadata.version.as_ref().is_some_and(|version| {
                            requirements::mod_version_matches(
                                constraint, version,
                            )
                        })
                    },
                )
        }))
    }
}
