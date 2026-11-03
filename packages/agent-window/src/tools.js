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

export function changeTitle(change) {
    if (change.review?.mode === 'resources')
        return text(
            change.review.intent === 'install' ? 'installResourceCount' : 'updateResourceCount',
            { count: change.review.actions.length },
        )
    if (change.review?.mode === 'modpack')
        return text('installModpack', { name: change.review.pack?.title || change.review.name })
    return change.tool || changeArguments(change)
        ? toolHelp(change.tool || change.meta)
        : change.meta || change.name
}

// Read older saved approvals without exposing their serialized JSON as prose.
export function changeArguments(change) {
    if (change.arguments && typeof change.arguments === 'object') return change.arguments
    if (!change.meta || !change.note?.startsWith('{')) return null
    try {
        const value = JSON.parse(change.note)
        return value && typeof value === 'object' && !Array.isArray(value) ? value : null
    } catch {
        return null
    }
}

export function argumentRows(arguments_, instances = [], technical = false) {
    const rows = []
    function visit(value, path) {
        if (value && typeof value === 'object') {
            const entries = Object.entries(value)
            if (entries.length) {
                for (const [key, child] of entries) {
                    const label = Array.isArray(value) ? String(Number(key) + 1) : fieldLabel(key)
                    visit(child, [...path, label])
                }
                return
            }
        }
        let display =
            value == null
                ? text('argumentUnset')
                : value === true
                  ? text('argumentYes')
                  : value === false
                    ? text('argumentNo')
                    : typeof value === 'object'
                      ? text('argumentEmpty')
                      : String(value)
        const target =
            path.at(-1) === fieldLabel('instance_id') &&
            instances.find((instance) => instance.id === value)
        if (target) display = technical ? `${target.name} (${value})` : target.name
        rows.push({ label: path.join(' › '), value: display })
    }
    for (const [key, value] of Object.entries(arguments_)) {
        if (!technical && ['project_id', 'release_id', 'plan_id', 'revision'].includes(key))
            continue
        visit(value, [fieldLabel(key)])
    }
    return rows
}

function fieldLabel(key) {
    const messageKey = `argument${key
        .split('_')
        .map((part) => part[0]?.toUpperCase() + part.slice(1))
        .join('')}`
    const translated = text(messageKey)
    return translated === messageKey ? key.replaceAll('_', ' ') : translated
}
