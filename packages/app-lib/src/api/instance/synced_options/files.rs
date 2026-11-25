use crate::State;
use crate::state::store::atom;
use crate::state::store::instance_sync_checkpoints::{
    CheckpointKey, CheckpointLinkMode as StoredLinkMode, CheckpointRecord,
    CheckpointStatus as StoredStatus, InstanceSyncCheckpoints,
};
use crate::state::sync_state::collection::Collection as _;
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

impl SyncCheckpoint {
    pub fn matches_projection(&self, sha1: &str, revision: i64) -> bool {
        self.status == CheckpointStatus::Ready
            && self.expected_sha1 == sha1
            && self.source_revision == revision
    }
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
    Copy,
}

impl LinkMode {
    fn stored(self) -> StoredLinkMode {
        match self {
            Self::Symbolic => StoredLinkMode::Symbolic,
            #[cfg(windows)]
            Self::Hard => StoredLinkMode::Hard,
            Self::Copy => StoredLinkMode::Copy,
        }
    }
}

pub(in crate::api::instance) fn safe_instance_id(instance_id: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(instance_id.as_bytes()))
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
    // Writable game files must never alias a shared source generation: both
    // symbolic and hard links let the game change immutable shared bytes.
    let bytes = read_projection_bytes(source).await?;
    publish_detached_copy(target, bytes).await?;
    Ok(LinkMode::Copy)
}

/// A fresh inode replaces the directory entry, so equal bytes cannot preserve
/// an old writable alias into the shared source. Rename publishes in one step.
pub(in crate::api::instance) async fn publish_detached_copy(
    target: &Path,
    bytes: Vec<u8>,
) -> crate::Result<()> {
    crate::state::persistence::document::write_detached_blob(target, bytes)
        .await
}

// Kept as the common publication seam used by the shared-state coordinator.
pub(in crate::api::instance) async fn atomic_native_write(
    path: &Path,
    bytes: &[u8],
) -> crate::Result<()> {
    publish_detached_copy(path, bytes.to_vec()).await
}

pub(super) const MAX_PROJECTION_BYTES: u64 = 16 * 1024 * 1024;

/// Read only a regular projection whose open file has no other hard links.
pub(in crate::api::instance) async fn read_detached_projection(
    path: &Path,
    limit: u64,
) -> crate::Result<Option<Vec<u8>>> {
    let Some(file) = open_detached_projection(path).await? else {
        return Ok(None);
    };
    read_bounded_projection(file, limit).await.map(Some)
}

pub(super) async fn detached_projection_matches(
    path: &Path,
    expected: &[u8],
) -> crate::Result<bool> {
    let Some(file) = open_detached_projection(path).await? else {
        return Ok(false);
    };
    let size = expected.len() as u64;
    if file.metadata().await?.len() != size {
        return Ok(false);
    }
    Ok(read_bounded_projection(file, size).await? == expected)
}

async fn open_detached_projection(
    path: &Path,
) -> crate::Result<Option<tokio::fs::File>> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    options.custom_flags(
        windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT.0,
    );
    let file = match options.open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        #[cfg(unix)]
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    if !is_detached(&file).await? {
        return Ok(None);
    }
    Ok(Some(file))
}

#[cfg(unix)]
async fn is_detached(file: &tokio::fs::File) -> crate::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().await?;
    Ok(metadata.is_file() && metadata.nlink() == 1)
}

#[cfg(windows)]
async fn is_detached(file: &tokio::fs::File) -> crate::Result<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
        GetFileInformationByHandle,
    };
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let verified = unsafe {
        GetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            &raw mut information,
        )
        .is_ok()
    };
    Ok(verified
        && information.nNumberOfLinks == 1
        && information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0
        && file.metadata().await?.is_file())
}

#[cfg(not(any(windows, unix)))]
async fn is_detached(_file: &tokio::fs::File) -> crate::Result<bool> {
    Ok(false)
}

async fn read_projection_bytes(path: &Path) -> crate::Result<Vec<u8>> {
    read_bounded_projection(
        tokio::fs::File::open(path).await?,
        MAX_PROJECTION_BYTES,
    )
    .await
}

async fn read_bounded_projection(
    file: tokio::fs::File,
    limit: u64,
) -> crate::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    if file.metadata().await?.len() > limit {
        return Err(crate::ErrorKind::InputError(
            "Sync projection exceeds size limit".into(),
        )
        .into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).await?;
    if bytes.len() as u64 > limit {
        return Err(crate::ErrorKind::InputError(
            "Sync projection grew beyond size limit".into(),
        )
        .into());
    }
    Ok(bytes)
}

pub(in crate::api::instance) async fn detach_link(
    _source: &Path,
    target: &Path,
) -> crate::Result<()> {
    // Missing local data is an intentional deletion, never a request to restore
    // the canonical source while disabling synchronization.
    let contents = match read_projection_bytes(target).await {
        Ok(bytes) => bytes,
        Err(error) if matches!(error.raw.as_ref(),crate::ErrorKind::StdIOError(error) if error.kind()==std::io::ErrorKind::NotFound) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    publish_detached_copy(target, contents).await
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::api::instance) struct CheckpointProof {
    pub expected_sha1: String,
    pub source_revision: i64,
}
impl CheckpointProof {
    pub fn new(expected_sha1: &str, source_revision: i64) -> Self {
        Self {
            expected_sha1: expected_sha1.to_owned(),
            source_revision,
        }
    }
    fn matches(&self, record: &CheckpointRecord) -> bool {
        record.status == StoredStatus::Pending
            && record.expected_sha1 == self.expected_sha1
            && record.source_revision == self.source_revision
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
) -> crate::Result<CheckpointProof> {
    let key = checkpoint_key(instance_id, option, variant);
    let record = CheckpointRecord {
        expected_sha1: expected_sha1.to_string(),
        merge_base: merge_base.map(|bytes| bytes.to_vec()),
        source_revision,
        status: StoredStatus::Pending,
        link_mode: None,
    };
    InstanceSyncCheckpoints::set(&state.data, &key, Some(&record)).await?;
    Ok(CheckpointProof::new(expected_sha1, source_revision))
}

pub(in crate::api::instance) async fn finish_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    mode: LinkMode,
    proof: &CheckpointProof,
    state: &State,
) -> crate::Result<()> {
    let key = checkpoint_key(instance_id, option, variant);
    let mut unit = atom(&state.data).await?;
    if let Some(mut record) =
        InstanceSyncCheckpoints::atom_get(&mut unit, &key).await?
    {
        if !proof.matches(&record) {
            return Ok(());
        }
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
    proof: &CheckpointProof,
    state: &State,
) -> crate::Result<()> {
    let key = checkpoint_key(instance_id, option, variant);
    let mut unit = atom(&state.data).await?;
    if let Some(mut record) =
        InstanceSyncCheckpoints::atom_get(&mut unit, &key).await?
    {
        if !proof.matches(&record) {
            return Ok(());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_state_ids_preserve_exact_identity_without_separator_collisions()
     {
        assert_ne!(safe_instance_id("a/b"), safe_instance_id("a_b"));
        assert_ne!(safe_instance_id("a:b"), safe_instance_id("a\\b"));
        assert_eq!(safe_instance_id("instance").len(), 64);
    }
    #[test]
    fn acknowledgement_requires_the_pending_revision_and_hash() {
        let pending = CheckpointRecord {
            expected_sha1: "one".into(),
            merge_base: None,
            source_revision: 2,
            status: StoredStatus::Pending,
            link_mode: None,
        };
        assert!(CheckpointProof::new("one", 2).matches(&pending));
        assert!(!CheckpointProof::new("one", 1).matches(&pending));
        assert!(!CheckpointProof::new("two", 2).matches(&pending));
        let mut completed = pending;
        completed.status = StoredStatus::Ready;
        assert!(!CheckpointProof::new("one", 2).matches(&completed));
    }
    #[tokio::test]
    async fn writable_projection_does_not_alias_shared_bytes_and_republish_keeps_old_targets()
     {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let first = temporary.path().join("first");
        let second = temporary.path().join("second");
        tokio::fs::write(&source, b"first generation")
            .await
            .unwrap();
        ensure_link(&source, &first).await.unwrap();
        ensure_link(&source, &second).await.unwrap();
        tokio::fs::write(&first, b"game changed").await.unwrap();
        assert_eq!(
            tokio::fs::read(&source).await.unwrap(),
            b"first generation"
        );
        crate::state::persistence::document::write_blob(
            &source,
            b"second generation".to_vec(),
        )
        .await
        .unwrap();
        assert_eq!(
            tokio::fs::read(&second).await.unwrap(),
            b"first generation"
        );
        ensure_link(&source, &first).await.unwrap();
        assert_eq!(
            tokio::fs::read(&first).await.unwrap(),
            b"second generation"
        );
    }
    #[tokio::test]
    async fn equal_bytes_still_detach_old_hardlink_targets() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let target = temporary.path().join("target");
        tokio::fs::write(&source, b"same").await.unwrap();
        tokio::fs::hard_link(&source, &target).await.unwrap();
        ensure_link(&source, &target).await.unwrap();
        tokio::fs::write(&target, b"edited").await.unwrap();
        assert_eq!(tokio::fs::read(&source).await.unwrap(), b"same");
    }
    #[tokio::test]
    async fn disabling_sync_does_not_restore_deleted_local_file() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let target = temporary.path().join("deleted");
        tokio::fs::write(&source, b"canonical").await.unwrap();
        detach_link(&source, &target).await.unwrap();
        assert!(!target.exists());
    }
}
