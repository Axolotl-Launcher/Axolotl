use std::path::{Path, PathBuf};

pub(crate) const INSTALLATION: &str = "data/persistence/installation.json";
pub(crate) const RESTORE_INTENT: &str = "data/persistence/storage-restore.json";
pub(crate) const IMPORT_SNAPSHOT: &str = "data/persistence/legacy-app.db";
pub(crate) const INSTANCE_RECOVERY: &str = "data/recovery/instances";
pub(crate) const EXTERNAL_OPERATIONS: &str = "data/recovery/operations";
pub(crate) const IMPORT_RECEIPTS: &str = "data/persistence/imports";
pub(crate) const SETTINGS: &str = "data/settings";
pub(crate) const ACCOUNTS: &str = "data/accounts/accounts.json";
pub(crate) const TELEMETRY: &str = "data/telemetry/telemetry.json";
pub(crate) const MEDIA: &str = "data/media/media.json";
pub(crate) const INSTANCE_GROUPS: &str = "data/groups/instance_groups.json";
pub(crate) const FAVORITE_GROUPS: &str = "data/groups/favorite_groups.json";
pub(crate) const SYNC_ROOT: &str = "data/sync";
pub(crate) const SYNC_AUTHORITY: &str = "data/sync/authority.json";
pub(crate) const SYNC_PENDING: &str = "data/sync/pending.json";
pub(crate) const PLAYTIME: &str = "data/persistence/records.json";
pub(crate) const SNAPSHOT_VERSION: u64 = 2;

pub(crate) fn data(root: &Path) -> PathBuf {
    root.join("data")
}

pub(crate) fn cache(root: &Path) -> PathBuf {
    root.join("cache")
}
