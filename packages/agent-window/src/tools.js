// Plain words for the launcher tools that steps name, for people who have never
// watched an agent work. The step keeps the tool's real name, which is what the
// agent and a bug report would quote; this says what the tool does. Each
// sentence follows the tool's description in packages/launcher-tools.
import { text } from './messages.js'

// A Map, so a tool name such as `constructor` cannot reach Object's prototype.
const TOOLS = new Map([
    ['instance_list', 'toolHelpInstanceList'],
    ['instance_get', 'toolHelpInstanceGet'],
    ['instance_create', 'toolHelpInstanceCreate'],
    ['world_list', 'toolHelpWorldList'],
    ['world_get', 'toolHelpWorldGet'],
    ['world_backup', 'toolHelpWorldBackup'],
    ['resource_search', 'toolHelpResourceSearch'],
    ['resource_list_releases', 'toolHelpResourceListReleases'],
    ['resource_prepare_install', 'toolHelpResourcePrepareInstall'],
    ['resource_execute_plan', 'toolHelpResourceExecutePlan'],
    ['resource_check_updates', 'toolHelpResourceCheckUpdates'],
    ['resource_prepare_update', 'toolHelpResourcePrepareUpdate'],
    ['resource_execute_changes', 'toolHelpResourceExecuteChanges'],
    ['resource_history_list', 'toolHelpResourceHistoryList'],
    ['resource_history_rollback', 'toolHelpResourceHistoryRollback'],
    ['content_list', 'toolHelpContentList'],
    ['logs_list', 'toolHelpLogsList'],
    ['logs_read', 'toolHelpLogsRead'],
    ['backup_get_config', 'toolHelpBackupGetConfig'],
    ['backup_create', 'toolHelpBackupCreate'],
    ['launch_set_memory', 'toolHelpLaunchSetMemory'],
    ['install_job_get', 'toolHelpInstallJobGet'],
    ['operations_get', 'toolHelpOperationsGet'],
    ['process_list', 'toolHelpProcessList'],
    ['system_get_memory', 'toolHelpSystemGetMemory'],
    ['metadata_list_loader_versions', 'toolHelpMetadataListLoaderVersions'],
])

// A tool without a sentence of its own is described by its family, the first
// prefix that matches. Families that do the same thing for the user share a
// sentence: install jobs and operations are both background work that the
// agent follows, skins go with capes and the proxy with the network.
const FAMILIES = [
    ['instance_', 'toolFamilyInstance'],
    ['world_', 'toolFamilyWorld'],
    ['resource_', 'toolFamilyResource'],
    ['content_', 'toolFamilyContent'],
    ['backup_', 'toolFamilyBackup'],
    ['logs_', 'toolFamilyLogs'],
    ['launch_', 'toolFamilyLaunch'],
    ['install_job_', 'toolFamilyJob'],
    ['operations_', 'toolFamilyJob'],
    ['process_', 'toolFamilyProcess'],
    ['system_', 'toolFamilySystem'],
    ['metadata_', 'toolFamilyMetadata'],
    ['get_launcher_', 'toolFamilyLauncher'],
    ['launcher_', 'toolFamilyLauncher'],
    ['account_', 'toolFamilyAccount'],
    ['skin_', 'toolFamilySkin'],
    ['cape_', 'toolFamilySkin'],
    ['java_', 'toolFamilyJava'],
    ['core_', 'toolFamilyCore'],
    ['modpack_', 'toolFamilyModpack'],
    ['download_', 'toolFamilyDownload'],
    ['file_', 'toolFamilyFile'],
    ['archive_', 'toolFamilyArchive'],
    ['nbt_', 'toolFamilyNbt'],
    ['screenshot_', 'toolFamilyScreenshot'],
    ['schematic_', 'toolFamilySchematic'],
    ['seed_', 'toolFamilySeed'],
    // An instance's saved server list shares `server_` with the servers the
    // launcher runs itself, so it has to match first.
    ['server_entry_', 'toolFamilyServerList'],
    ['server_ping', 'toolFamilyServerList'],
    ['server_', 'toolFamilyServer'],
    ['multiplayer_', 'toolFamilyMultiplayer'],
    ['settings_', 'toolFamilySettings'],
    ['privacy_', 'toolFamilyPrivacy'],
    ['proxy_', 'toolFamilyNetwork'],
    ['network_', 'toolFamilyNetwork'],
    ['storage_', 'toolFamilyStorage'],
    ['database_', 'toolFamilyDatabase'],
    ['shortcut_', 'toolFamilyShortcut'],
    ['news_', 'toolFamilyNews'],
    ['ui_', 'toolFamilyInterface'],
]

// The family's message key, or null; exported so a test can hold every tool in
// the registry to a family.
export function toolFamily(tool) {
    return FAMILIES.find(([prefix]) => tool.startsWith(prefix))?.[1] ?? null
}

export function toolHelp(tool) {
    return text(TOOLS.get(tool) || toolFamily(tool) || 'toolGeneric')
}
