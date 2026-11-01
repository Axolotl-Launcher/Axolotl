//! Imports a verified schematic while retaining its source-qualified artifact.

use crate::resources::{InstallScope, ResourceArtifact};
use crate::state::instances::adapters::sqlite::{
    content_entries, files, write_guards,
};
use crate::state::instances::{
    ContentOwnershipKind, ContentRequirement, ContentSourceKind,
};
use crate::state::{ProjectType, State};

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
    let mut tx = state.pool.begin().await?;
    write_guards::ensure_content_write_parents(
        instance_id,
        &scope.content_set_id,
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
            relative_path: &relative,
            file_name: &file_name,
            enabled: true,
            sha1: &hash.value,
            size: bytes.len() as u64,
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
                content_set_id: &scope.content_set_id,
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
    // Publishing never overwrites an existing file, including a file created by
    // another application since the user selected the installation target.
    temporary.persist_noclobber(&destination).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            crate::ErrorKind::InputError("schematic_target_exists".into())
                .into()
        } else {
            crate::Error::from(error.error)
        }
    })?;
    if let Err(error) = tx.commit().await {
        let _ = tokio::fs::remove_file(&destination).await;
        return Err(error.into());
    }
    Ok(relative)
}
