import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/instance.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/*
 * Strings of the instance page, grouped by the desktop app file that owns them.
 * Ids and English text are the app's own (apps/app-frontend/src/locales/en-US),
 * so the zh-CN overlay carries the app's real translations.
 */

/** `apps/app-frontend/src/pages/instance/Index.vue` */
export const instancePageMessages = defineMessages({
    upgradedTo: {
        id: 'app.instance.post-upgrade-status',
        defaultMessage: 'Upgraded to {version}',
    },
    linkedTo: { id: 'app.instance.linked-to', defaultMessage: 'Linked to' },
    joinServer: { id: 'app.instance.join-server', defaultMessage: 'Join server' },
    launchInstance: { id: 'app.instance.launch-instance', defaultMessage: 'Launch instance' },
    upgradeInstance: { id: 'app.instance.upgrade-instance', defaultMessage: 'Upgrade instance' },
    neverPlayed: {
        id: 'app.instance.never-played',
        defaultMessage: 'Never played',
    },
    stopping: {
        id: 'app.instance.stopping',
        defaultMessage: 'Stopping...',
    },
    starting: {
        id: 'app.instance.starting',
        defaultMessage: 'Starting... {seconds, number}s',
    },
    instanceSettings: {
        id: 'app.instance.settings',
        defaultMessage: 'Instance settings',
    },
    exportModpack: {
        id: 'app.instance.export-modpack',
        defaultMessage: 'Export modpack',
    },
    createShortcut: {
        id: 'app.instance.create-shortcut',
        defaultMessage: 'Create shortcut',
    },
    addContent: {
        id: 'app.instances.add-content',
        defaultMessage: 'Add content',
    },
    copyPath: {
        id: 'app.instances.copy-path',
        defaultMessage: 'Copy path',
    },
    contentTab: {
        id: 'app.instance.tabs.content',
        defaultMessage: 'Content',
    },
    filesTab: {
        id: 'app.instance.tabs.files',
        defaultMessage: 'Files',
    },
    worldsTab: {
        id: 'app.instance.tabs.worlds',
        defaultMessage: 'Worlds',
    },
    screenshotsTab: {
        id: 'app.instance.tabs.screenshots',
        defaultMessage: 'Screenshots',
    },
    logsTab: {
        id: 'app.instance.tabs.logs',
        defaultMessage: 'Logs',
    },
})

/** `apps/app-frontend/src/pages/instance/Mods.vue` */
export const modsMessages = defineMessages({
    postUpgradeNoticeTitle: {
        id: 'app.instance.mods.post-upgrade-notice.title',
        defaultMessage: 'Post-upgrade compatibility review',
    },
    postUpgradeNoticeBody: {
        id: 'app.instance.mods.post-upgrade-notice.body',
        defaultMessage:
            'Some preserved content may not support the current Minecraft version and should be reviewed manually.',
    },
    ignoreAllPostUpgradeWarnings: {
        id: 'app.instance.mods.post-upgrade-notice.ignore-all',
        defaultMessage: 'Ignore all',
    },
    postUpgradeWarningTooltip: {
        id: 'app.instance.mods.post-upgrade-notice.item-tooltip',
        defaultMessage:
            'This content was preserved during the instance upgrade without installing a new compatible version. It may be incompatible with the current Minecraft version.',
    },
    compatibilityWarning: {
        id: 'app.resources.installed.compatibility',
        defaultMessage:
            'Minecraft: {game, select, compatible {compatible} incompatible {incompatible} other {no supported version declaration}}; loader: {loader, select, compatible {compatible} incompatible {incompatible} other {no loader declaration}}.',
    },
    contentRefreshWarningTitle: {
        id: 'app.instance.mods.content-refresh-warning.title',
        defaultMessage: 'Some online details are unavailable',
    },
    contentRefreshWarningBody: {
        id: 'app.instance.mods.content-refresh-warning.body',
        defaultMessage:
            'Local content is still complete and can be managed. Names, icons, or update information may be temporarily unavailable.',
    },
    contentHistoryWarningTitle: {
        id: 'app.instance.mods.content-history-warning.title',
        defaultMessage: 'Some local history details are unavailable',
    },
    contentHistoryWarningBody: {
        id: 'app.instance.mods.content-history-warning.body',
        defaultMessage:
            'Local content is still complete and can be managed. Rollback or cleanup details may be temporarily unavailable.',
    },
    versionsFailed: {
        id: 'app.instance.content.versions-failed',
        defaultMessage:
            'The release list could not be loaded. Reopen the version selector and try again.',
    },
    manageBackups: {
        id: 'app.instance.mods.manage-backups',
        defaultMessage: 'Old files and backups',
    },
    shareTitle: {
        id: 'app.instance.mods.share-title',
        defaultMessage: 'Sharing modpack content',
    },
    shareText: {
        id: 'app.instance.mods.share-text',
        defaultMessage: "Check out the projects I'm using in my modpack!",
    },
    successfullyUploaded: {
        id: 'app.instance.mods.successfully-uploaded',
        defaultMessage: 'Successfully uploaded',
    },
    projectWasAdded: {
        id: 'app.instance.mods.project-was-added',
        defaultMessage: '"{name}" was added',
    },
    projectsWereAdded: {
        id: 'app.instance.mods.projects-were-added',
        defaultMessage: '{count} projects were added',
    },
    contentTypeProject: {
        id: 'app.instance.mods.content-type-project',
        defaultMessage: 'project',
    },
    updateAddedContent: {
        id: 'app.instance.mods.update-added-content',
        defaultMessage: 'Update added content',
    },
    updateAddedContentDescription: {
        id: 'app.instance.mods.update-added-content.description',
        defaultMessage:
            'Updates only content added after the modpack was installed. Modpack files are not changed.',
    },
    parseFailed: {
        id: 'app.instance.mods.parse-failed',
        defaultMessage: 'Failed to parse. This may not be a modpack',
    },
    fileAlreadyExists: {
        id: 'app.instance.mods.file-already-exists',
        defaultMessage: 'Failed to add. The file may already exist',
    },
    restorePackDefault: {
        id: 'app.instance.mods.restore-pack-default',
        defaultMessage: 'Restore modpack default',
    },
    packMemberMissing: {
        id: 'app.instance.mods.pack-member-missing',
        defaultMessage: 'This modpack file is missing locally',
    },
    packMemberRemoved: {
        id: 'app.instance.mods.pack-member-removed',
        defaultMessage: 'This modpack file was removed locally',
    },
    openInMcmod: {
        id: 'app.project.open-in-mcmod',
        defaultMessage: 'Open in MC Mod',
    },
})

/** `apps/app-frontend/src/components/ui/export/messages.ts` */
export const exportMessages = defineMessages({
    finalizingAnalysis: {
        id: 'app.export.finalizing-analysis',
        defaultMessage: 'Preparing verification results…',
    },
    queued: {
        id: 'app.export.queued',
        defaultMessage: 'Export queued',
    },
    queuedDescription: {
        id: 'app.export.queued-description',
        defaultMessage: '{name} will finish verification and export in Downloads.',
    },
    verifyingLocal: {
        id: 'app.export.verify-local',
        defaultMessage: 'Verifying exportable local files…',
    },
    verifyingSource: {
        id: 'app.export.verify-source',
        defaultMessage: 'Verifying exact matches on {source}…',
    },
    checkNeedsAttention: {
        id: 'app.export.check-attention',
        defaultMessage:
            'Some files need attention. Review them before export, or let the export task retry verification.',
    },
    failed: {
        id: 'app.export.failed',
        defaultMessage: 'Export failed. Retry analysis before exporting again.',
    },
    canceled: {
        id: 'app.export.canceled',
        defaultMessage: 'Export canceled. The previous output was preserved.',
    },
    author: {
        id: 'app.export.author',
        defaultMessage: 'Author (optional)',
    },
    format: {
        id: 'app.export.format',
        defaultMessage: 'Export format',
    },
    modrinth: {
        id: 'app.export.format.modrinth',
        defaultMessage: 'Modrinth standard (.mrpack)',
    },
    genericZip: {
        id: 'app.export.format.genericZip',
        defaultMessage: 'Generic ZIP (.zip)',
    },
    curseForge: {
        id: 'app.export.format.curseForge',
        defaultMessage: 'CurseForge ZIP (.zip)',
    },
    formatHint: {
        id: 'app.export.format-hint',
        defaultMessage:
            'Modrinth references matched MR files; generic ZIP uses verified public links; CurseForge references matched CF files. Other selected files are embedded.',
    },
    writing: {
        id: 'app.export.writing',
        defaultMessage: 'Writing the export archive…',
    },
    cancelExport: {
        id: 'app.export.cancel',
        defaultMessage: 'Cancel export',
    },
    analyzing: {
        id: 'app.export.analyzing',
        defaultMessage: 'Verifying exportable files and refreshing exact source matches…',
    },
    analyzed: {
        id: 'app.export.analyzed',
        defaultMessage:
            'Background verification finished. Unmatched selected files will be embedded.',
    },
    retryAnalysis: {
        id: 'app.export.retry-analysis',
        defaultMessage: 'Retry analysis',
    },
    failedAnalysis: {
        id: 'app.export.failed-analysis',
        defaultMessage:
            'Some files could not be resolved. Retry, or explicitly embed the files with failed source requests.',
    },
    localFailure: {
        id: 'app.export.local-failure',
        defaultMessage:
            'This local file could not be read. Retry or remove it from the file selection.',
    },
    embedFailure: {
        id: 'app.export.embed-failure',
        defaultMessage: 'Embed this file and continue despite the failed source request',
    },
    categories: {
        id: 'app.export.categories',
        defaultMessage: 'Choose categories',
    },
    files: {
        id: 'app.export.files',
        defaultMessage: 'Adjust files',
    },
    package: {
        id: 'app.export.package',
        defaultMessage: 'Package information',
    },
    next: {
        id: 'app.export.next',
        defaultMessage: 'Next',
    },
    scan: {
        id: 'app.export.scan',
        defaultMessage: 'Scanning local files…',
    },
    scanFailed: {
        id: 'app.export.scan-failed',
        defaultMessage: 'Could not scan the instance files',
    },
    retry: {
        id: 'app.export.retry',
        defaultMessage: 'Retry scan',
    },
    requirements: {
        id: 'app.export.requirements',
        defaultMessage: 'Game and loader requirements',
    },
    requirementsDescription: {
        id: 'app.export.requirements-description',
        defaultMessage: 'Included in the manifest. Game files and runtime libraries are excluded.',
    },
    categoryHint: {
        id: 'app.export.category-hint',
        defaultMessage: 'Choose what to include, then adjust individual files on the next step.',
    },
    selection: {
        id: 'app.export.selection',
        defaultMessage: '{count, plural, one {# file selected} other {# files selected}} · {size}',
    },
    count: {
        id: 'app.export.count',
        defaultMessage: '{count, plural, one {# file} other {# files}} · {size}',
    },
    selected: {
        id: 'app.export.selected',
        defaultMessage: 'Selected files',
    },
    search: {
        id: 'app.export.search',
        defaultMessage: 'Search file paths',
    },
    restore: {
        id: 'app.export.restore',
        defaultMessage: 'Restore category selection',
    },
    fileHint: {
        id: 'app.export.file-hint',
        defaultMessage:
            'Folder checkboxes include all files in the snapshot, even unopened subfolders. Manual exclusions are kept when you change categories.',
    },
    excluded: {
        id: 'app.export.excluded',
        defaultMessage:
            '{count, plural, one {# protected path excluded} other {# protected paths excluded}}. Authentication files, runtime libraries and symbolic links cannot be exported.',
    },
    noFiles: {
        id: 'app.export.no-files',
        defaultMessage: 'This instance has no exportable files.',
    },
    name: {
        id: 'app.export-modal.modpack-name-label',
        defaultMessage: 'Modpack name',
    },
    version: {
        id: 'app.export-modal.version-number-label',
        defaultMessage: 'Version number',
    },
    export: {
        id: 'app.export-modal.export-button',
        defaultMessage: 'Export',
    },
    complete: {
        id: 'app.export-modal.export-complete',
        defaultMessage: 'Export complete',
    },
    completeDescription: {
        id: 'app.export-modal.export-complete-description',
        defaultMessage: '{name} was exported successfully.',
    },
})

/** `apps/app-frontend/src/components/ui/export/messages.ts` */
export const exportCategoryMessages = defineMessages({
    gameSettings: {
        id: 'app.export.category.gameSettings',
        defaultMessage: 'Game settings',
    },
    personalRecords: {
        id: 'app.export.category.personalRecords',
        defaultMessage: 'Personal game records',
    },
    optifine: {
        id: 'app.export.category.optifine',
        defaultMessage: 'OptiFine settings',
    },
    oldResources: {
        id: 'app.export.category.oldResources',
        defaultMessage: 'Disabled and old resources',
    },
    mapRecords: {
        id: 'app.export.category.mapRecords',
        defaultMessage: 'Personal maps and waypoints',
    },
    jeiRecords: {
        id: 'app.export.category.jeiRecords',
        defaultMessage: 'JEI personal records',
    },
    emiRecords: {
        id: 'app.export.category.emiRecords',
        defaultMessage: 'EMI personal records',
    },
    patchouliRecords: {
        id: 'app.export.category.patchouliRecords',
        defaultMessage: 'Patchouli personal records',
    },
    mods: {
        id: 'app.export.category.mods',
        defaultMessage: 'Mods and plugins',
    },
    modConfig: {
        id: 'app.export.category.modConfig',
        defaultMessage: 'Mod configuration',
    },
    defaultConfig: {
        id: 'app.export.category.defaultConfig',
        defaultMessage: 'Default configuration',
    },
    importantData: {
        id: 'app.export.category.importantData',
        defaultMessage: 'Scripts, data packs and important resources',
    },
    resourcePacks: {
        id: 'app.export.category.resourcePacks',
        defaultMessage: 'Resource packs',
    },
    shaders: {
        id: 'app.export.category.shaders',
        defaultMessage: 'Shader packs and settings',
    },
    worlds: {
        id: 'app.export.category.worlds',
        defaultMessage: 'World saves',
    },
    screenshots: {
        id: 'app.export.category.screenshots',
        defaultMessage: 'Screenshots',
    },
    schematics: {
        id: 'app.export.category.schematics',
        defaultMessage: 'Schematics',
    },
    recordings: {
        id: 'app.export.category.recordings',
        defaultMessage: 'Replays and videos',
    },
    servers: {
        id: 'app.export.category.servers',
        defaultMessage: 'Server list',
    },
    other: {
        id: 'app.export.category.other',
        defaultMessage: 'Other folders and root files',
    },
})

/** `apps/app-frontend/src/pages/instance/Worlds.vue` */
export const worldsMessages = defineMessages({
    searchWorldsPlaceholder: {
        id: 'app.instance.worlds.search-worlds-placeholder',
        defaultMessage: 'Search {count} worlds...',
    },
    addServer: {
        id: 'app.instance.worlds.add-server',
        defaultMessage: 'Add server',
    },
    browseServers: {
        id: 'app.instance.worlds.browse-servers',
        defaultMessage: 'Browse servers',
    },
    browseMaps: {
        id: 'app.instance.worlds.browse-maps',
        defaultMessage: 'Browse maps',
    },
    noWorldsHeading: {
        id: 'app.instance.worlds.no-worlds-heading',
        defaultMessage: 'No servers or worlds added',
    },
    noWorldsDescription: {
        id: 'app.instance.worlds.no-worlds-description',
        defaultMessage: 'Add a server or browse to get started',
    },
    vanillaFilter: {
        id: 'app.instance.worlds.filter-vanilla',
        defaultMessage: 'Vanilla',
    },
    moddedFilter: {
        id: 'app.instance.worlds.filter-modded',
        defaultMessage: 'Modded',
    },
    onlineFilter: {
        id: 'app.instance.worlds.filter-online',
        defaultMessage: 'Online',
    },
    offlineFilter: {
        id: 'app.instance.worlds.filter-offline',
        defaultMessage: 'Offline',
    },
})

/** `apps/app-frontend/src/pages/instance/Files.vue` */
export const filesMessages = defineMessages({
    saveAs: {
        id: 'instance.files.save-as',
        defaultMessage: 'Save as...',
    },
    addingFiles: {
        id: 'instance.files.adding-files',
        defaultMessage: 'Adding files ({completed}/{total})',
    },
    openInSchematicWorkshop: {
        id: 'instance.files.open-in-schematic-workshop',
        defaultMessage: 'Open in schematic workshop',
    },
    openStudio: {
        id: 'instance.files.open-studio',
        defaultMessage: 'Open Studio',
    },
})

/** `apps/app-frontend/src/pages/instance/Logs.vue` */
export const logsMessages = defineMessages({
    liveLog: {
        id: 'instance.logs.source.live',
        defaultMessage: 'Live Log',
    },
    unknownLog: {
        id: 'instance.logs.source.unknown',
        defaultMessage: 'Unknown',
    },
    logName: {
        id: 'instance.logs.source.numbered',
        defaultMessage: 'Log {index}',
    },
    cannotDeleteLatest: {
        id: 'instance.logs.delete.latest-running',
        defaultMessage: 'Cannot delete latest.log while the instance is running',
    },
})

/** `apps/app-frontend/src/components/ui/modal/InstanceSettingsModal.vue` */
export const settingsTabMessages = defineMessages({
    tabsGeneral: {
        id: 'instance.settings.tabs.general',
        defaultMessage: 'General',
    },
    tabsInstallation: {
        id: 'instance.settings.tabs.installation',
        defaultMessage: 'Installation',
    },
    tabsSync: {
        id: 'instance.settings.tabs.sync',
        defaultMessage: 'Synchronization',
    },
    tabsBackups: {
        id: 'instance.settings.tabs.backups',
        defaultMessage: 'Backups',
    },
    tabsCoreComponents: {
        id: 'instance.settings.tabs.core-components',
        defaultMessage: 'Core components',
    },
    tabsWindow: {
        id: 'instance.settings.tabs.window',
        defaultMessage: 'Window',
    },
    tabsJava: {
        id: 'instance.settings.tabs.java',
        defaultMessage: 'Java and memory',
    },
    tabsHooks: {
        id: 'instance.settings.tabs.hooks',
        defaultMessage: 'Launch preparation',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/GeneralSettings.vue` */
export const generalSettingsMessages = defineMessages({
    nameRequired: {
        id: 'instance.settings.general.name-required',
        defaultMessage: 'Enter a name with no more than 80 characters.',
    },
    nameSaveFailed: {
        id: 'instance.settings.general.name-save-failed',
        defaultMessage: 'The name could not be saved.',
    },
    icon: {
        id: 'instance.settings.tabs.general.icon',
        defaultMessage: 'Icon',
    },
    name: {
        id: 'instance.settings.tabs.general.name',
        defaultMessage: 'Name',
    },
    editIcon: {
        id: 'instance.settings.tabs.general.edit-icon',
        defaultMessage: 'Edit icon',
    },
    selectIcon: {
        id: 'instance.settings.tabs.general.edit-icon.select',
        defaultMessage: 'Select icon',
    },
    replaceIcon: {
        id: 'instance.settings.tabs.general.edit-icon.replace',
        defaultMessage: 'Replace icon',
    },
    removeIcon: {
        id: 'instance.settings.tabs.general.edit-icon.remove',
        defaultMessage: 'Remove icon',
    },
    duplicateInstance: {
        id: 'instance.settings.tabs.general.duplicate-instance',
        defaultMessage: 'Duplicate instance',
    },
    duplicateInstanceDescription: {
        id: 'instance.settings.tabs.general.duplicate-instance.description',
        defaultMessage: 'Creates a copy of this instance, including worlds, configs, mods, etc.',
    },
    duplicateButtonTooltipInstalling: {
        id: 'instance.settings.tabs.general.duplicate-button.tooltip.installing',
        defaultMessage: 'Cannot duplicate while installing.',
    },
    duplicateButton: {
        id: 'instance.settings.tabs.general.duplicate-button',
        defaultMessage: 'Duplicate',
    },
    desktopShortcut: {
        id: 'instance.settings.tabs.general.desktop-shortcut',
        defaultMessage: 'Desktop shortcut',
    },
    desktopShortcutDescription: {
        id: 'instance.settings.tabs.general.desktop-shortcut.description',
        defaultMessage: 'Create a desktop shortcut that launches this instance directly.',
    },
    createDesktopShortcut: {
        id: 'instance.settings.tabs.general.desktop-shortcut.create',
        defaultMessage: 'Create desktop shortcut',
    },
    creatingDesktopShortcut: {
        id: 'instance.settings.tabs.general.desktop-shortcut.creating',
        defaultMessage: 'Creating shortcut...',
    },
    gameDir: {
        id: 'instance.settings.tabs.general.game-dir',
        defaultMessage: 'Game directory',
    },
    gameDirDescription: {
        id: 'instance.settings.tabs.general.game-dir.description',
        defaultMessage:
            'Uses a separate folder as the working directory for this instance. The game reads mods, saves, configs, and resource packs from that folder instead of the managed instance folder.',
    },
    gameDirCurrent: {
        id: 'instance.settings.tabs.general.game-dir.current',
        defaultMessage: 'Current directory',
    },
    gameDirIsolated: {
        id: 'instance.settings.tabs.general.game-dir.isolated',
        defaultMessage: 'Version isolated (stored in versions/)',
    },
    gameDirNotIsolated: {
        id: 'instance.settings.tabs.general.game-dir.not-isolated',
        defaultMessage: 'Version shared (.minecraft/)',
    },
    gameDirMoveNote: {
        id: 'instance.settings.tabs.general.game-dir.move-note',
        defaultMessage:
            'Switching isolation only updates the launcher path. Move the mods, saves, and config folders yourself to match.',
    },
    gameDirManagedNote: {
        id: 'instance.settings.tabs.general.game-dir.managed-note',
        defaultMessage: 'This instance uses the Axolotl-managed folder.',
    },
    gameDirExternalNote: {
        id: 'instance.settings.tabs.general.game-dir.external-note',
        defaultMessage: 'This instance uses an external .minecraft folder managed in place.',
    },
    updateChannel: {
        id: 'instance.settings.tabs.general.update-channel',
        defaultMessage: 'Update channel',
    },
    updateChannelReleaseDescription: {
        id: 'instance.settings.tabs.general.update-channel.release.description',
        defaultMessage: 'Only release versions will be shown as available updates.',
    },
    updateChannelBetaDescription: {
        id: 'instance.settings.tabs.general.update-channel.beta.description',
        defaultMessage: 'Release and beta versions will be shown as available updates.',
    },
    updateChannelAlphaDescription: {
        id: 'instance.settings.tabs.general.update-channel.alpha.description',
        defaultMessage: 'Release, beta, and alpha versions will be shown as available updates.',
    },
    updateChannelRelease: {
        id: 'instance.settings.tabs.general.update-channel.release',
        defaultMessage: 'Release',
    },
    updateChannelBeta: {
        id: 'instance.settings.tabs.general.update-channel.beta',
        defaultMessage: 'Beta',
    },
    updateChannelAlpha: {
        id: 'instance.settings.tabs.general.update-channel.alpha',
        defaultMessage: 'Alpha',
    },
    selectUpdateChannelAriaLabel: {
        id: 'instance.settings.tabs.general.update-channel.select',
        defaultMessage: 'Select update channel',
    },
    deleteInstance: {
        id: 'instance.settings.tabs.general.delete',
        defaultMessage: 'Delete instance',
    },
    deleteInstanceDescription: {
        id: 'instance.settings.tabs.general.delete.description',
        defaultMessage:
            'Permanently deletes an instance from your device, including your worlds, configs, and all installed content. Be careful, as once you delete a instance there is no way to recover it.',
    },
    deleteInstanceButton: {
        id: 'instance.settings.tabs.general.delete.button',
        defaultMessage: 'Delete instance',
    },
    deletingInstanceButton: {
        id: 'instance.settings.tabs.general.deleting.button',
        defaultMessage: 'Deleting...',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/InstallationSettings.vue` */
export const installationSettingsMessages = defineMessages({
    loaderVersion: {
        id: 'instance.settings.tabs.installation.loader-version',
        defaultMessage: '{loader} version',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/SyncSettings.vue` */
export const syncSettingsMessages = defineMessages({
    title: {
        id: 'instance.settings.sync.title',
        defaultMessage: 'Settings synchronization',
    },
    description: {
        id: 'instance.settings.sync.description',
        defaultMessage: 'Choose which settings this instance shares with other instances.',
    },
    unsupported: {
        id: 'instance.settings.sync.unsupported',
        defaultMessage: 'Unavailable for this instance',
    },
    gameOptions: {
        id: 'instance.settings.sync.game-options',
        defaultMessage: 'Game settings',
    },
    commandHistory: {
        id: 'instance.settings.sync.command-history',
        defaultMessage: 'Command history',
    },
    multiplayerServers: {
        id: 'instance.settings.sync.multiplayer-servers',
        defaultMessage: 'Multiplayer servers',
    },
    creativeHotbars: {
        id: 'instance.settings.sync.creative-hotbars',
        defaultMessage: 'Creative hotbars',
    },
    resourcePacks: {
        id: 'instance.settings.sync.resource-packs',
        defaultMessage: 'Resource packs',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/BackupSettings.vue` */
export const backupSettingsMessages = defineMessages({
    title: {
        id: 'instance.backups.title',
        defaultMessage: 'Instance backups',
    },
    description: {
        id: 'instance.backups.description',
        defaultMessage: 'Create deduplicated snapshots of the whole instance while it is closed.',
    },
    loading: {
        id: 'instance.backups.loading',
        defaultMessage: 'Loading backup settings...',
    },
    unavailable: {
        id: 'instance.backups.unavailable',
        defaultMessage: 'Backups are unavailable',
    },
    notInstalled: {
        id: 'instance.backups.unavailable.not-installed',
        defaultMessage: 'Finish installing this instance before enabling backups.',
    },
    symlinkInstance: {
        id: 'instance.backups.unavailable.symlink',
        defaultMessage: 'Imported symbolic-link instances cannot be backed up.',
    },
    directLinked: {
        id: 'instance.backups.unavailable.direct-linked',
        defaultMessage: 'Instances linked directly to another launcher cannot be backed up.',
    },
    externalDirectory: {
        id: 'instance.backups.unavailable.external-directory',
        defaultMessage: 'Instances that use an external game directory cannot be backed up.',
    },
    rootMissing: {
        id: 'instance.backups.unavailable.root-missing',
        defaultMessage: 'The instance folder is missing.',
    },
    rootLink: {
        id: 'instance.backups.unavailable.root-link',
        defaultMessage: 'Instances whose root folder is a link cannot be backed up.',
    },
    choose: {
        id: 'instance.backups.choose',
        defaultMessage:
            'Everything is backed up by default. Add files or folders that should be excluded.',
    },
    enable: {
        id: 'instance.backups.enable',
        defaultMessage: 'Enable backups',
    },
    editExclusions: {
        id: 'instance.backups.edit-exclusions',
        defaultMessage: 'Edit exclusions',
    },
    saveExclusions: {
        id: 'instance.backups.save-exclusions',
        defaultMessage: 'Save exclusions',
    },
    allIncluded: {
        id: 'instance.backups.all-included',
        defaultMessage: 'All instance files and folders are included.',
    },
    excludedPaths: {
        id: 'instance.backups.excluded-paths',
        defaultMessage: 'Excluded: {paths}',
    },
    backupNow: {
        id: 'instance.backups.backup-now',
        defaultMessage: 'Back up now',
    },
    cancelBackup: {
        id: 'instance.backups.cancel',
        defaultMessage: 'Cancel backup',
    },
    stageScanning: {
        id: 'instance.backups.stage.scanning',
        defaultMessage: 'Scanning files',
    },
    stageHashing: {
        id: 'instance.backups.stage.hashing',
        defaultMessage: 'Hashing files',
    },
    stageSaving: {
        id: 'instance.backups.stage.saving',
        defaultMessage: 'Saving snapshot index',
    },
    stageQueued: {
        id: 'instance.backups.stage.queued',
        defaultMessage: 'Waiting to start',
    },
    stageValidating: {
        id: 'instance.backups.stage.validating',
        defaultMessage: 'Validating restore',
    },
    stageMaterializing: {
        id: 'instance.backups.stage.materializing',
        defaultMessage: 'Preparing changed files',
    },
    stageApplying: {
        id: 'instance.backups.stage.applying',
        defaultMessage: 'Applying changed files',
    },
    stagePreview: {
        id: 'instance.backups.stage.preview',
        defaultMessage: 'Analyzing restore changes',
    },
    progressBytes: {
        id: 'instance.backups.progress-bytes',
        defaultMessage: '{processed} of {total}',
    },
    stageFailed: {
        id: 'instance.backups.stage.failed',
        defaultMessage: 'Backup failed',
    },
    stageInterrupted: {
        id: 'instance.backups.stage.interrupted',
        defaultMessage: 'Backup operation was interrupted',
    },
    snapshots: {
        id: 'instance.backups.snapshots',
        defaultMessage: 'Snapshots',
    },
    empty: {
        id: 'instance.backups.empty',
        defaultMessage: 'No backups have been created yet.',
    },
    snapshotMeta: {
        id: 'instance.backups.snapshot-meta',
        defaultMessage: '{files, plural, one {# file} other {# files}} · {size}',
    },
    addedSize: {
        id: 'instance.backups.added-size',
        defaultMessage: '{size} newly stored',
    },
    restore: {
        id: 'instance.backups.restore',
        defaultMessage: 'Restore backup',
    },
    delete: {
        id: 'instance.backups.delete',
        defaultMessage: 'Delete backup',
    },
    disable: {
        id: 'instance.backups.disable',
        defaultMessage: 'Disable backups',
    },
    disableBlocked: {
        id: 'instance.backups.disable-blocked',
        defaultMessage: 'Delete every snapshot before disabling backups.',
    },
    restoreTitle: {
        id: 'instance.backups.restore-title',
        defaultMessage: 'Restore this backup?',
    },
    restoreBody: {
        id: 'instance.backups.restore-body',
        defaultMessage:
            'Upcoming operations: add {added, plural, one {# file} other {# files}}, modify {modified, plural, one {# file} other {# files}}, and delete {deleted, plural, one {# file} other {# files}}. Continue?',
    },
    deleteTitle: {
        id: 'instance.backups.delete-title',
        defaultMessage: 'Delete this backup?',
    },
    deleteBody: {
        id: 'instance.backups.delete-body',
        defaultMessage:
            'This snapshot will be permanently deleted. Unused stored files will also be removed.',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/BackupExclusionSelector.vue` */
export const backupExclusionMessages = defineMessages({
    empty: {
        id: 'instance.backups.exclusions.empty',
        defaultMessage: 'No files or folders are excluded.',
    },
    addFile: {
        id: 'instance.backups.exclusions.add-file',
        defaultMessage: 'Add excluded file',
    },
    addFolder: {
        id: 'instance.backups.exclusions.add-folder',
        defaultMessage: 'Add excluded folder',
    },
    remove: {
        id: 'instance.backups.exclusions.remove',
        defaultMessage: 'Remove {path} from exclusions',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/CoreComponentsSettings.vue` */
export const coreComponentsMessages = defineMessages({
    add: {
        id: 'instance.settings.tabs.core-components.add',
        defaultMessage: 'Add to Minecraft.jar',
    },
    replace: {
        id: 'instance.settings.tabs.core-components.replace',
        defaultMessage: 'Replace Minecraft.jar',
    },
    moveUp: {
        id: 'instance.settings.tabs.core-components.move-up',
        defaultMessage: 'Move up',
    },
    moveDown: {
        id: 'instance.settings.tabs.core-components.move-down',
        defaultMessage: 'Move down',
    },
    remove: {
        id: 'instance.settings.tabs.core-components.remove',
        defaultMessage: 'Remove',
    },
    restore: {
        id: 'instance.settings.tabs.core-components.restore',
        defaultMessage: 'Restore',
    },
    jarMod: {
        id: 'instance.settings.tabs.core-components.jar-mod',
        defaultMessage: 'JAR mod',
    },
    replacement: {
        id: 'instance.settings.tabs.core-components.replacement',
        defaultMessage: 'Replacement JAR',
    },
    sha256: {
        id: 'instance.settings.tabs.core-components.sha256',
        defaultMessage: 'SHA-256',
    },
    failure: {
        id: 'instance.settings.tabs.core-components.failure',
        defaultMessage: 'Failure',
    },
    sha1: {
        id: 'instance.settings.tabs.core-components.sha1',
        defaultMessage: 'SHA-1',
    },
    source: {
        id: 'instance.settings.tabs.core-components.source',
        defaultMessage: 'Source',
    },
    targetVersion: {
        id: 'instance.settings.tabs.core-components.target-version',
        defaultMessage: 'Target Minecraft version',
    },
    preview: {
        id: 'instance.settings.tabs.core-components.preview',
        defaultMessage: 'Preview assembled JAR',
    },
    previewed: {
        id: 'instance.settings.tabs.core-components.previewed',
        defaultMessage: 'Assembled {components} components into {entries} entries',
    },
    modLoader: {
        id: 'instance.settings.tabs.core-components.modloader',
        defaultMessage: 'Install ModLoader',
    },
    modLoaderManual: {
        id: 'instance.settings.tabs.core-components.modloader-manual',
        defaultMessage:
            '{fileName} needs a manual download. Download it from the source page, then import the verified archive here.',
    },
    openSource: {
        id: 'instance.settings.tabs.core-components.open-source',
        defaultMessage: 'Open source page',
    },
    importModLoader: {
        id: 'instance.settings.tabs.core-components.import-modloader',
        defaultMessage: 'Import verified ModLoader archive',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/WindowSettings.vue` */
export const windowSettingsMessages = defineMessages({
    customWindowSettings: {
        id: 'instance.settings.tabs.window.custom-window-settings',
        defaultMessage: 'Custom window settings',
    },
    fullscreen: {
        id: 'instance.settings.tabs.window.fullscreen',
        defaultMessage: 'Fullscreen',
    },
    fullscreenDescription: {
        id: 'instance.settings.tabs.window.fullscreen.description',
        defaultMessage: 'Make the game start in full screen when launched (using options.txt).',
    },
    maximizeWindow: {
        id: 'instance.settings.tabs.window.maximize-window',
        defaultMessage: 'Maximize window',
    },
    maximizeWindowDescription: {
        id: 'instance.settings.tabs.window.maximize-window.description',
        defaultMessage: 'Maximize the Minecraft window when launched.',
    },
    maximizeWindowUnsupported: {
        id: 'instance.settings.tabs.window.maximize-window.unsupported',
        defaultMessage: 'Not supported on this operating system.',
    },
    width: {
        id: 'instance.settings.tabs.window.width',
        defaultMessage: 'Width',
    },
    widthDescription: {
        id: 'instance.settings.tabs.window.width.description',
        defaultMessage: 'The width of the game window when launched.',
    },
    enterWidth: {
        id: 'instance.settings.tabs.window.width.enter',
        defaultMessage: 'Enter width...',
    },
    height: {
        id: 'instance.settings.tabs.window.height',
        defaultMessage: 'Height',
    },
    heightDescription: {
        id: 'instance.settings.tabs.window.height.description',
        defaultMessage: 'The height of the game window when launched.',
    },
    enterHeight: {
        id: 'instance.settings.tabs.window.height.enter',
        defaultMessage: 'Enter height...',
    },
    windowTitle: {
        id: 'instance.settings.tabs.window.window-title',
        defaultMessage: 'Window title',
    },
    windowTitleDescription: {
        id: 'instance.settings.tabs.window.window-title.description',
        defaultMessage: 'Customize the Minecraft window title. Leave empty to keep the default.',
    },
    enterWindowTitle: {
        id: 'instance.settings.tabs.window.window-title.enter',
        defaultMessage: 'Enter window title...',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/JavaSettings.vue` */
export const javaSettingsMessages = defineMessages({
    javaInstallation: {
        id: 'instance.settings.tabs.java.java-installation',
        defaultMessage: 'Java installation',
    },
    customJavaInstallation: {
        id: 'instance.settings.tabs.java.custom-java-installation',
        defaultMessage: 'Use a custom Java installation for this instance',
    },
    javaPathPlaceholder: {
        id: 'instance.settings.tabs.java.java-path-placeholder',
        defaultMessage: '/path/to/java',
    },
    javaMemory: {
        id: 'instance.settings.tabs.java.java-memory',
        defaultMessage: 'Memory allocated',
    },
    customMemoryAllocation: {
        id: 'instance.settings.tabs.java.custom-memory-allocation',
        defaultMessage: 'Custom memory allocation',
    },
    automaticMemory: {
        id: 'instance.settings.tabs.java.automatic-memory',
        defaultMessage: 'Automatically allocate memory at launch',
    },
    optimizeMemoryBeforeLaunch: {
        id: 'instance.settings.tabs.java.optimize-memory-before-launch',
        defaultMessage: 'Optimize memory before launching the game',
    },
    optimizeMemoryBeforeLaunchDescription: {
        id: 'instance.settings.tabs.java.optimize-memory-before-launch-description',
        defaultMessage: 'Waits for Windows memory optimization to finish before starting the game.',
    },
    javaArguments: {
        id: 'instance.settings.tabs.java.java-arguments',
        defaultMessage: 'Java arguments',
    },
    customJavaArguments: {
        id: 'instance.settings.tabs.java.custom-java-arguments',
        defaultMessage: 'Custom Java arguments',
    },
    enterJavaArguments: {
        id: 'instance.settings.tabs.java.enter-java-arguments',
        defaultMessage: 'Enter Java arguments...',
    },
    javaEnvironmentVariables: {
        id: 'instance.settings.tabs.java.environment-variables',
        defaultMessage: 'Environment variables',
    },
    customEnvironmentVariables: {
        id: 'instance.settings.tabs.java.custom-environment-variables',
        defaultMessage: 'Custom environment variables',
    },
    enterEnvironmentVariables: {
        id: 'instance.settings.tabs.java.enter-environment-variables',
        defaultMessage: 'Enter environmental variables...',
    },
})

/** `apps/app-frontend/src/components/ui/instance_settings/HooksSettings.vue` */
export const hooksSettingsMessages = defineMessages({
    hooks: {
        id: 'instance.settings.tabs.hooks.title',
        defaultMessage: 'Launch preparation',
    },
    hooksDescription: {
        id: 'instance.settings.tabs.hooks.description',
        defaultMessage:
            'Configure the time allowed for launch preparation and optional commands that run before and after the game.',
    },
    launchPreparationTimeout: {
        id: 'instance.settings.tabs.hooks.launch-preparation-timeout',
        defaultMessage: 'Launch preparation timeout',
    },
    launchPreparationTimeoutDescription: {
        id: 'instance.settings.tabs.hooks.launch-preparation-timeout.description',
        defaultMessage:
            'Maximum time to wait for launch preparation to finish, in seconds (30–600).',
    },
    customLaunchPreparationTimeout: {
        id: 'instance.settings.tabs.hooks.custom-launch-preparation-timeout',
        defaultMessage: 'Use a custom launch preparation timeout',
    },
    customHooks: {
        id: 'instance.settings.tabs.hooks.custom-hooks',
        defaultMessage: 'Custom launch hooks',
    },
    preLaunch: {
        id: 'instance.settings.tabs.hooks.pre-launch',
        defaultMessage: 'Pre-launch',
    },
    preLaunchDescription: {
        id: 'instance.settings.tabs.hooks.pre-launch.description',
        defaultMessage: 'Ran before the instance is launched.',
    },
    preLaunchEnter: {
        id: 'instance.settings.tabs.hooks.pre-launch.enter',
        defaultMessage: 'Enter pre-launch command...',
    },
    wrapper: {
        id: 'instance.settings.tabs.hooks.wrapper',
        defaultMessage: 'Wrapper',
    },
    wrapperDescription: {
        id: 'instance.settings.tabs.hooks.wrapper.description',
        defaultMessage: 'Wrapper command for launching Minecraft.',
    },
    wrapperEnter: {
        id: 'instance.settings.tabs.hooks.wrapper.enter',
        defaultMessage: 'Enter wrapper command...',
    },
    postExit: {
        id: 'instance.settings.tabs.hooks.post-exit',
        defaultMessage: 'Post-exit',
    },
    postExitDescription: {
        id: 'instance.settings.tabs.hooks.post-exit.description',
        defaultMessage: 'Ran after the game closes.',
    },
    postExitEnter: {
        id: 'instance.settings.tabs.hooks.post-exit.enter',
        defaultMessage: 'Enter post-exit command...',
    },
})

/** `apps/app-frontend/src/components/ui/modal/ConfirmDeleteInstanceModal.vue` */
export const confirmDeleteInstanceMessages = defineMessages({
    header: {
        id: 'app.instance.confirm-delete.header',
        defaultMessage: 'Delete instance',
    },
    batchHeader: {
        id: 'app.instance.confirm-delete.batch-header',
        defaultMessage: 'Delete instances',
    },
    admonitionHeader: {
        id: 'app.instance.confirm-delete.admonition-header',
        defaultMessage: 'This action cannot be undone',
    },
    admonitionBody: {
        id: 'app.instance.confirm-delete.admonition-body',
        defaultMessage:
            'All data for your instance will be permanently deleted, including your worlds, configs, and all installed content.',
    },
    batchAdmonitionBody: {
        id: 'app.instance.confirm-delete.batch-admonition-body',
        defaultMessage:
            '{count, plural, one {# instance} other {# instances}} will be permanently deleted, including worlds, configs, and all installed content.',
    },
    symlinkDeleteWarning: {
        id: 'app.instance.confirm-delete.symlink-warning',
        defaultMessage:
            'This is a shared instance linked to "{path}". Only the link will be removed; the original files will not be deleted.',
    },
    backupCascadeWarning: {
        id: 'app.instance.confirm-delete.backup-cascade-warning',
        defaultMessage:
            '{count, plural, one {# backup} other {# backups}} ({size} total before deduplication) will also be permanently deleted.',
    },
    deleteButton: {
        id: 'app.instance.confirm-delete.delete-button',
        defaultMessage: 'Delete instance',
    },
    batchDeleteButton: {
        id: 'app.instance.confirm-delete.batch-delete-button',
        defaultMessage: 'Delete {count, plural, one {# instance} other {# instances}}',
    },
})

/** `apps/app-frontend/src/components/ui/JavaSelector.vue` */
export const javaSelectorMessages = defineMessages({
    alreadyInstalled: {
        id: 'app.java.already-installed',
        defaultMessage: 'Already installed',
    },
    installRecommended: {
        id: 'app.java.install-recommended',
        defaultMessage: 'Install recommended',
    },
    detect: {
        id: 'app.java.detect',
        defaultMessage: 'Detect',
    },
    select: {
        id: 'app.java.select',
        defaultMessage: 'Select',
    },
    browse: {
        id: 'app.java.browse',
        defaultMessage: 'Browse',
    },
    browseForExecutable: {
        id: 'app.java.browse-for-executable',
        defaultMessage: 'Browse for Java executable',
    },
    testInstallation: {
        id: 'app.java.test-installation',
        defaultMessage: 'Test Java installation',
    },
})

/** `apps/app-frontend/src/components/ui/JavaDetectionModal.vue` */
export const javaDetectionMessages = defineMessages({
    selectJavaVersion: {
        id: 'app.java.select-version',
        defaultMessage: 'Select Java version',
    },
    select: {
        id: 'app.java.select',
        defaultMessage: 'Select',
    },
    noneFound: {
        id: 'app.java.none-found',
        defaultMessage: 'No Java installations found!',
    },
    version: {
        id: 'app.java.table.version',
        defaultMessage: 'Version',
    },
    path: {
        id: 'app.java.table.path',
        defaultMessage: 'Path',
    },
    actions: {
        id: 'app.java.table.actions',
        defaultMessage: 'Actions',
    },
})

/** `apps/app-frontend/src/components/ui/JavaArgumentsInput.vue` */
export const javaArgumentsMessages = defineMessages({
    presetsButton: {
        id: 'app.java-arguments.presets.button',
        defaultMessage: 'Argument presets',
    },
    presetsModalTitle: {
        id: 'app.java-arguments.presets.modal-title',
        defaultMessage: 'Java argument presets',
    },
    usePreset: {
        id: 'app.java-arguments.presets.use',
        defaultMessage: 'Use preset',
    },
    presetApplied: {
        id: 'app.java-arguments.presets.applied',
        defaultMessage: 'Applied',
    },
    removePreset: {
        id: 'app.java-arguments.presets.remove',
        defaultMessage: 'Remove preset',
    },
    presetArguments: {
        id: 'app.java-arguments.presets.arguments',
        defaultMessage: 'Arguments',
    },
    collapseGroup: {
        id: 'app.java-arguments.presets.collapse-group',
        defaultMessage: 'Collapse group',
    },
    expandGroup: {
        id: 'app.java-arguments.presets.expand-group',
        defaultMessage: 'Expand group',
    },
    autoResolved: {
        id: 'app.java-arguments.presets.gc.auto.resolved',
        defaultMessage: 'Resolved → {strategy}',
    },
    autoReasonChain: {
        id: 'app.java-arguments.presets.gc.auto.reason-chain',
        defaultMessage: 'Decision path: {chain}',
    },
    autoVerified: {
        id: 'app.java-arguments.presets.gc.auto.verified',
        defaultMessage: 'Last launch: {chosen}',
    },
})

/** `apps/app-frontend/src/components/ui/EnvironmentVariablesInput.vue` */
export const environmentVariablesMessages = defineMessages({
    environmentVariablesInvalidFormat: {
        id: 'app.settings.environment-variables.invalid-format',
        defaultMessage:
            'Use NAME=value, separated by spaces. A name and an equals sign are required; the value may be empty or contain more equals signs.',
    },
})

/** `apps/app-frontend/src/components/ui/MemoryAllocationDisplay.vue` */
export const memoryAllocationMessages = defineMessages({
    used: {
        id: 'app.memory-display.used',
        defaultMessage: 'Memory in use',
    },
    game: {
        id: 'app.memory-display.game',
        defaultMessage: 'Game allocation',
    },
    remaining: {
        id: 'app.memory-display.remaining',
        defaultMessage: 'Remaining after allocation',
    },
    available: {
        id: 'app.memory-display.available',
        defaultMessage: '{memory} available',
    },
    optimize: {
        id: 'app.memory-display.optimize',
        defaultMessage: 'Optimize memory',
    },
    optimizing: {
        id: 'app.memory-display.optimizing',
        defaultMessage: 'Optimizing...',
    },
    optimizationDescription: {
        id: 'app.memory-display.optimization-description',
        defaultMessage: 'Free unused Windows working sets and standby memory.',
    },
    unsupported: {
        id: 'app.memory-display.unsupported',
        defaultMessage: 'Memory optimization is only available on Windows.',
    },
})

/** `apps/app-frontend/src/components/ui/SymlinkInstanceWarning.vue` */
export const symlinkWarningMessages = defineMessages({
    writeHeader: {
        id: 'app.symlink-warning.write.header',
        defaultMessage: 'Shared instance',
    },
    writeBody: {
        id: 'app.symlink-warning.write.body',
        defaultMessage:
            'This instance is linked to "{path}". Changes will also affect the original files.',
    },
    deleteHeader: {
        id: 'app.symlink-warning.delete.header',
        defaultMessage: 'Delete shared instance',
    },
    deleteBody: {
        id: 'app.symlink-warning.delete.body',
        defaultMessage:
            'Deleting this instance removes only the launcher entry. The original files at "{path}" will remain.',
    },
    dismissPermanently: {
        id: 'app.symlink-warning.dismiss-permanently',
        defaultMessage: "Don't show again for this instance",
    },
    sharedBadge: {
        id: 'app.symlink-warning.shared-badge',
        defaultMessage: 'Shared',
    },
})

/** Strings of the preview's own surfaces and ids missing from the groups above. */
export const previewMessages = defineMessages({
    timePlayedHours: {
        id: 'launcher-preview.instance.time-played.hours',
        defaultMessage: '{count, plural, one {# hour} other {# hours}}',
    },
    timePlayedMinutes: {
        id: 'launcher-preview.instance.time-played.minutes',
        defaultMessage: '{count, plural, one {# minute} other {# minutes}}',
    },
    timePlayedSeconds: {
        id: 'launcher-preview.instance.time-played.seconds',
        defaultMessage: '{count, plural, one {# second} other {# seconds}}',
    },
})

/** Preview-only: what the preview's launcher disk has no entries or contents for. */
export const filesPreviewMessages = defineMessages({
    folderUnavailable: {
        id: 'launcher-preview.instance.files.folder-unavailable',
        defaultMessage: "This preview doesn't include the files in this folder.",
    },
    contentUnavailable: {
        id: 'launcher-preview.instance.files.content-unavailable',
        defaultMessage: "This preview doesn't include this file's contents.",
    },
})
