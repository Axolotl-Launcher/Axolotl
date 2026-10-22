import { defineMessages } from '@modrinth/ui'

export const exportMessages = defineMessages({
    finalizingAnalysis: {
        id: 'app.export.finalizing-analysis',
        defaultMessage: 'Preparing verification results…',
    },
    queued: { id: 'app.export.queued', defaultMessage: 'Export queued' },
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
    author: { id: 'app.export.author', defaultMessage: 'Author (optional)' },
    format: { id: 'app.export.format', defaultMessage: 'Export format' },
    modrinth: { id: 'app.export.format.modrinth', defaultMessage: 'Modrinth standard (.mrpack)' },
    genericZip: { id: 'app.export.format.genericZip', defaultMessage: 'Generic ZIP (.zip)' },
    curseForge: { id: 'app.export.format.curseForge', defaultMessage: 'CurseForge ZIP (.zip)' },
    formatHint: {
        id: 'app.export.format-hint',
        defaultMessage:
            'Modrinth references matched MR files; generic ZIP uses verified public links; CurseForge references matched CF files. Other selected files are embedded.',
    },
    writing: { id: 'app.export.writing', defaultMessage: 'Writing the export archive…' },
    cancelExport: { id: 'app.export.cancel', defaultMessage: 'Cancel export' },
    analyzing: {
        id: 'app.export.analyzing',
        defaultMessage: 'Verifying exportable files and refreshing exact source matches…',
    },
    analyzed: {
        id: 'app.export.analyzed',
        defaultMessage:
            'Background verification finished. Unmatched selected files will be embedded.',
    },
    retryAnalysis: { id: 'app.export.retry-analysis', defaultMessage: 'Retry analysis' },
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
    categories: { id: 'app.export.categories', defaultMessage: 'Choose categories' },
    files: { id: 'app.export.files', defaultMessage: 'Adjust files' },
    package: { id: 'app.export.package', defaultMessage: 'Package information' },
    next: { id: 'app.export.next', defaultMessage: 'Next' },
    scan: { id: 'app.export.scan', defaultMessage: 'Scanning local files…' },
    scanFailed: {
        id: 'app.export.scan-failed',
        defaultMessage: 'Could not scan the instance files',
    },
    retry: { id: 'app.export.retry', defaultMessage: 'Retry scan' },
    requirements: { id: 'app.export.requirements', defaultMessage: 'Game and loader requirements' },
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
    selected: { id: 'app.export.selected', defaultMessage: 'Selected files' },
    search: { id: 'app.export.search', defaultMessage: 'Search file paths' },
    restore: { id: 'app.export.restore', defaultMessage: 'Restore category selection' },
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
    name: { id: 'app.export-modal.modpack-name-label', defaultMessage: 'Modpack name' },
    version: { id: 'app.export-modal.version-number-label', defaultMessage: 'Version number' },
    export: { id: 'app.export-modal.export-button', defaultMessage: 'Export' },
    complete: { id: 'app.export-modal.export-complete', defaultMessage: 'Export complete' },
    completeDescription: {
        id: 'app.export-modal.export-complete-description',
        defaultMessage: '{name} was exported successfully.',
    },
})

export const categoryMessages = defineMessages({
    gameSettings: { id: 'app.export.category.gameSettings', defaultMessage: 'Game settings' },
    personalRecords: {
        id: 'app.export.category.personalRecords',
        defaultMessage: 'Personal game records',
    },
    optifine: { id: 'app.export.category.optifine', defaultMessage: 'OptiFine settings' },
    oldResources: {
        id: 'app.export.category.oldResources',
        defaultMessage: 'Disabled and old resources',
    },
    mapRecords: {
        id: 'app.export.category.mapRecords',
        defaultMessage: 'Personal maps and waypoints',
    },
    jeiRecords: { id: 'app.export.category.jeiRecords', defaultMessage: 'JEI personal records' },
    emiRecords: { id: 'app.export.category.emiRecords', defaultMessage: 'EMI personal records' },
    patchouliRecords: {
        id: 'app.export.category.patchouliRecords',
        defaultMessage: 'Patchouli personal records',
    },
    mods: { id: 'app.export.category.mods', defaultMessage: 'Mods and plugins' },
    modConfig: { id: 'app.export.category.modConfig', defaultMessage: 'Mod configuration' },
    defaultConfig: {
        id: 'app.export.category.defaultConfig',
        defaultMessage: 'Default configuration',
    },
    importantData: {
        id: 'app.export.category.importantData',
        defaultMessage: 'Scripts, data packs and important resources',
    },
    resourcePacks: { id: 'app.export.category.resourcePacks', defaultMessage: 'Resource packs' },
    shaders: { id: 'app.export.category.shaders', defaultMessage: 'Shader packs and settings' },
    worlds: { id: 'app.export.category.worlds', defaultMessage: 'World saves' },
    screenshots: { id: 'app.export.category.screenshots', defaultMessage: 'Screenshots' },
    schematics: { id: 'app.export.category.schematics', defaultMessage: 'Schematics' },
    recordings: { id: 'app.export.category.recordings', defaultMessage: 'Replays and videos' },
    servers: { id: 'app.export.category.servers', defaultMessage: 'Server list' },
    other: { id: 'app.export.category.other', defaultMessage: 'Other folders and root files' },
})
