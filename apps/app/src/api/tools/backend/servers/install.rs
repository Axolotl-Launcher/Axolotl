use super::{error, info, require_stopped, summary, validate_java};
use launcher_tools::{Command, ToolError};
use serde_json::Value;
use theseus::{resources::*, servers};
use tokio_util::sync::CancellationToken;

fn loader_target(
    info: &servers::ServerInfo,
    loader: &str,
) -> Result<(), ToolError> {
    require_stopped(info)?;
    if info.manifest.server_type != loader {
        return Err(ToolError::new(
            "server_loader_mismatch",
            "Installer must match this server's configured type",
        ));
    }
    Ok(())
}

async fn install_loader(
    server_id: String,
    build: String,
    registered_java_path: Option<String>,
    loader: &str,
) -> Result<Value, ToolError> {
    let current = info(&server_id).await?;
    loader_target(&current, loader)?;
    // Both an explicit override and a saved path must refer to a registered
    // runtime; absent paths retain the native installer's PATH-based default.
    let java = registered_java_path.or(current.manifest.java_path.clone());
    validate_java(java.as_deref()).await?;
    let key = if loader == "neoforge" { "neo" } else { "forge" };
    let manifest = theseus::metadata::get_loader_versions_for_game(
        key,
        &current.manifest.game_version,
    )
    .await
    .map_err(error)?;
    if !super::super::metadata::selected_loaders(
        &manifest,
        &current.manifest.game_version,
    )
    .iter()
    .any(|version| version.id == build)
    {
        return Err(ToolError::new(
            "loader_build_unavailable",
            "Choose an exact build returned by metadata_list_loader_versions for this server's Minecraft version",
        ));
    }
    if loader == "neoforge" {
        servers::install_neoforge(
            &server_id,
            &current.manifest.game_version,
            &build,
            java,
        )
        .await
        .map_err(error)?;
    } else {
        servers::install_forge(
            &server_id,
            &current.manifest.game_version,
            &build,
            java,
        )
        .await
        .map_err(error)?;
    }
    Ok(summary(info(&server_id).await?))
}

struct PackDownload {
    url: String,
    sha1: String,
}

fn pack_download(
    release: &ResourceRelease,
    artifact_id: Option<&str>,
    game_version: &str,
) -> Result<PackDownload, ToolError> {
    let target = ipc::ResourceTarget {
        game_versions: vec![game_version.into()],
        loaders: vec!["neoforge".into()],
        channels: vec![],
    };
    let compatibility =
        assess_compatibility(release, &target, ResourceType::ModPack);
    if compatibility.game_version != CompatibilityState::Compatible
        || compatibility.loader != CompatibilityState::Compatible
    {
        return Err(ToolError::new(
            "server_pack_incompatible",
            "This release must declare compatibility with the server's Minecraft version and NeoForge",
        ));
    }
    let candidates: Vec<_> = release
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.reference.release == release.reference
                && artifact.available != Some(false)
                && artifact.file_name.as_ref().is_some_and(|name| {
                    name.to_ascii_lowercase().ends_with(".mrpack")
                })
                && artifact_id.is_none_or(|id| {
                    artifact.reference.artifact_id.as_str() == id
                })
        })
        .collect();
    let selected = if candidates.len() == 1 {
        candidates[0]
    } else {
        let primary: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|artifact| artifact.role == ArtifactRole::Primary)
            .collect();
        if artifact_id.is_none() && primary.len() == 1 {
            primary[0]
        } else {
            return Err(ToolError::new(
                "server_pack_artifact_required",
                "Select one available .mrpack artifact_id from resource_get_release",
            ));
        }
    };
    let sha1=selected.integrity.hashes.iter().find(|hash|hash.algorithm==HashAlgorithm::Sha1).map(|hash|hash.value.clone())
        .filter(|hash|hash.len()==40&&hash.bytes().all(|c|c.is_ascii_hexdigit()))
        .ok_or_else(||ToolError::new("server_pack_hash_required","The selected archive needs a valid SHA-1 checksum from its provider"))?;
    let ArtifactAcquisition::PublicDownload { urls } = &selected.acquisition
    else {
        return Err(ToolError::new(
            "server_pack_manual_download",
            "Selected archive is not available for automatic download",
        ));
    };
    let url = urls
        .iter()
        .find(|value| {
            url::Url::parse(value).is_ok_and(|url| {
                url.scheme() == "https"
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            })
        })
        .ok_or_else(|| {
            ToolError::new(
                "server_pack_download_unavailable",
                "Selected archive has no usable provider download",
            )
        })?;
    Ok(PackDownload {
        url: url.clone(),
        sha1,
    })
}

async fn install_modpack(
    server_id: String,
    project_id: String,
    release_id: String,
    artifact_id: Option<String>,
) -> Result<Value, ToolError> {
    let current = info(&server_id).await?;
    loader_target(&current, "neoforge")?;
    validate_java(current.manifest.java_path.as_deref()).await?;
    let project = SourceProjectRef {
        source: SourceId::modrinth(),
        project_id: SourceProjectId::new(project_id.clone()).map_err(|_| {
            ToolError::new("invalid_project_id", "Invalid Modrinth project ID")
        })?,
    };
    let reference = SourceReleaseRef {
        project: project.clone(),
        release_id: SourceReleaseId::new(release_id.clone()).map_err(|_| {
            ToolError::new("invalid_release_id", "Invalid release ID")
        })?,
    };
    let state = theseus::State::get().await.map_err(error)?;
    let resources = state.resources().await.map_err(|_| {
        ToolError::new(
            "resource_unavailable",
            "Resource provider is unavailable",
        )
    })?;
    let cancel = CancellationToken::new();
    let metadata = resources
        .get(
            &ResourceRef::Source {
                project: project.clone(),
            },
            None,
            None,
            &cancel,
        )
        .await
        .map_err(|_| {
            ToolError::new(
                "server_pack_project_unavailable",
                "Unable to fetch source project metadata",
            )
        })?;
    if !metadata.projects.iter().any(|p| {
        p.project == project && p.types.contains(&ResourceType::ModPack)
    }) {
        return Err(ToolError::new(
            "not_a_modpack",
            "Selected source project must be a Modrinth modpack",
        ));
    }
    let release =
        resources
            .release(&reference, None, &cancel)
            .await
            .map_err(|_| {
                ToolError::new(
                    "server_pack_release_unavailable",
                    "Unable to resolve the exact Modrinth project/release",
                )
            })?;
    let pack = pack_download(
        &release,
        artifact_id.as_deref(),
        &current.manifest.game_version,
    )?;
    // NeoForge's existing server installer derives its loader build from the
    // verified pack index and validates index paths/hashes before downloads.
    servers::install_modpack(
        &server_id,
        &pack.url,
        Some(pack.sha1),
        None,
        None,
        None,
        Some(project_id),
        Some(release_id),
        Some(current.manifest.name),
        None,
    )
    .await
    .map_err(error)?;
    Ok(summary(info(&server_id).await?))
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        Command::ServerInstallForge {
            server_id,
            build,
            registered_java_path,
        } => {
            install_loader(server_id, build, registered_java_path, "forge")
                .await
        }
        Command::ServerInstallNeoforge {
            server_id,
            build,
            registered_java_path,
        } => {
            install_loader(server_id, build, registered_java_path, "neoforge")
                .await
        }
        Command::ServerInstallModpack {
            server_id,
            project_id,
            release_id,
            artifact_id,
        } => {
            install_modpack(server_id, project_id, release_id, artifact_id)
                .await
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a server installation operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn release() -> ResourceRelease {
        serde_json::from_value(json!({"reference":{"project":{"source":"modrinth","projectId":"pack"},"releaseId":"release"},"version":"1.0","channel":"release","publishedAt":null,
            "compatibility":{"gameVersions":["1.21.1"],"loaders":["neoforge"]},"equivalentReleases":[],"artifacts":[{
                "reference":{"release":{"project":{"source":"modrinth","projectId":"pack"},"releaseId":"release"},"artifactId":"archive"},
                "fileName":"pack.mrpack","role":"primary","available":true,"integrity":{"size":123,"hashes":[{"algorithm":"sha1","value":"a".repeat(40)}]},
                "acquisition":{"kind":"publicDownload","urls":["https://cdn.modrinth.com/pack.mrpack"]},"dependencies":[]
            }]})).unwrap()
    }
    #[test]
    fn exact_mrpack_requires_declared_compatibility_and_hash() {
        let mut release = release();
        assert!(pack_download(&release, None, "1.21.1").is_ok());
        assert_eq!(
            pack_download(&release, None, "1.20.1").err().unwrap().code,
            "server_pack_incompatible"
        );
        release.compatibility.loaders.clear();
        assert!(pack_download(&release, None, "1.21.1").is_err());
        release.compatibility.loaders = vec!["neoforge".into()];
        release.artifacts[0].integrity.hashes.clear();
        assert_eq!(
            pack_download(&release, None, "1.21.1").err().unwrap().code,
            "server_pack_hash_required"
        );
    }
    #[test]
    fn ambiguous_manual_foreign_and_non_https_artifacts_are_rejected() {
        let mut release = release();
        let mut second = release.artifacts[0].clone();
        second.reference.artifact_id = SourceArtifactId::new("second").unwrap();
        release.artifacts.push(second);
        assert_eq!(
            pack_download(&release, None, "1.21.1").err().unwrap().code,
            "server_pack_artifact_required"
        );
        assert!(pack_download(&release, Some("archive"), "1.21.1").is_ok());
        release.artifacts.truncate(1);
        release.artifacts[0].acquisition =
            ArtifactAcquisition::PublicDownload {
                urls: vec!["file:///private".into()],
            };
        assert_eq!(
            pack_download(&release, None, "1.21.1").err().unwrap().code,
            "server_pack_download_unavailable"
        );
        release.artifacts[0].acquisition =
            ArtifactAcquisition::ManualDownload {
                page_url: "https://example.com".into(),
            };
        assert_eq!(
            pack_download(&release, None, "1.21.1").err().unwrap().code,
            "server_pack_manual_download"
        );
        release.artifacts[0].reference.release.release_id =
            SourceReleaseId::new("foreign").unwrap();
        assert_eq!(
            pack_download(&release, None, "1.21.1").err().unwrap().code,
            "server_pack_artifact_required"
        );
    }
    #[test]
    fn installers_require_matching_stopped_server_types() {
        let manifest:servers::ServerManifest=serde_json::from_value(json!({"id":"server-123","name":"Server","serverType":"forge","gameVersion":"1.20.1","createdAt":"2026-01-01T00:00:00Z"})).unwrap();
        let mut current = servers::ServerInfo {
            manifest,
            path: "unused".into(),
            running: false,
            starting: false,
            stopping: false,
            eula_exists: false,
            eula_accepted: false,
            port: None,
            online_mode: None,
        };
        assert!(loader_target(&current, "forge").is_ok());
        assert_eq!(
            loader_target(&current, "neoforge").unwrap_err().code,
            "server_loader_mismatch"
        );
        current.starting = true;
        assert_eq!(
            loader_target(&current, "forge").unwrap_err().code,
            "server_active"
        );
    }
}
