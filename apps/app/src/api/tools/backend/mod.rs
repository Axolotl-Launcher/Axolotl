mod accounts;
mod backups;
mod core;
mod downloads;
mod files;
mod gallery;
mod imports;
mod instance_maintenance;
mod instances;
mod interface;
mod java;
mod jobs;
mod launch;
mod launcher_system;
mod lifecycle;
mod logs;
mod maintenance;
mod metadata;
mod modpacks;
mod multiplayer;
mod preferences;
mod queries;
mod resource_history;
mod resource_updates;
mod resources;
mod reviewed;
mod schematics;
mod seed_tools;
mod servers;
mod skins;
mod upgrades;
mod worlds;

use futures::future::BoxFuture;
use launcher_tools::{
    Backend, BackendContext, Command, ScopedRegistry, ToolError,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct LauncherBackend {
    app: Option<tauri::AppHandle>,
    plans: ScopedRegistry<theseus::resources::ResourceOperationPlan>,
    restores: ScopedRegistry<backups::RestorePreview>,
    deletions: ScopedRegistry<instance_maintenance::DeletePreview>,
    resource_cleanups:
        ScopedRegistry<theseus::resources::ResourceHistoryCleanupPlan>,
    modpacks: ScopedRegistry<modpacks::PackPlan>,
    import_sources: ScopedRegistry<imports::ImportSource>,
    import_plans: ScopedRegistry<imports::ImportSource>,
    upgrades: ScopedRegistry<upgrades::UpgradePlan>,
    schematics: ScopedRegistry<schematics::Document>,
    metadata_cleanups: ScopedRegistry<String>,
    updates: ScopedRegistry<launcher_system::UpdateHandle>,
}

impl LauncherBackend {
    pub(super) fn new(app: tauri::AppHandle) -> Self {
        Self {
            app: Some(app),
            ..Default::default()
        }
    }
}

impl Backend for LauncherBackend {
    fn execute(
        &self,
        command: Command,
        context: BackendContext,
    ) -> BoxFuture<'_, Result<Value, ToolError>> {
        Box::pin(async move {
            match command {
                command @ (Command::SchematicListFiles { .. }
                | Command::SchematicOpen { .. }
                | Command::SchematicGet { .. }
                | Command::SchematicGetBlock { .. }
                | Command::SchematicReadRegion { .. }
                | Command::SchematicApplyEdits { .. }
                | Command::SchematicTransform { .. }
                | Command::SchematicExport { .. }
                | Command::SchematicClose { .. }) => {
                    schematics::execute(self, command, context).await
                }
                command @ (Command::SeedGetProfiles { .. }
                | Command::SeedReadWorld { .. }
                | Command::SeedGetSpawn { .. }
                | Command::SeedGetBiome { .. }
                | Command::SeedFindFeatures { .. }
                | Command::SeedRenderTile { .. }
                | Command::SeedGetStructureDetails { .. }
                | Command::SeedGetContainerLoot { .. }
                | Command::SeedScanOres { .. }
                | Command::SeedRenderUnderground { .. }) => {
                    seed_tools::execute(self, command, context).await
                }
                command @ (Command::SettingsGet { .. }
                | Command::SettingsGetSchema { .. }
                | Command::SettingsPatch { .. }
                | Command::PrivacyGet { .. }
                | Command::PrivacySet { .. }
                | Command::ProxyGet { .. }
                | Command::ProxySet { .. }
                | Command::ProxyTest { .. }
                | Command::NetworkTest { .. }
                | Command::NetworkGetEnvironment { .. }) => {
                    preferences::execute(self, command, context).await
                }
                command @ (Command::StorageScan { .. }
                | Command::StorageClearCache { .. }
                | Command::StorageAnalyzeMetadata { .. }
                | Command::StorageCleanMetadata { .. }
                | Command::DatabaseGetInfo { .. }
                | Command::DatabaseCheckIntegrity { .. }
                | Command::DatabaseBackup { .. }
                | Command::DatabaseListBackups { .. }
                | Command::DatabaseMaintainBackups { .. }
                | Command::DatabaseCopyChannel { .. }) => {
                    maintenance::execute(self, command, context).await
                }
                command @ (Command::LauncherGetUpdateSettings { .. }
                | Command::LauncherSetUpdateChannel { .. }
                | Command::LauncherSetUpdatePreferences { .. }
                | Command::LauncherCheckUpdate { .. }
                | Command::LauncherGetUpdateSize { .. }
                | Command::LauncherDownloadUpdate { .. }
                | Command::LauncherCancelUpdate { .. }
                | Command::LauncherRestart { .. }
                | Command::LauncherExit { .. }
                | Command::ShortcutCreateInstance { .. }
                | Command::SystemOpenPath { .. }
                | Command::SystemOpenUrl { .. }
                | Command::LauncherParseLink { .. }
                | Command::NewsGetMinecraft { .. }) => {
                    launcher_system::execute(self, command, context).await
                }
                command @ (Command::CoreListComponents { .. }
                | Command::CorePreviewJar { .. }
                | Command::CoreAddJarMod { .. }
                | Command::CoreReplaceJar { .. }
                | Command::CoreSetEnabled { .. }
                | Command::CoreMoveComponent { .. }
                | Command::CoreRemoveComponent { .. }
                | Command::CoreRestoreComponent { .. }
                | Command::CoreInstallMcarchiveLoader { .. }
                | Command::CoreImportMcarchiveLoader { .. }) => {
                    core::execute(self, command, context).await
                }
                command @ (Command::MultiplayerGetState { .. }
                | Command::MultiplayerGetConnection { .. }
                | Command::MultiplayerListNodes { .. }
                | Command::MultiplayerDetectPorts { .. }
                | Command::MultiplayerSetProvider { .. }
                | Command::MultiplayerPrepareProvider { .. }
                | Command::MultiplayerHost { .. }
                | Command::MultiplayerJoin { .. }
                | Command::MultiplayerStop { .. }
                | Command::MultiplayerReset { .. }
                | Command::MultiplayerCheckUpdate { .. }
                | Command::MultiplayerUpdate { .. }
                | Command::MultiplayerLogsRead { .. }
                | Command::MultiplayerGetDiagnostics { .. }) => {
                    multiplayer::execute(self, command, context).await
                }
                command @ (Command::ScreenshotList { .. }
                | Command::ScreenshotGet { .. }
                | Command::ScreenshotSaveEdit { .. }
                | Command::ScreenshotDelete { .. }
                | Command::ScreenshotMove { .. }
                | Command::ScreenshotExport { .. }
                | Command::ScreenshotGroupList { .. }
                | Command::ScreenshotGroupCreate { .. }
                | Command::ScreenshotGroupRename { .. }
                | Command::ScreenshotGroupDelete { .. }
                | Command::ScreenshotSetGroup { .. }) => {
                    gallery::execute(self, command, context).await
                }
                command @ (Command::SkinList { .. }
                | Command::CapeList { .. }
                | Command::SkinGetTexture { .. }
                | Command::SkinImport { .. }
                | Command::SkinSave { .. }
                | Command::SkinEquip { .. }
                | Command::SkinDelete { .. }
                | Command::SkinUnequip { .. }
                | Command::SkinSetOrder { .. }
                | Command::CapeEquip { .. }
                | Command::CapeUnequip { .. }) => {
                    skins::execute(self, command, context).await
                }
                command @ (Command::FileList { .. }
                | Command::FileStat { .. }
                | Command::FileSearch { .. }
                | Command::FileReadText { .. }
                | Command::FileReadBinary { .. }
                | Command::FileWrite { .. }
                | Command::FileApplyPatch { .. }
                | Command::FileCreateDirectory { .. }
                | Command::FileCopy { .. }
                | Command::FileMove { .. }
                | Command::FileTrash { .. }
                | Command::ArchiveCreate { .. }
                | Command::ArchiveExtract { .. }
                | Command::NbtRead { .. }
                | Command::NbtPatch { .. }) => {
                    files::execute(self, command, context).await
                }
                command @ (Command::UiGetContext {}
                | Command::UiScreenshot {}
                | Command::UiNavigate { .. }
                | Command::UiClick { .. }
                | Command::UiType { .. }
                | Command::UiKey { .. }
                | Command::UiScroll { .. }) => {
                    interface::execute(self, command, context).await
                }
                command @ (Command::InstanceScanImports { .. }
                | Command::InstancePrepareImport { .. }
                | Command::InstanceExecuteImport { .. }) => {
                    imports::execute(self, command, context).await
                }
                command @ (Command::InstancePlanUpgrade { .. }
                | Command::InstanceGetUpgradePlan { .. }
                | Command::InstanceReviseUpgrade { .. }
                | Command::InstanceSelectUpgradeSolution {
                    ..
                }
                | Command::InstanceExecuteUpgrade { .. }) => {
                    upgrades::execute(self, command, context).await
                }
                command @ (Command::ModpackPrepareInstall { .. }
                | Command::ModpackPrepareLocal { .. }
                | Command::ModpackExecuteInstall { .. }) => {
                    modpacks::execute(self, command, context).await
                }
                command @ (Command::ResourceCheckUpdates { .. }
                | Command::ResourcePrepareUpdate { .. }
                | Command::ResourcePrepareSwitch { .. }) => {
                    resource_updates::execute(self, command, context).await
                }
                command @ (Command::ResourceFavoritesList { .. }
                | Command::ResourceFavoriteAdd { .. }
                | Command::ResourceFavoriteRemove { .. }
                | Command::ResourceHistoryList { .. }
                | Command::ResourceHistoryRollback { .. }
                | Command::ResourceScanBackups { .. }
                | Command::ResourcePrepareHistoryCleanup {
                    ..
                }
                | Command::ResourceExecuteHistoryCleanup {
                    ..
                }) => resource_history::execute(self, command, context).await,
                command @ (Command::InstanceDuplicate { .. }
                | Command::InstanceReinstall { .. }
                | Command::InstanceCheckInstallation { .. }
                | Command::InstanceDeletePreview { .. }
                | Command::InstanceDelete { .. }) => {
                    instance_maintenance::execute(self, command, context).await
                }
                command @ (Command::ServerList { .. }
                | Command::ServerInstallForge { .. }
                | Command::ServerInstallNeoforge { .. }
                | Command::ServerInstallModpack { .. }
                | Command::ServerGet { .. }
                | Command::ServerCreate { .. }
                | Command::ServerUpdateSettings { .. }
                | Command::ServerStart { .. }
                | Command::ServerWaitReady { .. }
                | Command::ServerStop { .. }
                | Command::ServerKill { .. }
                | Command::ServerDelete { .. }
                | Command::ServerConsoleSend { .. }
                | Command::ServerLogsRead { .. }
                | Command::ServerLogsSearch { .. }) => {
                    servers::execute(command).await
                }
                command @ (Command::AccountList { .. }
                | Command::AccountGet { .. }
                | Command::AccountGetDefault { .. }
                | Command::AccountSetDefault { .. }
                | Command::AccountRemove { .. }
                | Command::AccountAddOffline { .. }
                | Command::AccountGetEligibility {}) => {
                    accounts::execute(command).await
                }
                command @ (Command::DownloadJobList { .. }
                | Command::DownloadJobGet { .. }
                | Command::DownloadJobGetItems { .. }
                | Command::DownloadJobRetry { .. }
                | Command::DownloadJobResume { .. }
                | Command::DownloadJobCancel { .. }
                | Command::DownloadJobDelete { .. }
                | Command::DownloadHistoryClear {}
                | Command::InstallJobMissingFiles { .. }
                | Command::InstallJobScanMissingFiles { .. }
                | Command::InstallJobSkipMissing { .. }
                | Command::InstallJobRepairAndRetry { .. }) => {
                    downloads::execute(command).await
                }
                command @ (Command::JavaListVendors {}
                | Command::JavaListVersions { .. }
                | Command::JavaInstall { .. }
                | Command::JavaTest { .. }
                | Command::JavaSetDefault { .. }
                | Command::JavaRemoveDefault { .. }
                | Command::JavaRemoveRegistration { .. }) => {
                    java::execute(command).await
                }
                command @ (Command::WorldList { .. }
                | Command::WorldGet { .. }
                | Command::WorldRename { .. }
                | Command::WorldSetDisplayStatus { .. }
                | Command::WorldUpdateSettings { .. }
                | Command::WorldBackup { .. }
                | Command::WorldDelete { .. }
                | Command::InstanceLaunchWorld { .. }
                | Command::InstanceJoinServer { .. }
                | Command::ServerEntryList { .. }
                | Command::ServerEntryAdd { .. }
                | Command::ServerEntryUpdate { .. }
                | Command::ServerEntryRemove { .. }
                | Command::ServerPing { .. }
                | Command::InstanceGetProtocolVersion { .. }) => {
                    worlds::execute(command).await
                }
                Command::MetadataListGameVersions {
                    release_type,
                    search,
                    offset,
                    limit,
                } => metadata::games(release_type, search, offset, limit).await,
                Command::MetadataListLoaderVersions {
                    loader,
                    game_version,
                    stable_only,
                    offset,
                    limit,
                } => {
                    metadata::loaders(
                        loader,
                        game_version,
                        stable_only,
                        offset,
                        limit,
                    )
                    .await
                }
                command @ (Command::LaunchConfigGet { .. }
                | Command::LaunchSetMemory { .. }
                | Command::LaunchSetWindowOptions { .. }
                | Command::LaunchSetJava { .. }
                | Command::LaunchResetOverrides { .. }
                | Command::LaunchGetMemoryStatus { .. }) => {
                    launch::execute(command).await
                }
                command @ (Command::BackupRepositoryStatus {}
                | Command::BackupGetConfig { .. }
                | Command::BackupEnable { .. }
                | Command::BackupDisable { .. }
                | Command::BackupSetExclusions { .. }
                | Command::BackupCreate { .. }
                | Command::BackupListSnapshots { .. }
                | Command::BackupListOperations { .. }
                | Command::BackupGetOperation { .. }
                | Command::BackupCancel { .. }
                | Command::BackupPreviewRestore { .. }
                | Command::BackupGetRestorePreview { .. }
                | Command::BackupRestore { .. }
                | Command::BackupDeleteSnapshot { .. }) => {
                    backups::execute(self, command, context).await
                }
                command @ (Command::ResourceListSources {}
                | Command::ResourceSearch { .. }
                | Command::ResourceGet { .. }
                | Command::ResourceListReleases { .. }
                | Command::ResourceGetRelease { .. }
                | Command::ResourcePrepareInstall { .. }
                | Command::ResourceExecutePlan { .. }
                | Command::ResourceExecuteChanges { .. }
                | Command::ResourceRevisePlan { .. }
                | Command::ContentRemove { .. }
                | Command::ContentList { .. }
                | Command::ContentSetEnabled { .. }) => {
                    resources::execute(self, command, context).await
                }
                Command::InstanceLaunch {
                    instance_id,
                    offline_mode,
                } => lifecycle::launch(instance_id, offline_mode).await,
                Command::ProcessGet { process_id } => {
                    lifecycle::get(process_id).await
                }
                Command::ProcessWait {
                    process_id,
                    timeout_seconds,
                } => lifecycle::wait(process_id, timeout_seconds).await,
                Command::ProcessKill { process_id } => {
                    lifecycle::kill(process_id).await
                }
                Command::InstallJobRetry { job_id } => Ok(jobs::summary(
                    theseus::install::retry_job(job_id)
                        .await
                        .map_err(business_error)?,
                )),
                Command::InstallJobResume { job_id } => Ok(jobs::summary(
                    theseus::install::resume_job(job_id)
                        .await
                        .map_err(business_error)?,
                )),
                Command::InstallJobCancel { job_id } => Ok(jobs::summary(
                    theseus::install::cancel_job(job_id)
                        .await
                        .map_err(business_error)?,
                )),
                Command::LogsList {
                    instance_id,
                    offset,
                    limit,
                } => logs::list(instance_id, offset, limit).await,
                Command::LogsRead {
                    instance_id,
                    log_type,
                    filename,
                    offset,
                    max_bytes,
                } => {
                    logs::read(
                        instance_id,
                        log_type,
                        filename,
                        offset,
                        max_bytes,
                    )
                    .await
                }
                Command::LogsSearch {
                    instance_id,
                    log_type,
                    filename,
                    query,
                    start_line,
                    limit,
                } => {
                    logs::search(
                        instance_id,
                        log_type,
                        filename,
                        query,
                        start_line,
                        limit,
                    )
                    .await
                }
                Command::InstallJobList {
                    include_finished,
                    offset,
                    limit,
                } => jobs::list(include_finished, offset, limit).await,
                Command::InstallJobGet { job_id } => jobs::get(job_id).await,
                Command::ProcessList { instance_id } => {
                    queries::processes(instance_id).await
                }
                Command::JavaList {} => queries::java().await,
                Command::SystemGetMemory {} => queries::memory().await,
                command => instances::execute(command).await,
            }
        })
    }
}

fn business_error(error: theseus::Error) -> ToolError {
    tracing::warn!(%error, "MCP launcher operation failed");
    ToolError::new("operation_failed", error.user_facing_message())
}

fn serialize(value: impl Serialize) -> Result<Value, ToolError> {
    serde_json::to_value(value).map_err(|_| {
        ToolError::new(
            "serialization_failed",
            "Unable to encode launcher result",
        )
    })
}

fn page<T: Serialize>(
    items: Vec<T>,
    offset: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let total = items.len();
    let items: Vec<_> = items.into_iter().skip(offset).take(limit).collect();
    let end = offset.saturating_add(items.len());
    Ok(
        json!({"items":items,"total":total,"next_offset": if end < total { Some(end) } else { None }}),
    )
}
