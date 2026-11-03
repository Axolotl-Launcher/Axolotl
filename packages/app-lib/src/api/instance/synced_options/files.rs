use crate::State;
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::instance_sync_checkpoints::{
    CheckpointKey, CheckpointLinkMode as StoredLinkMode, CheckpointRecord,
    CheckpointStatus as StoredStatus, InstanceSyncCheckpoints,
};
use crate::state::{InstanceInstallStage, InstanceMetadata, SyncedOption};
use crate::util::io;
use quartz_nbt::NbtCompound;
use sha1_smol::Sha1;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub(in crate::api::instance) struct SyncCheckpoint {
    pub expected_sha1: String,
    pub merge_base: Option<Vec<u8>>,
    pub source_revision: i64,
    pub status: CheckpointStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::api::instance) enum CheckpointStatus {
    Pending,
    Ready,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::api::instance) enum LinkMode {
    Symbolic,
    #[cfg(windows)]
    Hard,
    #[cfg(windows)]
    Copy,
}

impl LinkMode {
    fn stored(self) -> StoredLinkMode {
        match self {
            Self::Symbolic => StoredLinkMode::Symbolic,
            #[cfg(windows)]
            Self::Hard => StoredLinkMode::Hard,
            #[cfg(windows)]
            Self::Copy => StoredLinkMode::Copy,
        }
    }
}

pub(in crate::api::instance) fn safe_instance_id(instance_id: &str) -> String {
    instance_id.replace([':', '/', '\\'], "_")
}

pub(crate) fn instance_game_dir(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<PathBuf> {
    crate::state::instances::commands::instance_content_root(
        &state.directories,
        &metadata.instance,
    )
}

pub(in crate::api::instance) fn sync_files_are_protected(
    metadata: &InstanceMetadata,
) -> bool {
    matches!(
        metadata.instance.install_stage,
        InstanceInstallStage::MinecraftInstalling
            | InstanceInstallStage::PackInstalling
    )
}

pub(in crate::api::instance) async fn instance_is_running(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<bool> {
    crate::state::instance_has_running_process(&metadata.instance.id, state)
        .await
}

pub(in crate::api::instance) fn option_can_apply_while_running(
    option: SyncedOption,
) -> bool {
    matches!(
        option,
        SyncedOption::GameOptions
            | SyncedOption::Screenshots
            | SyncedOption::ResourcePacks
    )
}

pub(in crate::api::instance) fn instance_option_enabled(
    metadata: &InstanceMetadata,
    option: SyncedOption,
) -> bool {
    match option {
        SyncedOption::GameOptions => metadata.synced_options.game_options,
        SyncedOption::CommandHistory => metadata.synced_options.command_history,
        SyncedOption::MultiplayerServers => {
            metadata.synced_options.multiplayer_servers
        }
        SyncedOption::CreativeHotbars => {
            metadata.synced_options.creative_hotbars
        }
        SyncedOption::Screenshots => metadata.synced_options.screenshots,
        SyncedOption::ResourcePacks => metadata.synced_options.resource_packs,
        SyncedOption::DataPacks => metadata.synced_options.data_packs,
    }
}

pub(in crate::api::instance) async fn ensure_link(
    source: &Path,
    target: &Path,
) -> crate::Result<LinkMode> {
    if let Some(parent) = target.parent() {
        io::create_dir_all(parent).await?;
    }
    if tokio::fs::symlink_metadata(target)
        .await
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
        && tokio::fs::read_link(target)
            .await
            .is_ok_and(|current| current == source)
    {
        return Ok(LinkMode::Symbolic);
    }
    if tokio::fs::symlink_metadata(target).await.is_ok() {
        io::remove_file(target).await?;
    }

    #[cfg(unix)]
    {
        tokio::fs::symlink(source, target).await?;
        Ok(LinkMode::Symbolic)
    }
    #[cfg(windows)]
    {
        if tokio::fs::symlink_file(source, target).await.is_ok() {
            return Ok(LinkMode::Symbolic);
        }
        if tokio::fs::hard_link(source, target).await.is_ok() {
            return Ok(LinkMode::Hard);
        }
        io::copy(source, target).await?;
        Ok(LinkMode::Copy)
    }
}

pub(in crate::api::instance) async fn detach_link(
    source: &Path,
    target: &Path,
) -> crate::Result<()> {
    let target_metadata = tokio::fs::symlink_metadata(target).await.ok();
    let contents = if target_metadata.is_some() && target.exists() {
        Some(io::read(target).await?)
    } else if source.exists() {
        Some(io::read(source).await?)
    } else {
        None
    };
    if target_metadata.is_some() {
        io::remove_file(target).await?;
    }
    if let Some(contents) = contents {
        io::write(target, contents).await?;
    }
    Ok(())
}

fn checkpoint_key(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
) -> CheckpointKey {
    CheckpointKey {
        instance_id: instance_id.to_string(),
        feature: option.as_str().to_string(),
        variant: variant.to_string(),
    }
}

pub(in crate::api::instance) async fn begin_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    expected_sha1: &str,
    merge_base: Option<&[u8]>,
    source_revision: i64,
    state: &State,
) -> crate::Result<()> {
    let key = checkpoint_key(instance_id, option, variant);
    let record = CheckpointRecord {
        expected_sha1: expected_sha1.to_string(),
        merge_base: merge_base.map(|bytes| bytes.to_vec()),
        source_revision,
        status: StoredStatus::Pending,
        link_mode: None,
    };
    InstanceSyncCheckpoints::set(&state.data, &key, Some(&record)).await
}

pub(in crate::api::instance) async fn finish_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    mode: LinkMode,
    state: &State,
) -> crate::Result<()> {
    let key = checkpoint_key(instance_id, option, variant);
    let mut unit = atom(&state.data).await?;
    if let Some(mut record) =
        InstanceSyncCheckpoints::atom_get(&mut unit, &key).await?
    {
        record.status = StoredStatus::Ready;
        record.link_mode = Some(mode.stored());
        InstanceSyncCheckpoints::atom_set(&mut unit, &key, Some(&record))
            .await?;
    }
    unit.commit().await
}

pub(in crate::api::instance) async fn finish_plain_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    state: &State,
) -> crate::Result<()> {
    let key = checkpoint_key(instance_id, option, variant);
    let mut unit = atom(&state.data).await?;
    if let Some(mut record) =
        InstanceSyncCheckpoints::atom_get(&mut unit, &key).await?
    {
        record.status = StoredStatus::Ready;
        record.link_mode = None;
        InstanceSyncCheckpoints::atom_set(&mut unit, &key, Some(&record))
            .await?;
    }
    unit.commit().await
}

pub(in crate::api::instance) async fn checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    state: &State,
) -> crate::Result<Option<SyncCheckpoint>> {
    let key = checkpoint_key(instance_id, option, variant);
    let Some(record) = InstanceSyncCheckpoints::get(&state.data, &key).await?
    else {
        return Ok(None);
    };
    Ok(Some(SyncCheckpoint {
        expected_sha1: record.expected_sha1,
        merge_base: record.merge_base,
        source_revision: record.source_revision,
        status: match record.status {
            StoredStatus::Pending => CheckpointStatus::Pending,
            StoredStatus::Ready => CheckpointStatus::Ready,
        },
    }))
}

pub(in crate::api::instance) async fn sha1_file(
    path: &Path,
) -> crate::Result<String> {
    Ok(Sha1::from(io::read(path).await?).digest().to_string())
}

pub(in crate::api::instance) fn sha1_bytes(bytes: &[u8]) -> String {
    Sha1::from(bytes).digest().to_string()
}

pub(in crate::api::instance) async fn read_nbt_file(
    path: &Path,
) -> crate::Result<NbtCompound> {
    let bytes = io::read(path).await?;
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(bytes),
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    Ok(root)
}

pub(in crate::api::instance) fn nbt_to_bytes(
    root: &NbtCompound,
) -> crate::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    quartz_nbt::io::write_nbt(
        &mut bytes,
        None,
        root,
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    Ok(bytes)
}

pub(in crate::api::instance) fn nbt_from_bytes(
    bytes: Vec<u8>,
) -> crate::Result<NbtCompound> {
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(bytes),
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    Ok(root)
}
