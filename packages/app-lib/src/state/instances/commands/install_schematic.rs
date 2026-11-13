//! Imports a verified schematic while retaining its source-qualified artifact.

use crate::resources::{InstallScope, ResourceArtifact};
use crate::state::instances::adapters::sqlite::{
    content_entries, content_sets, files, write_guards,
};
use crate::state::instances::{
    ContentOwnershipKind, ContentRequirement, ContentSourceKind,
};
use crate::state::{ProjectType, State};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Publication {
    pub content_set_id: String,
    pub relative_path: String,
    pub staging_path: String,
    pub artifact: ResourceArtifact,
}

pub(crate) async fn install(
    state: &State,
    instance_id: &str,
    file_name: &str,
    bytes: &[u8],
    artifact: &ResourceArtifact,
) -> crate::Result<String> {
    let file_name = crate::util::io::sanitize_download_file_name(file_name)?;
    let _admission =
        crate::install::runner::interactive_content_guard(state, instance_id)
            .await?;
    let _database = state.acquire_install_db_permit().await?;
    let _content = state.lock_instance_content_exclusive(instance_id).await;
    if crate::state::get_instance(instance_id, &state.pool)
        .await?
        .is_some_and(|metadata| {
            matches!(
                metadata.link,
                crate::state::InstanceLink::ServerProject { .. }
                    | crate::state::InstanceLink::ServerProjectModpack { .. }
            )
        })
    {
        return Err(crate::ErrorKind::InputError(
            "Schematics require a client instance".into(),
        )
        .into());
    }
    let scope = super::resolve_content_scope(instance_id, None, state).await?;
    let root =
        super::instance_content_root(&state.directories, &scope.instance)?;
    super::content_file_operations::recover(state, instance_id, &root).await?;
    let relative = format!("schematics/{file_name}");
    let destination =
        crate::resources::execution::checked_path(&root, &relative)?;
    let parent = destination.parent().ok_or_else(|| {
        crate::ErrorKind::InputError("Invalid schematic target".into())
    })?;
    tokio::fs::create_dir_all(parent).await?;
    let temporary = tempfile::Builder::new()
        .prefix(".axolotl-schematic-")
        .tempfile_in(parent)?;
    tokio::fs::write(temporary.path(), bytes).await?;
    temporary.as_file().sync_all()?;
    let staging_name = temporary
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Invalid schematic staging file".into(),
            )
        })?;
    let publication = Publication {
        content_set_id: scope.content_set_id,
        relative_path: relative.clone(),
        staging_path: format!("schematics/{staging_name}"),
        artifact: artifact.clone(),
    };
    if tokio::fs::try_exists(&destination).await? {
        return Err(crate::ErrorKind::InputError(
            "schematic_target_exists".into(),
        )
        .into());
    }
    super::content_file_operations::prepare_schematic(
        state,
        instance_id,
        &root,
        &publication,
    )
    .await?;
    // The durable journal precedes publication; its source identity survives a
    // process exit before the metadata transaction can commit.
    if let Err(error) = temporary.persist_noclobber(&destination) {
        super::content_file_operations::discard_schematic(state, instance_id)
            .await?;
        return Err(
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                crate::ErrorKind::InputError("schematic_target_exists".into())
                    .into()
            } else {
                crate::Error::from(error.error)
            },
        );
    }
    commit_publication(state, instance_id, &publication).await?;
    super::content_file_operations::finish(state, instance_id, &root).await;
    Ok(relative)
}

/// Commits source metadata and the journal marker in the same transaction.
/// Recovery calls this only for a published file with the journaled integrity.
pub(super) async fn commit_publication(
    state: &State,
    instance_id: &str,
    publication: &Publication,
) -> crate::Result<()> {
    let artifact = &publication.artifact;
    let relative = &publication.relative_path;
    let file_name = std::path::Path::new(relative)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Invalid schematic file name".into())
        })?;
    let size = artifact.integrity.size.ok_or_else(|| {
        crate::ErrorKind::InputError("Missing verified schematic size".into())
    })?;
    let mut tx = state.pool.begin().await?;
    write_guards::ensure_content_write_parents(
        instance_id,
        &publication.content_set_id,
        &mut tx,
    )
    .await?;
    let hash = artifact
        .integrity
        .hashes
        .iter()
        .find(|hash| hash.algorithm == crate::resources::HashAlgorithm::Sha1)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Missing verified schematic hash".into(),
            )
        })?;
    let file = files::upsert_instance_file_from_parts_in_transaction(
        files::UpsertInstanceFile {
            instance_id,
            relative_path: relative,
            file_name,
            enabled: true,
            sha1: &hash.value,
            size,
            missing: false,
            local_mod_data: None,
            icon_path: None,
        },
        &mut tx,
    )
    .await?;
    let entry =
        content_entries::upsert_content_entry_from_parts_in_transaction(
            content_entries::UpsertContentEntry {
                instance_id,
                content_set_id: &publication.content_set_id,
                file_id: Some(&file.id),
                project_type: ProjectType::Schematic,
                source_kind: ContentSourceKind::Local,
                ownership_kind: ContentOwnershipKind::UserAdded,
                auto_dependency: false,
                server_requirement: ContentRequirement::Unknown,
                client_requirement: ContentRequirement::Required,
                enabled: true,
            },
            &mut tx,
        )
        .await?;
    crate::resources::installed::set_installed_artifact(
        &entry.id,
        artifact,
        &InstallScope::Instance,
        &mut tx,
    )
    .await?;
    content_sets::bump_content_set_revision_in_transaction(
        &publication.content_set_id,
        &mut tx,
    )
    .await?;
    super::content_file_operations::mark_committed(instance_id, &mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
