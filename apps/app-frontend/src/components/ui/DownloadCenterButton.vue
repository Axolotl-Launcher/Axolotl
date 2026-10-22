<template>
    <PopoutMenu
        v-model:open="open"
        placement="bottom-end"
        :side-offset="0"
        dropdown-class="download-center-menu"
    >
        <template #trigger>
            <button
                type="button"
                :aria-label="formatMessage(messages.downloads)"
                :title="activeJobs[0]?.display?.title ?? formatMessage(messages.downloads)"
                :class="[
                    'inline-flex h-9 min-w-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-xl border border-solid border-transparent px-2.5 text-base font-semibold leading-5 text-[var(--color-text-primary)] transition-colors',
                    activeJobs.length || hasLegacyDownload
                        ? 'bg-surface-4 hover:bg-surface-5'
                        : 'w-9 shrink-0 !rounded-full bg-transparent hover:bg-surface-4',
                ]"
                :style="triggerStyle"
            >
                <LoaderCircleIcon v-if="hasActivity" class="size-5 shrink-0 animate-spin" />
                <DownloadIcon v-else class="size-5 shrink-0" />
                <template v-if="activeJobs.length || hasLegacyDownload">
                    <span class="max-w-[12rem] truncate text-sm font-semibold">
                        {{ activeJobs[0]?.display?.title ?? formatMessage(messages.downloads) }}
                    </span>
                    <span
                        v-if="firstJobSpeed"
                        class="min-w-0 truncate text-xs text-[var(--color-text-tertiary)]"
                        :title="`${formatBytes(firstJobSpeed)}/s`"
                    >
                        {{ formatBytes(firstJobSpeed) }}/s
                    </span>
                    <span
                        class="flex size-5 shrink-0 items-center justify-center rounded-full bg-green text-xs font-semibold text-white"
                    >
                        {{ Math.min(activeJobs.length || 1, 99) }}
                    </span>
                </template>
            </button>
        </template>
        <template #menu="{ hide }">
            <div class="w-[26rem] max-w-[calc(100vw-2rem)] p-2">
                <div v-if="hasActivity" class="mb-2 flex items-center gap-2 px-2">
                    <span class="text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.downloads) }}
                    </span>
                    <span
                        class="flex size-5 items-center justify-center rounded-full bg-green text-xs font-semibold text-white"
                    >
                        {{ activeJobs.length || 1 }}
                    </span>
                    <span class="text-sm text-[var(--color-text-tertiary)]">
                        {{ totalSpeedLabel }}
                    </span>
                </div>
                <div
                    v-if="downloadManager.initializationState.value === 'loading'"
                    class="flex flex-col gap-2 p-2"
                    aria-hidden="true"
                >
                    <div class="h-4 w-3/4 animate-pulse rounded bg-surface-4" />
                    <div class="h-2 w-full animate-pulse rounded bg-surface-4" />
                </div>
                <Button
                    v-else-if="downloadManager.initializationState.value === 'error'"
                    @click="retryInitialization"
                >
                    {{ formatMessage(commonMessages.retryButton) }}
                </Button>
                <div
                    v-else-if="activeJobs.length"
                    class="flex max-h-[24rem] flex-col gap-1 overflow-auto"
                >
                    <div
                        v-for="job in activeJobs"
                        :key="job.job_id"
                        class="rounded-lg p-2 hover:bg-surface-4"
                    >
                        <div class="flex items-center gap-2">
                            <span class="size-2 shrink-0 rounded-full bg-green" />
                            <button
                                class="min-w-0 flex-1 truncate text-left text-sm font-semibold text-[var(--color-text-primary)]"
                                @click="runPopoutAction(hide, () => openDetails(job.job_id))"
                            >
                                {{ job.display?.title ?? phaseLabel(job.phase) }}
                            </button>
                            <button
                                v-tooltip="
                                    formatMessage(
                                        job.kind === 'export_resource'
                                            ? exportMessages.cancelExport
                                            : messages.cancel,
                                    )
                                "
                                :disabled="cancelingJobs.has(job.job_id)"
                                :aria-label="
                                    formatMessage(
                                        job.kind === 'export_resource'
                                            ? exportMessages.cancelExport
                                            : messages.cancel,
                                    )
                                "
                                class="flex size-6 shrink-0 items-center justify-center rounded-full text-[var(--color-text-tertiary)] hover:bg-surface-5 hover:text-[var(--color-text-primary)]"
                                @click="void cancel(job.job_id)"
                            >
                                <XIcon class="size-4" />
                            </button>
                        </div>
                        <div
                            class="mt-1 flex items-center justify-between gap-2 text-xs text-[var(--color-text-tertiary)]"
                        >
                            <span class="truncate">{{ phaseLabel(job.phase) }}</span>
                            <span class="shrink-0">{{ progressLabel(job) }}</span>
                        </div>
                        <ProgressBar
                            class="mt-2"
                            full-width
                            color="green"
                            :progress="progressPercent(job) ?? 0"
                            :max="100"
                            :waiting="progressPercent(job) === null"
                            :aria-label="job.display?.title ?? phaseLabel(job.phase)"
                        />
                    </div>
                </div>
                <div
                    v-else-if="hasLegacyDownload"
                    class="px-2 py-4 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.legacyDownload) }}
                </div>
                <div v-else class="px-2 py-4 text-center text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.noActiveDownloads) }}
                </div>
                <button
                    v-if="!hasActivity"
                    class="mx-auto mt-1 block border-0 bg-transparent px-2 py-1 text-xs font-semibold text-brand hover:underline"
                    @click="runPopoutAction(hide, openHistory)"
                >
                    {{ formatMessage(messages.viewHistory) }}
                </button>
                <button
                    v-if="hasActivity"
                    class="mt-2 w-full rounded-lg px-2 py-1.5 text-center text-sm font-semibold text-brand hover:bg-surface-4"
                    @click="runPopoutAction(hide, () => openDetails())"
                >
                    {{ formatMessage(messages.viewDetails) }}
                </button>
            </div>
        </template>
    </PopoutMenu>
</template>

<script setup lang="ts">
import { DownloadIcon, LoaderCircleIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    PopoutMenu,
    ProgressBar,
    useFormatBytes,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import { exportMessages } from '@/components/ui/export/messages'
import { useDownloadTelemetry } from '@/composables/useDownloadTelemetry'
import type { InstallJobSnapshot, InstallPhaseId } from '@/helpers/install'
import {
    effectiveInstallProgress,
    hasDeterminateInstallProgress,
    installProgressFraction,
    installProgressTextSource,
} from '@/helpers/install-progress'
import { injectDownloadManager } from '@/providers/download-manager'

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const router = useRouter()
const downloadManager = injectDownloadManager()
const { handleError } = injectNotificationManager()
function retryInitialization() {
    void downloadManager.start().catch(handleError)
}
const cancelingJobs = ref(new Set<string>())
const open = ref(false)

const messages = defineMessages({
    downloads: { id: 'app.action-bar.downloads', defaultMessage: 'Downloads' },
    cancel: { id: 'app.action-bar.downloads.cancel', defaultMessage: 'Cancel download' },
    viewDetails: { id: 'app.action-bar.downloads.view-details', defaultMessage: 'View details' },
    legacyDownload: {
        id: 'app.action-bar.downloads.legacy',
        defaultMessage: 'A download is in progress.',
    },
    noActiveDownloads: {
        id: 'app.action-bar.downloads.empty',
        defaultMessage: 'No active downloads',
    },
    viewHistory: {
        id: 'app.action-bar.downloads.view-history',
        defaultMessage: 'View download history',
    },
    unknownPhase: { id: 'app.action-bar.downloads.phase.unknown', defaultMessage: 'Working' },
    preparingInstance: {
        id: 'app.action-bar.downloads.phase.preparing-instance',
        defaultMessage: 'Preparing instance',
    },
    resolvingPack: {
        id: 'app.action-bar.downloads.phase.resolving-pack',
        defaultMessage: 'Resolving modpack',
    },
    downloadingPack: {
        id: 'app.action-bar.downloads.phase.downloading-pack',
        defaultMessage: 'Downloading modpack',
    },
    downloadingContent: {
        id: 'app.action-bar.downloads.phase.downloading-content',
        defaultMessage: 'Downloading content',
    },
    downloadingMinecraft: {
        id: 'app.action-bar.downloads.phase.downloading-minecraft',
        defaultMessage: 'Downloading Minecraft',
    },
    preparingJava: {
        id: 'app.action-bar.downloads.phase.preparing-java',
        defaultMessage: 'Preparing Java',
    },
    verifying: { id: 'app.action-bar.downloads.phase.verifying', defaultMessage: 'Verifying' },
    finalizing: { id: 'app.action-bar.downloads.phase.finalizing', defaultMessage: 'Finalizing' },
})

const phaseDescriptors: Partial<Record<InstallPhaseId, (typeof messages)[keyof typeof messages]>> =
    {
        exporting: exportMessages.writing,
        preparing_instance: messages.preparingInstance,
        resolving_pack: messages.resolvingPack,
        downloading_pack_file: messages.downloadingPack,
        downloading_content: messages.downloadingContent,
        downloading_minecraft: messages.downloadingMinecraft,
        preparing_java: messages.preparingJava,
        verifying: messages.verifying,
        finalizing: messages.finalizing,
    }

const activeJobs = computed(() => downloadManager.activeJobs.value)
const displayedTelemetry = useDownloadTelemetry(activeJobs)
const firstJobSpeed = computed(
    () => displayedTelemetry(activeJobs.value[0])?.speed_bytes_per_second,
)
const hasLegacyDownload = computed(() => downloadManager.legacyDownloads.value.length > 0)
const hasActivity = computed(() => activeJobs.value.length > 0 || hasLegacyDownload.value)
const totalSpeedLabel = computed(() => {
    const speed = activeJobs.value.reduce(
        (total, job) => total + (displayedTelemetry(job)?.speed_bytes_per_second ?? 0),
        0,
    )
    return speed > 0 ? `${formatBytes(speed)}/s` : ''
})
const triggerStyle = computed(() => {
    if (!activeJobs.value.length) return undefined
    const percent = progressPercent(activeJobs.value[0])
    if (percent === null) return undefined
    return {
        backgroundImage: `linear-gradient(90deg, var(--color-brand-highlight) ${percent}%, var(--surface-4) ${percent}%)`,
    }
})

function phaseLabel(phase: InstallPhaseId) {
    return formatMessage(phaseDescriptors[phase] ?? messages.unknownPhase)
}

function progressPercent(job: InstallJobSnapshot) {
    if (job.status === 'queued') return null
    const fraction = installProgressFraction(job)
    return fraction === null ? null : fraction * 100
}

function progressLabel(job: InstallJobSnapshot) {
    const source = installProgressTextSource(job)
    if (source.type === 'bytes')
        return `${formatBytes(source.current)} / ${formatBytes(source.total)}`
    if (source.type === 'items')
        return `${Math.min(source.current, source.total)} / ${source.total}`
    if (source.type === 'required_files')
        return `${job.summary.files_completed} / ${job.summary.files_total ?? '—'}`
    const progress = effectiveInstallProgress(job)
    if (hasDeterminateInstallProgress(progress))
        return `${Math.min(progress.current, progress.total)} / ${progress.total}`
    return ''
}

function openDetails(jobId?: string) {
    router.push(jobId ? { path: '/downloads', query: { job: jobId } } : '/downloads')
}

function openHistory() {
    router.push({ path: '/downloads', query: { tab: 'history' } })
}

function runPopoutAction(hide: () => void, action: () => void) {
    hide()
    action()
}

async function cancel(jobId: string) {
    if (cancelingJobs.value.has(jobId)) return
    cancelingJobs.value.add(jobId)
    try {
        await downloadManager.cancel(jobId)
    } catch (error) {
        handleError(error)
    } finally {
        cancelingJobs.value.delete(jobId)
    }
}
</script>

<style>
.download-center-menu[data-state='open'] {
    animation: download-center-menu-in 140ms ease-out;
}

.download-center-menu .menu-arrow {
    transform: translateY(1px);
}

.download-center-menu[data-state='closed'] {
    animation: download-center-menu-out 100ms ease-in;
}

@keyframes download-center-menu-in {
    from {
        opacity: 0;
        transform: translateY(-0.25rem) scale(0.97);
    }
    to {
        opacity: 1;
        transform: translateY(0) scale(1);
    }
}

@keyframes download-center-menu-out {
    from {
        opacity: 1;
        transform: translateY(0) scale(1);
    }
    to {
        opacity: 0;
        transform: translateY(-0.2rem) scale(0.98);
    }
}
</style>
