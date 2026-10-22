import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'

import { get_full_path, get_mod_full_path } from '@/helpers/instance'

export async function isDev() {
    return await invoke('is_dev')
}

export async function isElevated() {
    return await invoke('is_elevated')
}

export async function areUpdatesEnabled() {
    return await invoke('are_updates_enabled')
}

export async function checkAppUpdate(channel = 'release') {
    return await invoke('check_app_update', { channel })
}

export async function getUpdateSize(updateRid) {
    return await invoke('get_update_size', { rid: updateRid })
}

export async function enqueueUpdateForInstallation(updateRid) {
    return await invoke('enqueue_update_for_installation', { rid: updateRid })
}

export async function backupAppDbForUpdate(version) {
    return await invoke('backup_app_db_for_update', { version })
}

export async function removeEnqueuedUpdate() {
    return await invoke('remove_enqueued_update')
}

export async function setRestartAfterPendingUpdate(should_restart) {
    return await invoke('set_restart_after_pending_update', { shouldRestart: should_restart })
}

// One of 'Windows', 'Linux', 'MacOS'
export async function getOS() {
    return await invoke('plugin:utils|get_os')
}

export async function isNetworkMetered() {
    return await invoke('plugin:utils|is_network_metered')
}

export async function openPath(path) {
    return await invoke('plugin:utils|open_path', { path })
}

export async function highlightInFolder(path) {
    return await invoke('plugin:utils|highlight_in_folder', { path })
}

export async function showLauncherLogsFolder() {
    return await invoke('plugin:utils|show_launcher_logs_folder', {})
}

export async function exportErrorLogs(
    errorMessage,
    fileNamePrefix = 'Axolotl Launcher error logs',
) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
    const outputPath = await save({
        defaultPath: `${fileNamePrefix} ${timestamp}.zip`,
        filters: [
            {
                name: 'ZIP archive',
                extensions: ['zip'],
            },
        ],
    })

    if (!outputPath) return

    return await invoke('plugin:utils|export_error_logs', { outputPath, errorMessage })
}

export async function exportLauncherLogs({
    range = 'last2Hours',
    level = 'all',
    includeSystemInfo = true,
    includeInstanceLogs = false,
    includeCrashAnalysis = false,
    includeNetworkConnections = false,
    fileNamePrefix = 'Axolotl Launcher logs',
} = {}) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
    const outputPath = await save({
        defaultPath: `${fileNamePrefix} ${timestamp}.zip`,
        filters: [
            {
                name: 'ZIP archive',
                extensions: ['zip'],
            },
        ],
    })

    if (!outputPath) return null

    await invoke('plugin:utils|export_launcher_logs', {
        outputPath,
        range,
        level,
        includeSystemInfo,
        includeInstanceLogs,
        includeCrashAnalysis,
        includeNetworkConnections,
    })
    return outputPath
}

export async function createInstanceShortcut(instanceName, instanceId, options = {}) {
    const outputPath = await save({
        defaultPath: `Axolotl - ${instanceName}`,
    })

    if (!outputPath) return null

    return await invoke('plugin:shortcuts|create_instance_shortcut', {
        instanceName,
        instanceId,
        outputPath,
        server: options.server,
        singleplayerWorld: options.singleplayerWorld,
    })
}

export async function createInstanceShortcutOnDesktop(instanceName, instanceId) {
    return await invoke('plugin:shortcuts|create_instance_shortcut', {
        instanceName,
        instanceId,
        outputPath: null,
        server: null,
        singleplayerWorld: null,
    })
}

export async function showAppDbBackupsFolder() {
    return await invoke('plugin:utils|show_app_db_backups_folder', {})
}

// Opens an instance's folder in the OS file explorer
export async function showInstanceInFolder(instanceId) {
    const fullPath = await get_full_path(instanceId)
    return await openPath(fullPath)
}

export async function highlightModInInstance(instanceId, projectPath) {
    const fullPath = await get_mod_full_path(instanceId, projectPath)
    return await highlightInFolder(fullPath)
}

export async function restartApp() {
    return await invoke('restart_app')
}

export const releaseColor = (releaseType) => {
    switch (releaseType) {
        case 'release':
            return 'green'
        case 'beta':
            return 'orange'
        case 'alpha':
            return 'red'
        default:
            return ''
    }
}

export async function copyToClipboard(text) {
    await navigator.clipboard.writeText(text)
}
