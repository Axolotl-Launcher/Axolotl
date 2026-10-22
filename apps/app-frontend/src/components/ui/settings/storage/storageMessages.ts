import { defineMessages } from '@modrinth/ui'

import type { StorageNodeType } from '@/helpers/storage'

export const storageMessages = defineMessages({
    openFolder: {
        id: 'app.settings.storage.openFolder',
        defaultMessage: 'Open folder in file manager',
    },
    scanSections: {
        id: 'app.settings.storage.scanSections',
        defaultMessage: '{completed} / {total} areas',
    },
    cachedScanHelp: {
        id: 'app.settings.storage.cachedScanHelp',
        defaultMessage: 'Showing the previous scan while storage usage is refreshed.',
    },
    clearCache: { id: 'app.settings.storage.clearCache', defaultMessage: 'Clear cache' },
    clearCacheHelp: {
        id: 'app.settings.storage.clearCacheHelp',
        defaultMessage:
            'Deletes downloaded mod and modpack caches, unused content objects and cached resource information. Installed games, saves and backups are kept. Files needed for offline use or recovery are kept. Downloads may be needed again.',
    },
    cacheCleanupResult: {
        id: 'app.settings.storage.cacheCleanupResult',
        defaultMessage: 'Freed {freed}. Remaining cache: {remaining}.',
    },
    cacheRetainedHelp: {
        id: 'app.settings.storage.cacheRetainedHelp',
        defaultMessage:
            'Instance icons, native archives and referenced content objects are kept for offline use and recovery. Cache files in previous data locations are kept. Cache database files also occupy some space.',
    },
    cachePartialFailure: {
        id: 'app.settings.storage.cachePartialFailure',
        defaultMessage:
            'Some cache files could not be cleared. See the details before trying again.',
    },
    cacheFailureDetails: {
        id: 'app.settings.storage.cacheFailureDetails',
        defaultMessage: 'Files that could not be cleared',
    },
    close: { id: 'app.settings.storage.close', defaultMessage: 'Close' },
    cacheClearFailed: {
        id: 'app.settings.storage.cacheClearFailed',
        defaultMessage: 'Could not clear the cache. Please try again.',
    },
    clearingCache: {
        id: 'app.settings.storage.clearingCache',
        defaultMessage: 'Clearing cache\u2026',
    },

    usageChart: {
        id: 'app.settings.storage.usageChart',
        defaultMessage: 'Storage usage by data type',
    },
    retainedAmount: {
        id: 'app.settings.storage.retainedAmount',
        defaultMessage: '{size} kept until usage can be verified',
    },
    unreadablePath: {
        id: 'app.settings.storage.unreadablePath',
        defaultMessage: 'Some files could not be read or are links. They are kept.',
    },
    externalMetadata: {
        id: 'app.settings.storage.externalMetadata',
        defaultMessage:
            'Linked instances also use this folder. Files with uncertain usage are kept.',
    },
    customArguments: {
        id: 'app.settings.storage.customArguments',
        defaultMessage: 'Custom launch arguments may use these files, so they are kept.',
    },
    activeOperation: {
        id: 'app.settings.storage.activeOperation',
        defaultMessage: 'Finish the current installation or recovery before cleaning shared files.',
    },
    databaseUnavailable: {
        id: 'app.settings.storage.databaseUnavailable',
        defaultMessage: 'Some launcher records could not be read. Unverified files are kept.',
    },
    processorUnresolved: {
        id: 'app.settings.storage.processorUnresolved',
        defaultMessage:
            'Some loader installation paths could not be read. Related shared files are kept. Versions: {names}.',
    },
    versionUnavailable: {
        id: 'app.settings.storage.versionUnavailable',
        defaultMessage:
            'Some installed versions are missing local version information. Shared files that may be needed are kept. Versions: {names}.',
    },
    legacyHelp: {
        id: 'app.settings.storage.legacyHelp',
        defaultMessage:
            'Copies of sounds, textures and other assets for older Minecraft versions. Unused copies can be cleaned after matching them to a local asset index.',
    },
    logConfigsHelp: {
        id: 'app.settings.storage.logConfigsHelp',
        defaultMessage: 'Files that control how Minecraft writes logs.',
    },
    localesHelp: {
        id: 'app.settings.storage.localesHelp',
        defaultMessage: 'Language indexes used by synced game settings.',
    },
    javaHelp: {
        id: 'app.settings.storage.javaHelp',
        defaultMessage: 'Java runtimes are kept because games can select them automatically.',
    },
    nativesHelp: {
        id: 'app.settings.storage.nativesHelp',
        defaultMessage: 'Platform files extracted for game versions.',
    },
    assetsHelp: {
        id: 'app.settings.storage.assetsHelp',
        defaultMessage: 'Sounds, textures and other game assets.',
    },
    librariesHelp: {
        id: 'app.settings.storage.librariesHelp',
        defaultMessage: 'Shared files needed to run Minecraft and its loaders.',
    },
    versionsHelp: {
        id: 'app.settings.storage.versionsHelp',
        defaultMessage:
            'Minecraft versions and their loaders. Expand to choose individual versions.',
    },
    metaItemCount: { id: 'app.settings.storage.metaItemCount', defaultMessage: '{count} items' },
    refreshingTotals: {
        id: 'app.settings.storage.refreshingTotals',
        defaultMessage: 'Refreshing storage totals…',
    },
    availableSpace: {
        id: 'app.settings.storage.availableSpace',
        defaultMessage: '{size} can be cleaned',
    },
    linkedSize: { id: 'app.settings.storage.linkedSize', defaultMessage: 'Linked files: {size}' },
    measurementTitle: {
        id: 'app.settings.storage.measurementTitle',
        defaultMessage: 'How sizes are counted',
    },
    fileSize: { id: 'app.settings.storage.fileSize', defaultMessage: 'Size' },
    dataType: { id: 'app.settings.storage.dataType', defaultMessage: 'Data type' },
    analysisTitle: { id: 'app.settings.storage.analysisTitle', defaultMessage: 'Storage analysis' },
    analysisDescription: {
        id: 'app.settings.storage.analysisDescription',
        defaultMessage:
            'Inspect launcher and game storage, then analyze unused metadata for cleanup.',
    },
    unknownUsage: {
        id: 'app.settings.storage.unknownUsage',
        defaultMessage: 'Files whose usage cannot be verified are kept.',
    },

    refreshFailed: {
        id: 'app.settings.storage.refresh-failed',
        defaultMessage: 'Files were deleted, but the storage total could not be refreshed.',
    },
    resourceBackups: {
        id: 'app.settings.storage.resource-backups',
        defaultMessage: 'Previous resource files',
    },
    previewCache: {
        id: 'app.settings.storage.preview-cache',
        defaultMessage: 'Avatar and skin preview cache',
    },
    previewCacheHelp: {
        id: 'app.settings.storage.preview-cache-help',
        defaultMessage:
            'Image payload sizes stored by this WebView. Database overhead is excluded; this value is separate from the launcher file total.',
    },
    backups: { id: 'app.settings.storage.backups', defaultMessage: 'Instance backups' },
    servers: { id: 'app.settings.storage.servers', defaultMessage: 'Managed servers' },
    syncedOptions: {
        id: 'app.settings.storage.syncedOptions',
        defaultMessage: 'Synced settings and files',
    },
    components: { id: 'app.settings.storage.components', defaultMessage: 'Multiplayer components' },
    launcherLogs: { id: 'app.settings.storage.launcherLogs', defaultMessage: 'Launcher logs' },
    launcherData: {
        id: 'app.settings.storage.launcherData',
        defaultMessage: 'Launcher settings and appearance',
    },
    icons: { id: 'app.settings.storage.icons', defaultMessage: 'Icons' },
    installRollbacks: {
        id: 'app.settings.storage.installRollbacks',
        defaultMessage: 'Installation rollback data',
    },
    contentStore: {
        id: 'app.settings.storage.contentStore',
        defaultMessage: 'Shared content files',
    },
    resourceCache: {
        id: 'app.settings.storage.resourceCache',
        defaultMessage: 'Resource metadata cache',
    },
    staging: { id: 'app.settings.storage.staging', defaultMessage: 'Operation staging' },
    folder: { id: 'app.settings.storage.folder', defaultMessage: 'Files' },
    javaVersions: { id: 'app.settings.storage.javaVersions', defaultMessage: 'Java runtimes' },
    versions: { id: 'app.settings.storage.versions', defaultMessage: 'Game versions' },
    libraries: { id: 'app.settings.storage.libraries', defaultMessage: 'Shared libraries' },
    assets: { id: 'app.settings.storage.assets', defaultMessage: 'Game assets' },
    natives: { id: 'app.settings.storage.natives', defaultMessage: 'Native libraries' },
    logConfigs: { id: 'app.settings.storage.logConfigs', defaultMessage: 'Logging configurations' },
    gameLocales: {
        id: 'app.settings.storage.gameLocales',
        defaultMessage: 'Game setting language indexes',
    },
    resources: { id: 'app.settings.storage.resources', defaultMessage: 'Legacy game resources' },
    logs: { id: 'app.settings.storage.logs', defaultMessage: 'Game logs' },
    crashReports: { id: 'app.settings.storage.crashReports', defaultMessage: 'Crash reports' },
    scanFailed: {
        id: 'app.settings.storage.scanFailed',
        defaultMessage:
            'Storage could not be scanned. The last complete snapshot is kept when available.',
    },
    retry: { id: 'app.settings.storage.retry', defaultMessage: 'Retry' },
    measurementHelp: {
        id: 'app.settings.storage.measurementHelp',
        defaultMessage:
            'Sizes are file lengths, not allocated disk space. Linked files are shown separately and may also be referenced elsewhere.',
    },
    incompleteScan: {
        id: 'app.settings.storage.incompleteScan',
        defaultMessage: 'Some paths could not be read; these totals are incomplete.',
    },
    externalData: {
        id: 'app.settings.storage.externalData',
        defaultMessage: 'External game directories',
    },
    externalHelp: {
        id: 'app.settings.storage.externalHelp',
        defaultMessage:
            'These directories are managed outside Axolotl and are excluded from the launcher total. Overlapping directories are listed once.',
    },
    analyzeMeta: { id: 'app.settings.storage.analyzeMeta', defaultMessage: 'Check unused files' },
    analyzingMeta: {
        id: 'app.settings.storage.analyzingMeta',
        defaultMessage: 'Checking file usage…',
    },
    metaHelp: {
        id: 'app.settings.storage.metaHelp',
        defaultMessage:
            'These files are shared across instances. Check their usage before choosing what to clean.',
    },
    used: { id: 'app.settings.storage.used', defaultMessage: 'In use' },
    unused: { id: 'app.settings.storage.unused', defaultMessage: 'Can be cleaned' },
    unknown: { id: 'app.settings.storage.unknown', defaultMessage: 'Kept' },
    metaGroup: { id: 'app.settings.storage.metaGroup', defaultMessage: 'Metadata group' },
    analyzeFailed: {
        id: 'app.settings.storage.analyzeFailed',
        defaultMessage:
            'Metadata could not be analyzed. Try again after current operations finish.',
    },
    cleanFailed: {
        id: 'app.settings.storage.cleanFailed',
        defaultMessage: 'Cleanup could not finish. Refresh the analysis before retrying.',
    },
    cleanSelected: {
        id: 'app.settings.storage.cleanSelected',
        defaultMessage: 'Clean selected files',
    },
    cleanTitle: {
        id: 'app.settings.storage.cleanTitle',
        defaultMessage: 'Clean shared game files',
    },
    cleanConfirm: {
        id: 'app.settings.storage.cleanConfirm',
        defaultMessage:
            'Permanently delete {size} of unused files? Files that are still needed will be kept.',
    },
    cleaning: { id: 'app.settings.storage.cleaning', defaultMessage: 'Deleting files…' },
    cancel: { id: 'app.settings.storage.cancel', defaultMessage: 'Cancel' },
    cleanResult: {
        id: 'app.settings.storage.cleanResult',
        defaultMessage:
            'Freed {size} by deleting {count} files. {skipped} changed or referenced files were kept.',
    },
    cleanFailures: {
        id: 'app.settings.storage.cleanFailures',
        defaultMessage: 'Some files could not be deleted.',
    },
    noUnused: {
        id: 'app.settings.storage.noUnused',
        defaultMessage: 'No files are currently available to clean.',
    },
    metaWarnings: {
        id: 'app.settings.storage.metaWarnings',
        defaultMessage: 'Why some files are kept',
    },
    selectGroup: {
        id: 'app.settings.storage.selectGroup',
        defaultMessage: 'Select unused files in {name}',
    },
    selectedSize: {
        id: 'app.settings.storage.selectedSize',
        defaultMessage: '{count} items selected · {size}',
    },
    refreshRequired: {
        id: 'app.settings.storage.refreshRequired',
        defaultMessage:
            'The analysis is a snapshot. Analyze again after installing, removing or changing instances.',
    },
    details: { id: 'app.settings.storage.details', defaultMessage: 'Details' },

    total: {
        id: 'app.settings.storage.total',
        defaultMessage: 'Total size',
    },
    totalDescription: {
        id: 'app.settings.storage.total-description',
        defaultMessage: 'Total size including symbolic links and junctions',
    },
    instanceData: {
        id: 'app.settings.storage.instance-data',
        defaultMessage: 'Instance data',
    },
    cacheData: {
        id: 'app.settings.storage.cache-data',
        defaultMessage: 'Cache data',
    },
    metaData: { id: 'app.settings.storage.meta-data', defaultMessage: 'Shared game files' },
    database: {
        id: 'app.settings.storage.database',
        defaultMessage: 'Database',
    },
    other: {
        id: 'app.settings.storage.other',
        defaultMessage: 'Other',
    },
    instance: {
        id: 'app.settings.storage.instance',
        defaultMessage: 'Instance',
    },
    mods: {
        id: 'app.settings.storage.mods',
        defaultMessage: 'Mods',
    },
    replay: {
        id: 'app.settings.storage.replay',
        defaultMessage: 'Replay recordings',
    },
    resourcepacks: {
        id: 'app.settings.storage.resourcepacks',
        defaultMessage: 'Resource packs',
    },
    saves: {
        id: 'app.settings.storage.saves',
        defaultMessage: 'Saves',
    },
    world: {
        id: 'app.settings.storage.world',
        defaultMessage: 'World',
    },
    schematics: {
        id: 'app.settings.storage.schematics',
        defaultMessage: 'Schematics',
    },
    screenshots: {
        id: 'app.settings.storage.screenshots',
        defaultMessage: 'Screenshots',
    },
    shaderpacks: {
        id: 'app.settings.storage.shaderpacks',
        defaultMessage: 'Shader packs',
    },
    minimap: {
        id: 'app.settings.storage.minimap',
        defaultMessage: 'Minimap data',
    },
    distantHorizons: {
        id: 'app.settings.storage.distant-horizons',
        defaultMessage: 'Distant Horizons cache',
    },
    dbFile: {
        id: 'app.settings.storage.db-file',
        defaultMessage: 'App database',
    },
    dbBackup: {
        id: 'app.settings.storage.db-backup',
        defaultMessage: 'Database backups',
    },
    itemCount: {
        id: 'app.settings.storage.item-count',
        defaultMessage: '{count} items',
    },
    instanceCount: {
        id: 'app.settings.storage.instance-count',
        defaultMessage: '{count} instances',
    },
    actualSizeTooltip: {
        id: 'app.settings.storage.actual-size-tooltip',
        defaultMessage: 'File size: {size}',
    },
    symlinkSizeTooltip: {
        id: 'app.settings.storage.symlink-size-tooltip',
        defaultMessage: 'Symbolic link or junction referenced size: {size}',
    },
    openAction: {
        id: 'app.settings.storage.open-action',
        defaultMessage: 'Open in launcher or open location',
    },
    symlinkLabel: {
        id: 'app.settings.storage.symlink-label',
        defaultMessage: 'symlink',
    },
    symlinkHelp: {
        id: 'app.settings.storage.symlink-help',
        defaultMessage: 'What is a symlink?',
    },
    symlinkHelpTooltip: {
        id: 'app.settings.storage.symlink-help-tooltip',
        defaultMessage:
            'Symbolic links reference Minecraft resources from other locations — these files may actually live in another launcher\u2019s directory.\nFor example, \u201c20MB + 1.2GB\u201d means the launcher folder contains 20MB of files and references an external 1.2GB of files.',
    },
    update: {
        id: 'app.settings.storage.update',
        defaultMessage: 'Update',
    },
    updating: {
        id: 'app.settings.storage.updating',
        defaultMessage: 'Updating…',
    },
    lastUpdatedLabel: {
        id: 'app.settings.storage.last-updated-label',
        defaultMessage: 'Last updated:',
    },
    scanning: {
        id: 'app.settings.storage.scanning',
        defaultMessage: 'Scanning storage…',
    },
    storageTab: {
        id: 'app.settings.tabs.storage',
        defaultMessage: 'Storage',
    },
})

export const storageLabels = {
    instances: storageMessages.instanceData,
    cache: storageMessages.cacheData,
    meta: storageMessages.metaData,
    database: storageMessages.database,
    other: storageMessages.other,
    instance: storageMessages.instance,
    mods: storageMessages.mods,
    replay: storageMessages.replay,
    resourcepacks: storageMessages.resourcepacks,
    saves: storageMessages.saves,
    world: storageMessages.world,
    schematics: storageMessages.schematics,
    screenshots: storageMessages.screenshots,
    shaderpacks: storageMessages.shaderpacks,
    minimap: storageMessages.minimap,
    'distant-horizons': storageMessages.distantHorizons,
    'db-file': storageMessages.dbFile,
    'db-backup': storageMessages.dbBackup,
    backups: storageMessages.backups,
    servers: storageMessages.servers,
    synced_options: storageMessages.syncedOptions,
    components: storageMessages.components,
    launcher_logs: storageMessages.launcherLogs,
    launcher_data: storageMessages.launcherData,
    icons: storageMessages.icons,
    install_rollbacks: storageMessages.installRollbacks,
    content_store: storageMessages.contentStore,
    resource_backups: storageMessages.resourceBackups,
    resource_cache: storageMessages.resourceCache,
    staging: storageMessages.staging,
    folder: storageMessages.folder,
    java_versions: storageMessages.javaVersions,
    versions: storageMessages.versions,
    libraries: storageMessages.libraries,
    assets: storageMessages.assets,
    natives: storageMessages.natives,
    log_configs: storageMessages.logConfigs,
    game_locales: storageMessages.gameLocales,
    resources: storageMessages.resources,
    logs: storageMessages.logs,
    crash_reports: storageMessages.crashReports,
} satisfies Record<StorageNodeType, (typeof storageMessages)['total']>
