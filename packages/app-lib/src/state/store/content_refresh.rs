//! Conditional writes for asynchronous content refresh work.

use super::content_refresh_checks::{
    ContentRefreshChecks, RefreshCheckContext, RefreshCheckKey,
    RefreshCheckRecord, location_key,
};
use super::content_refresh_presentations::{
    ContentRefreshPresentations, PresentationKey, PresentationRecord,
};
use super::{Collection as _, Handle, atom};
use crate::resources::*;

use crate::state::instances::adapters::sqlite::content_sets;

#[derive(Clone)]
pub(crate) struct RefreshWrite {
    pub entry_id: String,
    pub file_id: String,
    pub context: RefreshCheckContext,
    pub check: Option<RefreshCheckRecord>,
    pub presentation: Option<PresentationRecord>,
    pub artifact: Option<crate::resources::installed::InstalledArtifactRecord>,
}

/// Returns only writes that still name the installed bytes and current target.
pub(crate) async fn set_results(
    handle: &Handle,
    instance_id: &str,
    writes: Vec<RefreshWrite>,
) -> crate::Result<Vec<RefreshWrite>> {
    let mut unit = atom(handle).await?;
    let instance = match super::content_snapshot::get_instance(
        unit.connection(),
        instance_id,
    )
    .await
    {
        Ok(instance) => instance,
        Err(error)
            if matches!(
                error.raw.as_ref(),
                crate::ErrorKind::InputError(_)
            ) =>
        {
            return Ok(vec![]);
        }
        Err(error) => return Err(error),
    };
    let Some(content_set) =
        content_sets::get_applied_content_set(instance_id, unit.connection())
            .await?
    else {
        return Ok(vec![]);
    };
    let preferred: String = sqlx::query_scalar(
        "SELECT preferred_source FROM resource_preferences WHERE id = 1",
    )
    .fetch_one(unit.connection())
    .await?;
    let target = ipc::ResourceTarget {
        game_versions: vec![content_set.game_version],
        loaders: vec![content_set.loader.as_str().into()],
        channels: crate::resources::plans::update_channels(
            instance.update_channel,
        ),
    };
    let location = location_key(&instance);
    let mut applied = Vec::new();
    for mut write in writes {
        if write.context.location_key != location
            || write.context.target != target
            || write.context.preferred_source.as_str() != preferred
        {
            continue;
        }
        let row: Option<(String, i64, Option<String>, String)> = sqlx::query_as(
            "SELECT file.sha1, file.size, exact.artifact, file.relative_path FROM instance_content_entries entry
             JOIN instance_files file ON file.id = entry.file_id LEFT JOIN resource_installed_artifacts exact ON exact.entry_id = entry.id
             WHERE entry.id = ? AND entry.instance_id = ? AND entry.content_set_id = ? AND file.id = ? AND file.missing = 0"
        ).bind(&write.entry_id).bind(instance_id).bind(&content_set.id).bind(&write.file_id).fetch_optional(unit.connection()).await?;
        let Some((sha1, size, previous, path)) = row else {
            continue;
        };
        if !sha1.eq_ignore_ascii_case(&write.context.sha1)
            || u64::try_from(size).ok() != Some(write.context.size)
        {
            continue;
        }
        let previous = previous.and_then(|payload| {
            serde_json::from_str::<
                crate::resources::installed::InstalledArtifactRecord,
            >(&payload)
            .ok()
        });
        let release = previous
            .as_ref()
            .filter(|record| {
                record.artifact.integrity.hashes.iter().any(|hash| {
                    hash.algorithm == HashAlgorithm::Sha1
                        && hash.value.eq_ignore_ascii_case(&sha1)
                }) && record.artifact.integrity.size == Some(write.context.size)
            })
            .map(|record| &record.artifact.reference.release);
        if release != write.context.current_release.as_ref() {
            continue;
        }
        if let Some(artifact) = &write.artifact {
            if !crate::resources::identity::exact_integrity(
                &ArtifactIntegrity {
                    size: Some(write.context.size),
                    hashes: vec![ContentHash {
                        algorithm: HashAlgorithm::Sha1,
                        value: sha1.clone(),
                    }],
                },
                &artifact.artifact.integrity,
            ) {
                continue;
            }
            let scope =
                if path.starts_with("saves/") && path.contains("/datapacks/") {
                    InstallScope::World {
                        relative_path: path
                            .split('/')
                            .take(2)
                            .collect::<Vec<_>>()
                            .join("/"),
                    }
                } else {
                    InstallScope::Instance
                };
            installed::set_installed_artifact(
                &write.entry_id,
                artifact,
                &scope,
                unit.connection(),
            )
            .await?;
        }
        if let Some(check) = &write.check {
            ContentRefreshChecks::atom_set(
                &mut unit,
                &RefreshCheckKey {
                    instance_id: instance_id.into(),
                    entry_id: write.entry_id.clone(),
                },
                Some(check),
            )
            .await?;
        }
        if let Some(presentation) = &mut write.presentation {
            let key = PresentationKey {
                instance_id: instance_id.into(),
                file_id: write.file_id.clone(),
            };
            if let Some(previous) =
                ContentRefreshPresentations::atom_get(&mut unit, &key).await?
                && previous.location_key == presentation.location_key
                && previous.content.id == presentation.content.id
                && previous.content.size == presentation.content.size
            {
                presentation.content.project = presentation
                    .content
                    .project
                    .take()
                    .or(previous.content.project);
                presentation.content.version = presentation
                    .content
                    .version
                    .take()
                    .or(previous.content.version);
                presentation.content.owner = presentation
                    .content
                    .owner
                    .take()
                    .or(previous.content.owner);
                presentation.content.loader = presentation
                    .content
                    .loader
                    .take()
                    .or(previous.content.loader);
                presentation.content.environment = presentation
                    .content
                    .environment
                    .take()
                    .or(previous.content.environment);
                presentation.content.external &= previous.content.external;
            }
            ContentRefreshPresentations::atom_set(
                &mut unit,
                &PresentationKey {
                    instance_id: instance_id.into(),
                    file_id: write.file_id.clone(),
                },
                Some(presentation),
            )
            .await?;
        }
        applied.push(write);
    }
    unit.commit().await?;
    Ok(applied)
}
