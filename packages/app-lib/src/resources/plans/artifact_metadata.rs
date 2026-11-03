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
            let cached = crate::state::instances::adapters::sqlite::files::get_local_mod_data_for_artifact(
                &sha1.value,
                size as i64,
                &state.pool,
            )
            .await
            .map_err(|error| ResourceError::Database(error.to_string()))?;
            for json in cached {
                if let Ok(metadata) = serde_json::from_str(&json) {
                    return Ok(Some(metadata));
                }
            }
        }
        let ArtifactAcquisition::PublicDownload { urls } =
            &artifact.acquisition
        else {
            return Ok(None);
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
        Ok(self
            .local_requirements_assessment(release, requirements, cancel)
            .await?
            != CompatibilityState::Incompatible)
    }

    pub(crate) async fn local_requirements_assessment(
        &self,
        release: &ResourceRelease,
        requirements: &[requirements::RetainedRequirement],
        cancel: &CancellationToken,
    ) -> ResourceResult<CompatibilityState> {
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let local: Vec<_> = requirements
            .iter()
            .filter(|requirement| requirement.local_mod_id.is_some())
            .collect();
        if local.is_empty() {
            return Ok(CompatibilityState::Compatible);
        }
        let artifacts: Vec<_> = release
            .artifacts
            .iter()
            .filter(|artifact| artifact.role == ArtifactRole::Primary)
            .collect();
        if artifacts.len() != 1 {
            return Ok(CompatibilityState::Incompatible);
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
            let metadata = match self
                .artifact_metadata_reader
                .read(artifact, cancel)
                .await
            {
                Ok(Some(metadata)) => metadata,
                Ok(None)
                | Err(
                    ResourceError::Network { .. } | ResourceError::Unavailable,
                ) => {
                    return Ok(CompatibilityState::Unknown);
                }
                Err(error) => return Err(error),
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
        let mut assessment = CompatibilityState::Compatible;
        for requirement in local {
            if requirement.local_mod_id.as_ref() != Some(&metadata.mod_id) {
                return Ok(CompatibilityState::Incompatible);
            }
            if let Some(constraint) = &requirement.version_constraint {
                match metadata
                    .version
                    .as_ref()
                    .map_or(CompatibilityState::Unknown, |version| {
                        assess_release_constraint(constraint, version)
                    }) {
                    CompatibilityState::Incompatible => {
                        return Ok(CompatibilityState::Incompatible);
                    }
                    CompatibilityState::Unknown => {
                        assessment = CompatibilityState::Unknown
                    }
                    CompatibilityState::Compatible => {}
                }
            }
        }
        Ok(assessment)
    }
}
