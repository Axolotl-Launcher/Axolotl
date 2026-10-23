import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/downloads.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the Downloads page. Ids and English text are the desktop app's
 * own (`apps/app-frontend/src/pages/Downloads.vue`).
 */
export const messages = defineMessages({
    newDownload: { id: 'app.downloads.new-download', defaultMessage: 'New download' },
    inProgress: { id: 'app.downloads.in-progress', defaultMessage: 'In progress' },
    history: { id: 'app.downloads.history', defaultMessage: 'History' },
    search: { id: 'app.downloads.search', defaultMessage: 'Search downloads' },
    allSources: { id: 'app.downloads.all-sources', defaultMessage: 'All sources' },
    allStatuses: { id: 'app.downloads.all-statuses', defaultMessage: 'All statuses' },
    application: { id: 'app.downloads.application', defaultMessage: 'Application' },
    local: { id: 'app.downloads.local', defaultMessage: 'Local' },
    clearHistory: { id: 'app.downloads.clear-history', defaultMessage: 'Clear history' },
    clearHistoryTitle: {
        id: 'app.downloads.clear-history-title',
        defaultMessage: 'Clear download history?',
    },
    cancel: { id: 'app.downloads.cancel', defaultMessage: 'Cancel' },
    retry: { id: 'app.downloads.retry', defaultMessage: 'Retry' },
    copyDiagnostics: { id: 'app.downloads.copy-diagnostics', defaultMessage: 'Copy diagnostics' },
    openInstance: { id: 'app.downloads.open-instance', defaultMessage: 'Open instance' },
    instanceDeleted: { id: 'app.downloads.instance-deleted', defaultMessage: 'Instance deleted' },
    details: { id: 'app.downloads.details', defaultMessage: 'Details' },
    hideDetails: { id: 'app.downloads.hide-details', defaultMessage: 'Hide details' },
    deleteRecord: { id: 'app.downloads.delete-record', defaultMessage: 'Delete record' },
    errorDetails: { id: 'app.downloads.error-details', defaultMessage: 'Download failed' },
    cleanupIncomplete: {
        id: 'app.action-bar.install.summary.cleanup-incomplete',
        defaultMessage: "Cleanup didn't finish",
    },
    noFileDetailsTitle: {
        id: 'app.downloads.no-file-details-title',
        defaultMessage: 'No file details',
    },
    noFileDetails: {
        id: 'app.downloads.no-file-details',
        defaultMessage: 'No file details were recorded for this download.',
    },
    emptyTitle: { id: 'app.downloads.empty-title', defaultMessage: 'No downloads yet' },
    emptyDescription: {
        id: 'app.downloads.empty-description',
        defaultMessage: 'New downloads and installation progress will appear here.',
    },
    noResultsTitle: {
        id: 'app.downloads.no-results-title',
        defaultMessage: 'No matching downloads',
    },
    noResultsDescription: {
        id: 'app.downloads.no-results-description',
        defaultMessage: 'Try changing your search or filters.',
    },
    confirmClear: {
        id: 'app.downloads.confirm-clear',
        defaultMessage:
            'Completed, failed, interrupted, and canceled records will be permanently deleted.',
    },
    instanceTarget: { id: 'app.downloads.instance-target', defaultMessage: 'Instance: {instance}' },
    fileDetails: { id: 'app.downloads.file-details', defaultMessage: 'File details' },
    openFileDetails: {
        id: 'app.downloads.open-file-details',
        defaultMessage: 'View details for {name}',
    },
    copyFileDetails: { id: 'app.downloads.copy-file-details', defaultMessage: 'Copy file details' },
    jobId: { id: 'app.downloads.job-id', defaultMessage: 'Task ID' },
    instanceId: { id: 'app.downloads.instance-id', defaultMessage: 'Instance ID' },
    projectId: { id: 'app.downloads.project-id', defaultMessage: 'Project ID' },
    versionId: { id: 'app.downloads.version-id', defaultMessage: 'Version ID' },
    requestUrl: { id: 'app.downloads.request-url', defaultMessage: 'Request URL' },
    notAvailable: { id: 'app.downloads.not-available', defaultMessage: '—' },
    itemName: { id: 'app.downloads.item-name', defaultMessage: 'File' },
    itemStatus: { id: 'app.downloads.item-status', defaultMessage: 'Status' },
    itemAttempts: { id: 'app.downloads.item-attempts', defaultMessage: 'Attempts' },
    attemptProgress: {
        id: 'app.downloads.item-attempt-progress',
        defaultMessage: '{attempt}/{maxAttempts}',
    },
    itemProgress: { id: 'app.downloads.item-progress', defaultMessage: 'Downloaded' },
    downloadSource: { id: 'app.downloads.download-source', defaultMessage: 'Source: {source}' },
    downloadSourceOfficial: { id: 'app.downloads.source.official', defaultMessage: 'Official' },
    downloadSourceAlternate: {
        id: 'app.downloads.source.alternate',
        defaultMessage: 'Alternate source',
    },
    downloadSpeed: { id: 'app.downloads.download-speed', defaultMessage: 'Speed: {speed}/s' },
    downloadEtaSeconds: {
        id: 'app.downloads.download-eta-seconds',
        defaultMessage: '{seconds}s remaining',
    },
    downloadEtaMinutes: {
        id: 'app.downloads.download-eta-minutes',
        defaultMessage: '{minutes}m remaining',
    },
    downloadEtaHours: {
        id: 'app.downloads.download-eta-hours',
        defaultMessage: '{hours}h {minutes}m remaining',
    },
    unknownPhase: { id: 'app.downloads.phase.unknown', defaultMessage: 'Working' },
})

export const statusMessages = defineMessages({
    queued: { id: 'app.downloads.status.queued', defaultMessage: 'Queued' },
    running: { id: 'app.downloads.status.running', defaultMessage: 'Running' },
    canceling: { id: 'app.downloads.status.canceling', defaultMessage: 'Canceling' },
    waiting_for_user: {
        id: 'app.downloads.status.waiting-for-user',
        defaultMessage: 'Action needed',
    },
    succeeded: { id: 'app.downloads.status.succeeded', defaultMessage: 'Completed' },
    failed: { id: 'app.downloads.status.failed', defaultMessage: 'Failed' },
    interrupted: { id: 'app.downloads.status.interrupted', defaultMessage: 'Interrupted' },
    canceled: { id: 'app.downloads.status.canceled', defaultMessage: 'Canceled' },
    completed: { id: 'app.downloads.item-status.completed', defaultMessage: 'Completed' },
    skipped: { id: 'app.downloads.item-status.skipped', defaultMessage: 'Skipped' },
    worker_started: { id: 'app.downloads.item-status.worker-started', defaultMessage: 'Starting' },
    downloading: { id: 'app.downloads.item-status.downloading', defaultMessage: 'Downloading' },
    connecting: { id: 'app.downloads.item-status.connecting', defaultMessage: 'Connecting' },
    waiting_for_resource: {
        id: 'app.downloads.item-status.waiting-for-resource',
        defaultMessage: 'Waiting for download resources',
    },
    verifying: { id: 'app.downloads.item-status.verifying', defaultMessage: 'Verifying' },
    writing: { id: 'app.downloads.item-status.writing', defaultMessage: 'Writing' },
    metadata: { id: 'app.downloads.item-status.metadata', defaultMessage: 'Loading metadata' },
    waiting_for_database: {
        id: 'app.downloads.item-status.waiting-for-database',
        defaultMessage: 'Waiting for database',
    },
    finalizing: { id: 'app.downloads.item-status.finalizing', defaultMessage: 'Finalizing' },
})

export const phaseMessages = defineMessages({
    exporting: { id: 'app.downloads.phase.exporting', defaultMessage: 'Exporting selected files' },
    preparing_instance: {
        id: 'app.downloads.phase.preparing-instance',
        defaultMessage: 'Preparing instance',
    },
    resolving_pack: {
        id: 'app.downloads.phase.resolving-pack',
        defaultMessage: 'Resolving modpack',
    },
    downloading_pack_file: {
        id: 'app.downloads.phase.downloading-pack-file',
        defaultMessage: 'Downloading modpack',
    },
    reading_pack_manifest: {
        id: 'app.downloads.phase.reading-pack-manifest',
        defaultMessage: 'Reading manifest',
    },
    downloading_content: {
        id: 'app.downloads.phase.downloading-content',
        defaultMessage: 'Downloading content',
    },
    extracting_overrides: {
        id: 'app.downloads.phase.extracting-overrides',
        defaultMessage: 'Extracting overrides',
    },
    resolving_minecraft: {
        id: 'app.downloads.phase.resolving-minecraft',
        defaultMessage: 'Resolving Minecraft',
    },
    resolving_loader: {
        id: 'app.downloads.phase.resolving-loader',
        defaultMessage: 'Resolving loader',
    },
    preparing_java: { id: 'app.downloads.phase.preparing-java', defaultMessage: 'Preparing Java' },
    downloading_minecraft: {
        id: 'app.downloads.phase.downloading-minecraft',
        defaultMessage: 'Downloading Minecraft',
    },
    running_loader_processors: {
        id: 'app.downloads.phase.running-loader-processors',
        defaultMessage: 'Installing loader',
    },
    creating_backup: {
        id: 'app.downloads.phase.creating-backup',
        defaultMessage: 'Creating backup',
    },
    staging_content: {
        id: 'app.downloads.phase.staging-content',
        defaultMessage: 'Staging content',
    },
    applying_content: {
        id: 'app.downloads.phase.applying-content',
        defaultMessage: 'Applying content',
    },
    updating_loader: {
        id: 'app.downloads.phase.updating-loader',
        defaultMessage: 'Updating loader',
    },
    verifying: { id: 'app.downloads.phase.verifying', defaultMessage: 'Verifying' },
    finalizing: { id: 'app.downloads.phase.finalizing', defaultMessage: 'Finalizing' },
    completed: { id: 'app.downloads.phase.completed', defaultMessage: 'Completed' },
    rolling_back: {
        id: 'app.downloads.phase.rolling-back',
        defaultMessage: 'Rolling back changes',
    },
})
