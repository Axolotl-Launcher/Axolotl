<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.header)"
        :fade="remaining > 0 ? 'warning' : 'standard'"
        :on-hide="stopScanning"
        max-width="760px"
        scrollable
    >
        <div class="flex flex-col gap-4">
            <Admonition
                :type="continuing || remaining === 0 ? 'success' : 'warning'"
                :header="
                    continuing
                        ? formatMessage(messages.continuing)
                        : formatMessage(messages.remaining, { count: remaining })
                "
            >
                <template #icon>
                    <SpinnerIcon v-if="loading || continuing" class="size-5 animate-spin" />
                    <CheckIcon v-else-if="remaining === 0" class="size-5 text-green" />
                    <DownloadIcon v-else class="size-5" />
                </template>
                {{
                    continuing
                        ? formatMessage(messages.continuingBody)
                        : formatMessage(messages.body)
                }}
            </Admonition>

            <Admonition
                v-if="!continuing && remaining > 0"
                :type="scannerPresentation.phase === 'rejected' ? 'warning' : 'info'"
                :header="scannerHeader"
            >
                <template #icon>
                    <SpinnerIcon
                        v-if="
                            scannerPresentation.phase === 'importing' ||
                            scannerPresentation.phase === 'verifying' ||
                            scannerPresentation.phase === 'waiting_for_stability'
                        "
                        class="size-5 animate-spin"
                    />
                    <FolderSearchIcon v-else class="size-5" />
                </template>
                {{ scannerStatus }}
            </Admonition>

            <div
                v-if="files.length"
                class="flex flex-col overflow-hidden rounded-lg border border-surface-5"
            >
                <div
                    v-for="file in files"
                    :key="file.itemId"
                    class="flex flex-col gap-3 border-0 border-b border-solid border-surface-5 bg-surface-2 p-4 last:border-b-0"
                >
                    <div class="flex min-w-0 items-start justify-between gap-3">
                        <div class="min-w-0">
                            <div class="break-all font-medium text-[var(--color-text-primary)]">
                                {{ file.path }}
                            </div>
                            <div
                                class="mt-1 flex flex-wrap gap-2 text-sm text-[var(--color-text-tertiary)]"
                            >
                                <span>{{
                                    formatMessage(messages.expectedSize, {
                                        size: formatBytes(file.expectedSize),
                                    })
                                }}</span>
                                <span>·</span>
                                <span>{{ attemptText(file) }}</span>
                                <span v-if="file.browserUrls.length > 1">·</span>
                                <span v-if="file.browserUrls.length > 1">
                                    {{
                                        formatMessage(messages.fallbacks, {
                                            count: file.browserUrls.length - 1,
                                        })
                                    }}
                                </span>
                            </div>
                            <div
                                v-if="file.browserUrls[0]"
                                class="mt-1 truncate text-xs text-[var(--color-text-tertiary)]"
                            >
                                <code v-tooltip="file.browserUrls[0]">{{
                                    file.browserUrls[0]
                                }}</code>
                            </div>
                            <div v-if="file.lastError" class="mt-1 text-sm text-red">
                                {{ file.lastError }}
                            </div>
                            <div
                                v-if="scannerItemStatus(file.itemId)"
                                class="mt-1 text-sm text-orange"
                            >
                                {{ scannerItemStatus(file.itemId) }}
                            </div>
                        </div>
                        <Badge :color="statusColor(file.status)" :type="statusLabel(file.status)" />
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            size="2xs"
                            :disabled="isBusy(file.itemId)"
                            @click="retryOne(file.itemId)"
                            ><SpinnerIcon v-if="isBusy(file.itemId)" class="animate-spin" />
                            <RefreshCwIcon v-else />
                            {{ formatMessage(messages.retry) }}
                        </Button>
                        <Button
                            v-if="file.browserUrls.length"
                            type="outlined"
                            size="2xs"
                            :disabled="isBusy(file.itemId)"
                            @click="openBrowser(file.browserUrls[0])"
                            ><ExternalIcon />{{ formatMessage(messages.browserDownload) }}
                        </Button>
                        <Button
                            type="outlined"
                            size="2xs"
                            :disabled="isBusy(file.itemId)"
                            @click="selectLocal(file.itemId)"
                            ><UploadIcon />{{ formatMessage(messages.chooseFile) }}
                        </Button>
                    </div>
                </div>
            </div>
        </div>

        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button type="outlined" @click="modal?.hide()">{{
                    formatMessage(commonMessages.closeButton)
                }}</Button>
                <Button
                    v-if="remaining > 0"
                    type="colored"
                    color="brand"
                    :disabled="loading || busy.size > 0"
                    @click="retryAll"
                    ><RefreshCwIcon />{{ formatMessage(messages.retryAll) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import {
    CheckIcon,
    DownloadIcon,
    ExternalIcon,
    FolderSearchIcon,
    RefreshCwIcon,
    SpinnerIcon,
    UploadIcon,
} from '@modrinth/assets'
import {
    Admonition,
    Badge,
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    NewModal,
    useFormatBytes,
    useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onUnmounted, ref } from 'vue'

import { resolveDownloadUrl } from '@/helpers/download-url'
import { retiredDownloadMessage } from '@/helpers/download-url-messages'
import {
    createDownloadsScanLoop,
    createDownloadsScannerPresentationState,
    getMissingContentScannerSettings,
    reduceDownloadsScannerPresentation,
} from '@/helpers/downloads-scanner'
import { install_job_listener } from '@/helpers/events'
import {
    install_job_get,
    install_job_import_missing_file,
    install_job_missing_files,
    install_job_resume,
    install_job_retry_missing_file,
    install_job_scan_missing_files,
    type InstallJobSnapshot,
    type MissingModpackContentView,
} from '@/helpers/install'

type MissingFile = MissingModpackContentView['files'][number]

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const { handleError } = injectNotificationManager()
const modal = ref<InstanceType<typeof NewModal>>()
const jobId = ref<string | null>(null)
const content = ref<MissingModpackContentView>({ remaining: 0, files: [] })
const loading = ref(false)
const continuing = ref(false)
const busy = ref(new Set<string>())
const scannerPresentation = ref(createDownloadsScannerPresentationState())
const scannerErrors = ref(new Map<string, string>())
const scannerEnabled = ref(true)
const scanDirectory = ref<string | null>(null)
let unlisten: (() => void) | null = null

const files = computed(() => content.value.files)
const remaining = computed(() => content.value.remaining)
const scannerHeader = computed(() => {
    if (!scannerEnabled.value) return formatMessage(messages.automaticImportDisabled)
    if (scannerPresentation.value.phase === 'rejected') {
        return formatMessage(messages.fileMismatchTitle)
    }
    if (scannerPresentation.value.phase === 'verifying') {
        return formatMessage(messages.verifyingCandidate)
    }
    if (scannerPresentation.value.phase === 'importing') {
        return formatMessage(messages.importingCandidate)
    }
    if (
        scannerPresentation.value.phase === 'monitoring' ||
        scannerPresentation.value.phase === 'idle'
    ) {
        return formatMessage(messages.watchingDownloadsTitle)
    }
    return formatMessage(messages.automaticImport)
})
const scannerStatus = computed(() => {
    if (!scannerEnabled.value) return formatMessage(messages.automaticImportDisabledBody)
    const state = scannerPresentation.value
    if (state.phase === 'importing') return formatMessage(messages.importingCandidateBody)
    if (state.phase === 'verifying') return formatMessage(messages.verifyingCandidateBody)
    if (state.phase === 'waiting_for_stability') return formatMessage(messages.waitingForCompletion)
    if (state.phase === 'rejected') return formatMessage(messages.fileMismatchBody)
    if (state.phase === 'imported') {
        return formatMessage(messages.importedAutomatically, {
            count: state.importedCount,
            remaining: remaining.value,
        })
    }
    if (state.phase === 'error') return formatMessage(messages.scannerFailed)
    if (state.phase === 'monitoring' && state.downloadDirectory) {
        return formatMessage(messages.watchingDirectory, { path: state.downloadDirectory })
    }
    if (state.phase === 'idle') {
        return formatMessage(messages.watchingDownloadsBody)
    }
    return formatMessage(messages.downloadsUnavailable)
})

const messages = defineMessages({
    header: {
        id: 'app.downloads.missing-content.header',
        defaultMessage: 'Complete missing files',
    },
    body: {
        id: 'app.downloads.missing-content.body',
        defaultMessage:
            'Each local file is verified before it can replace the required instance file.',
    },
    remaining: {
        id: 'app.downloads.missing-content.remaining',
        defaultMessage:
            '{count, plural, one {# file still needs to be completed} other {# files still need to be completed}}',
    },
    continuing: {
        id: 'app.downloads.missing-content.continuing',
        defaultMessage: 'Continuing installation',
    },
    continuingBody: {
        id: 'app.downloads.missing-content.continuing-body',
        defaultMessage:
            'All required files are ready. The launcher is verifying them again and continuing installation.',
    },
    expectedSize: {
        id: 'app.downloads.missing-content.expected-size',
        defaultMessage: 'Expected size: {size}',
    },
    fallbacks: {
        id: 'app.downloads.missing-content.fallbacks',
        defaultMessage: '{count} fallback links',
    },
    retry: { id: 'app.downloads.missing-content.retry', defaultMessage: 'Retry download' },
    retryAll: {
        id: 'app.downloads.missing-content.retry-all',
        defaultMessage: 'Retry all missing files',
    },
    browserDownload: {
        id: 'app.downloads.missing-content.browser-download',
        defaultMessage: 'Browser download',
    },
    chooseFile: {
        id: 'app.downloads.missing-content.choose-file',
        defaultMessage: 'Choose local file',
    },
    attempts: {
        id: 'app.downloads.missing-content.attempts',
        defaultMessage: 'Attempt {attempt}/{max}',
    },
    noAttempts: {
        id: 'app.downloads.missing-content.no-attempts',
        defaultMessage: 'No attempt information',
    },
    automaticImport: {
        id: 'app.downloads.missing-content.automatic-import',
        defaultMessage: 'Automatic import from monitored folder',
    },
    automaticImportDisabled: {
        id: 'app.downloads.missing-content.automatic-import-disabled',
        defaultMessage: 'Automatic import is disabled',
    },
    automaticImportDisabledBody: {
        id: 'app.downloads.missing-content.automatic-import-disabled-body',
        defaultMessage: 'Retry downloading or choose each missing file manually.',
    },
    verifyingCandidate: {
        id: 'app.downloads.missing-content.verifying-candidate',
        defaultMessage: 'Verifying downloaded file',
    },
    verifyingCandidateBody: {
        id: 'app.downloads.missing-content.verifying-candidate-body',
        defaultMessage: 'Checking that the candidate matches the file required by the modpack.',
    },
    importingCandidate: {
        id: 'app.downloads.missing-content.importing-candidate',
        defaultMessage: 'Importing verified file',
    },
    importingCandidateBody: {
        id: 'app.downloads.missing-content.importing-candidate-body',
        defaultMessage: 'Adding the verified file to the instance...',
    },
    watchingDownloadsTitle: {
        id: 'app.downloads.missing-content.watching-downloads-title',
        defaultMessage: 'Watching the import folder',
    },
    watchingDownloadsBody: {
        id: 'app.downloads.missing-content.watching-downloads-body',
        defaultMessage: 'Completed downloads will be verified and imported automatically.',
    },
    watchingDirectory: {
        id: 'app.downloads.missing-content.watching-downloads',
        defaultMessage: 'Watching {path}. Matching files are verified before import.',
    },
    waitingForCompletion: {
        id: 'app.downloads.missing-content.waiting-for-completion',
        defaultMessage: 'Waiting for a browser download to finish writing...',
    },
    downloadsUnavailable: {
        id: 'app.downloads.missing-content.downloads-unavailable',
        defaultMessage: 'The monitored folder is unavailable. Choose a local file instead.',
    },
    scannerFailed: {
        id: 'app.downloads.missing-content.scanner-failed',
        defaultMessage: 'Automatic checking failed. Manual file selection is still available.',
    },
    fileMismatch: {
        id: 'app.downloads.missing-content.file-mismatch',
        defaultMessage:
            'A similarly named file was found, but it did not match this required file.',
    },
    fileMismatchTitle: {
        id: 'app.downloads.missing-content.file-mismatch-title',
        defaultMessage: 'A same-named file was found, but file verification failed',
    },
    fileMismatchBody: {
        id: 'app.downloads.missing-content.file-mismatch-body',
        defaultMessage:
            'The launcher will keep waiting for the correct file. You can also choose a local file manually.',
    },
    importedAutomatically: {
        id: 'app.downloads.missing-content.imported-automatically',
        defaultMessage:
            '{count, plural, one {# file imported automatically} other {# files imported automatically}}. {remaining, plural, one {# file still needs to be completed} other {# files still need to be completed}}.',
    },
})

const statusMessages = defineMessages({
    failed: { id: 'app.downloads.item-status.failed', defaultMessage: 'Failed' },
    worker_started: { id: 'app.downloads.item-status.worker-started', defaultMessage: 'Starting' },
    connecting: { id: 'app.downloads.item-status.connecting', defaultMessage: 'Connecting' },
    verifying: { id: 'app.downloads.item-status.verifying', defaultMessage: 'Verifying' },
    writing: { id: 'app.downloads.item-status.writing', defaultMessage: 'Writing' },
    finalizing: { id: 'app.downloads.item-status.finalizing', defaultMessage: 'Finalizing' },
    metadata: { id: 'app.downloads.item-status.metadata', defaultMessage: 'Loading metadata' },
    waiting_for_database: {
        id: 'app.downloads.item-status.waiting-for-database',
        defaultMessage: 'Waiting for database',
    },
    completed: { id: 'app.downloads.item-status.completed', defaultMessage: 'Completed' },
    downloading: { id: 'app.downloads.item-status.downloading', defaultMessage: 'Downloading' },
    waiting_for_resource: {
        id: 'app.downloads.item-status.waiting-for-resource',
        defaultMessage: 'Waiting for download resources',
    },
    queued: { id: 'app.downloads.status.queued', defaultMessage: 'Queued' },
})

const scanner = createDownloadsScanLoop({
    scan: async () => {
        if (!jobId.value) throw new Error('Missing install job ID')
        return await install_job_scan_missing_files(jobId.value, scanDirectory.value)
    },
    onResult: (result) => {
        content.value = result.content
        scannerPresentation.value = reduceDownloadsScannerPresentation(scannerPresentation.value, {
            type: 'scan_result',
            downloadDirectory: result.downloadDirectory ?? null,
            importedItemIds: result.importedItemIds,
            rejectedItemIds: result.rejectedItemIds,
            pendingCandidates: result.pendingCandidates,
            hasErrors: result.errors.length > 0,
            items: result.content.files.map((file) => ({ id: file.itemId, status: file.status })),
        })
        scannerErrors.value = new Map(result.errors.map((error) => [error.itemId, error.message]))
        for (const itemId of result.importedItemIds) {
            scannerErrors.value.delete(itemId)
        }
        if (result.job.status !== 'waiting_for_user') {
            continuing.value = true
            stopScanning()
        }
    },
    onError: () => {
        scannerPresentation.value = reduceDownloadsScannerPresentation(scannerPresentation.value, {
            type: 'scan_failed',
        })
    },
    intervalMs: 3000,
})

async function show(job: InstallJobSnapshot) {
    stopScanning()
    const scannerSettings = getMissingContentScannerSettings()
    scannerEnabled.value = scannerSettings.enabled
    scanDirectory.value = scannerSettings.directory
    jobId.value = job.job_id
    continuing.value = false
    scannerPresentation.value = reduceDownloadsScannerPresentation(scannerPresentation.value, {
        type: 'reset',
    })
    scannerErrors.value = new Map()
    content.value = {
        remaining: job.items.filter((item) => item.status === 'failed').length,
        files: [],
    }
    modal.value?.show()
    await refresh()
    if (!unlisten) {
        unlisten = await install_job_listener((update: InstallJobSnapshot) => {
            if (update.job_id !== jobId.value) return
            if (update.status === 'waiting_for_user') {
                scannerPresentation.value = reduceDownloadsScannerPresentation(
                    scannerPresentation.value,
                    {
                        type: 'items_updated',
                        items: update.items,
                    },
                )
                void refresh()
            } else if (update.status === 'queued' || update.status === 'running') {
                continuing.value = true
                stopScanning()
            }
        })
    }
    if (scannerEnabled.value) scanner.start()
}

function stopScanning() {
    scanner.stop()
    scannerPresentation.value = reduceDownloadsScannerPresentation(scannerPresentation.value, {
        type: 'reset',
    })
}

async function refresh() {
    if (!jobId.value || continuing.value) return
    loading.value = true
    try {
        const nextContent = await install_job_missing_files(jobId.value)
        content.value = nextContent
        scannerPresentation.value = reduceDownloadsScannerPresentation(scannerPresentation.value, {
            type: 'items_updated',
            items: nextContent.files.map((file) => ({ id: file.itemId, status: file.status })),
        })
    } catch (error) {
        handleError(error)
    } finally {
        loading.value = false
    }
}

async function runItem(itemId: string, action: () => Promise<InstallJobSnapshot>) {
    busy.value = new Set([...busy.value, itemId])
    try {
        const job = await action()
        await applyItemResult(itemId, job)
    } catch (error) {
        const latest = jobId.value ? await install_job_get(jobId.value).catch(() => null) : null
        if (
            latest &&
            (isContinuingStatus(latest.status) ||
                latest.items.some(
                    (item) => item.id === itemId && ['completed', 'skipped'].includes(item.status),
                ))
        ) {
            await applyItemResult(itemId, latest)
        } else {
            handleError(error)
            await refresh()
        }
    } finally {
        const next = new Set(busy.value)
        next.delete(itemId)
        busy.value = next
    }
}

async function applyItemResult(itemId: string, job: InstallJobSnapshot) {
    if (
        job.status !== 'waiting_for_user' ||
        job.items.some(
            (item) => item.id === itemId && ['completed', 'skipped'].includes(item.status),
        )
    ) {
        scannerPresentation.value = reduceDownloadsScannerPresentation(scannerPresentation.value, {
            type: 'items_resolved',
            itemIds: [itemId],
        })
        scannerErrors.value.delete(itemId)
    }
    if (job.status === 'waiting_for_user') await refresh()
    else if (isContinuingStatus(job.status)) {
        continuing.value = true
        stopScanning()
    }
}

function isContinuingStatus(status: InstallJobSnapshot['status']) {
    return status === 'queued' || status === 'running' || status === 'succeeded'
}

async function retryOne(itemId: string) {
    if (!jobId.value) return
    await runItem(itemId, () => install_job_retry_missing_file(jobId.value!, itemId))
}

async function selectLocal(itemId: string) {
    if (!jobId.value) return
    const selected = await open({ multiple: false })
    const path = selectedPath(selected)
    if (!path) return
    await runItem(itemId, () => install_job_import_missing_file(jobId.value!, itemId, path))
}

function selectedPath(selected: unknown) {
    if (typeof selected === 'string') return selected
    if (
        selected &&
        typeof selected === 'object' &&
        'path' in selected &&
        typeof selected.path === 'string'
    ) {
        return selected.path
    }
    return null
}

async function retryAll() {
    if (!jobId.value) return
    loading.value = true
    stopScanning()
    try {
        await install_job_resume(jobId.value)
        continuing.value = true
    } catch (error) {
        const latest = await install_job_get(jobId.value).catch(() => null)
        if (latest && isContinuingStatus(latest.status)) {
            continuing.value = true
        } else {
            handleError(error)
            if (latest?.status === 'waiting_for_user' && scannerEnabled.value) scanner.start()
        }
    } finally {
        loading.value = false
    }
}

async function openBrowser(url: string) {
    try {
        const resolved = resolveDownloadUrl(url)
        if (!resolved) throw new Error(formatMessage(retiredDownloadMessage))
        await openUrl(resolved)
    } catch (error) {
        handleError(error)
    }
}

function isBusy(itemId: string) {
    return busy.value.has(itemId)
}

function scannerItemStatus(itemId: string) {
    if (scannerErrors.value.has(itemId)) return scannerErrors.value.get(itemId)
    if (scannerPresentation.value.rejectedItemIds.includes(itemId)) {
        return formatMessage(messages.fileMismatch)
    }
    return null
}

function attemptText(file: MissingFile) {
    if (file.attempt == null || file.maxAttempts == null) return formatMessage(messages.noAttempts)
    return formatMessage(messages.attempts, { attempt: file.attempt, max: file.maxAttempts })
}

function statusLabel(status: MissingFile['status']) {
    return status in statusMessages
        ? formatMessage(statusMessages[status as keyof typeof statusMessages])
        : status
}

function statusColor(status: MissingFile['status']): 'green' | 'red' | 'orange' | 'blue' | 'gray' {
    if (status === 'completed') return 'green'
    if (status === 'failed') return 'red'
    if (
        status === 'verifying' ||
        status === 'writing' ||
        status === 'finalizing' ||
        status === 'metadata' ||
        status === 'waiting_for_database'
    )
        return 'orange'
    return 'blue'
}

onUnmounted(() => {
    stopScanning()
    unlisten?.()
})

defineExpose({ show })
</script>
