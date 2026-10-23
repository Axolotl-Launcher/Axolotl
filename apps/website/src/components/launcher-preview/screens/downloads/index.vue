<script setup lang="ts">
import ModrinthIcon from '@modrinth/assets/branding/logo.svg?component'
import CurseForgeIcon from '@modrinth/assets/external/curseforge.svg?component'
import CheckCircleIcon from '@modrinth/assets/icons/check-circle.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import ClockIcon from '@modrinth/assets/icons/clock.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import InfoIcon from '@modrinth/assets/icons/info.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Badge from '@modrinth/ui/src/components/base/Badge.vue'
import BulletDivider from '@modrinth/ui/src/components/base/BulletDivider.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Card from '@modrinth/ui/src/components/base/Card.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import type { TableColumn } from '@modrinth/ui/src/components/base/Table.vue'
import Table from '@modrinth/ui/src/components/base/Table.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import PreviewConfirmModal from '../../overlays/PreviewConfirmModal.vue'
import PreviewModal from '../../overlays/PreviewModal.vue'
import {
    createDownloadFocusState,
    focusedDownloadJobId,
    reconcileDownloadFocus,
    stopDownloadFocusAutoFollow,
} from './download-focus'
import {
    effectiveInstallProgress,
    hasDeterminateInstallProgress,
    installProgressFraction,
    installProgressTextSource,
} from './install-progress'
import { messages, phaseMessages, statusMessages } from './messages'
import { type DownloadItem, type DownloadJob, type JobProvider, useDownloadJobs } from './state'

/**
 * Port of `pages/Downloads.vue`. The download manager's jobs are the shared
 * content-install queue and the library's duplicate/repair jobs; legacy
 * loading bars, CurseForge manual downloads and content-change jobs do not
 * occur in the preview, so their branches are left out.
 */
const { preview, domain, localJobs, jobs, activeJobs, historyJobs } = useDownloadJobs()
const instanceNames = computed(
    () =>
        new Map(preview.instances.instances.value.map((instance) => [instance.id, instance.name])),
)
const fileDetailsModal = ref<InstanceType<typeof PreviewModal>>()
const selectedFileKey = ref<{ jobId: string; itemId: string } | null>(null)
const selectedFile = computed(() => {
    const selected = selectedFileKey.value
    return selected
        ? jobs.value
              .find((job) => job.job_id === selected.jobId)
              ?.items.find((item) => item.id === selected.itemId)
        : undefined
})
const selectedFileFields = computed(() => {
    const item = selectedFile.value
    if (!item) return []
    return [
        { label: formatMessage(messages.projectId), value: item.project_id },
        { label: formatMessage(messages.versionId), value: item.version_id },
        { label: formatMessage(messages.requestUrl), value: item.request_url },
    ].filter((field) => !!field.value)
})
function showFileDetails(job: DownloadJob, item: DownloadItem) {
    selectedFileKey.value = { jobId: job.job_id, itemId: item.id }
    fileDetailsModal.value?.show()
}
async function copyFileDetails() {
    const item = selectedFile.value
    if (!item) return
    await navigator.clipboard
        ?.writeText(
            [
                item.name,
                itemError(item),
                ...selectedFileFields.value.map((field) => `${field.label}: ${field.value}`),
            ]
                .filter(Boolean)
                .join('\n'),
        )
        .catch(() => undefined)
}
function targetInstanceName(job: DownloadJob) {
    return (
        (job.instance_id ? instanceNames.value.get(job.instance_id) : null) ??
        (job.details.type === 'instance' ? job.details.name : null)
    )
}
const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const tab = ref<'active' | 'history'>(route.query.tab === 'history' ? 'history' : 'active')
const query = ref('')
const provider = ref('all')
const historyStatus = ref('all')
const expanded = ref(new Set<string>())
const busy = ref(new Set<string>())
const clearHistoryModal = ref<InstanceType<typeof PreviewConfirmModal>>()
const focusedJobId = computed(() => focusedDownloadJobId(route.query.job))
const focusState = ref(createDownloadFocusState(focusedJobId.value))

watch(
    () => route.query.tab,
    (value) => {
        tab.value = value === 'history' ? 'history' : 'active'
    },
)

/** `useDownloadTelemetry`: speed and ETA are sampled once a second. */
type Telemetry = { speed_bytes_per_second: number | null; eta_seconds: number | null }
function sampleTelemetry() {
    return new Map<string, Telemetry>(
        activeJobs.value
            .filter((job) => job.status === 'running')
            .map((job) => [
                job.job_id,
                {
                    speed_bytes_per_second: job.summary.speed_bytes_per_second,
                    eta_seconds: job.summary.eta_seconds,
                },
            ]),
    )
}
const telemetry = shallowRef(sampleTelemetry())
let telemetryElapsed = 0
let stopTelemetry: (() => void) | null = null
// The app samples with a one-second interval; the preview's clock only ticks while a job runs.
watch(
    () => activeJobs.value.some((job) => job.status === 'running'),
    (running) => {
        stopTelemetry?.()
        stopTelemetry = null
        telemetry.value = sampleTelemetry()
        if (!running) return
        telemetryElapsed = 0
        stopTelemetry = preview.clock.every((delta) => {
            telemetryElapsed += delta
            if (telemetryElapsed < 1000) return
            telemetryElapsed = 0
            telemetry.value = sampleTelemetry()
        })
    },
    { immediate: true },
)
onScopeDispose(() => stopTelemetry?.())

function displayedTelemetry(job: DownloadJob): Telemetry | undefined {
    if (job.status !== 'running') return undefined
    const sampled = telemetry.value.get(job.job_id)
    if (!sampled) return undefined
    return {
        speed_bytes_per_second:
            job.summary.speed_bytes_per_second == null ? null : sampled.speed_bytes_per_second,
        eta_seconds: job.summary.eta_seconds == null ? null : sampled.eta_seconds,
    }
}

const providerOptions = [
    'all',
    'modrinth',
    'curse_forge',
    'minecraft',
    'java',
    'application',
    'local',
]
const historyStatusOptions = ['all', 'succeeded', 'failed', 'interrupted', 'canceled']
const downloadTabs = computed(() => [
    {
        href: 'active',
        label: formatMessage(messages.inProgress),
        icon: DownloadIcon,
    },
    { href: 'history', label: formatMessage(messages.history), icon: ClockIcon },
])
const itemColumns = computed<TableColumn[]>(() => [
    { key: 'name', label: formatMessage(messages.itemName), width: '48%' },
    { key: 'status', label: formatMessage(messages.itemStatus), width: '18%' },
    { key: 'attempts', label: formatMessage(messages.itemAttempts), width: '16%' },
    { key: 'progress', label: formatMessage(messages.itemProgress), width: '18%', align: 'right' },
])
const sourceJobs = computed(() => (tab.value === 'active' ? activeJobs.value : historyJobs.value))
const visibleJobs = computed(() => {
    const normalized = query.value.trim().toLowerCase()
    return sourceJobs.value.filter((job) => {
        if (provider.value !== 'all' && job.provider !== provider.value) return false
        if (
            tab.value === 'history' &&
            historyStatus.value !== 'all' &&
            job.status !== historyStatus.value
        )
            return false
        return (
            !normalized ||
            jobTitle(job).toLowerCase().includes(normalized) ||
            targetInstanceName(job)?.toLowerCase().includes(normalized) ||
            job.job_id.includes(normalized)
        )
    })
})

function jobTitle(job: DownloadJob) {
    return (
        job.display?.title ||
        (job.details.type === 'instance' ? job.details.name : null) ||
        jobTypeLabel(job)
    )
}

function providerLabel(value: JobProvider) {
    return {
        modrinth: 'Modrinth',
        curse_forge: 'CurseForge',
        minecraft: 'Minecraft',
        java: 'Java',
        application: formatMessage(messages.application),
        local: formatMessage(messages.local),
    }[value]
}

function providerFilterLabel(value: string) {
    return value === 'all'
        ? formatMessage(messages.allSources)
        : providerLabel(value as JobProvider)
}

function providerIcon(value: JobProvider) {
    return value === 'curse_forge'
        ? CurseForgeIcon
        : value === 'modrinth'
          ? ModrinthIcon
          : DownloadIcon
}

function jobTypeLabel(job: DownloadJob) {
    return providerLabel(job.provider)
}

function jobTypeIcon(job: DownloadJob) {
    return providerIcon(job.provider)
}

function historyStatusLabel(value: string) {
    return value === 'all' ? formatMessage(messages.allStatuses) : statusLabel(value)
}

function statusLabel(status: string) {
    return status in statusMessages
        ? formatMessage(statusMessages[status as keyof typeof statusMessages])
        : status
}

function phaseLabel(phase: string) {
    const message = (
        phaseMessages as Partial<Record<string, { id: string; defaultMessage: string }>>
    )[phase]
    return formatMessage(message ?? messages.unknownPhase)
}

function jobPhaseLabel(job: DownloadJob) {
    if (job.rollback_error) return formatMessage(messages.cleanupIncomplete)
    if (job.kind === 'install_content' && job.phase === 'preparing_instance') {
        return formatMessage(phaseMessages.downloading_content)
    }
    return phaseLabel(job.phase)
}

function statusColor(status: string): 'green' | 'red' | 'orange' | 'blue' | 'gray' {
    if (status === 'succeeded') return 'green'
    if (status === 'failed' || status === 'interrupted' || status === 'canceled') return 'red'
    if (status === 'running' || status === 'waiting_for_user' || status === 'canceling')
        return 'orange'
    return 'blue'
}

function itemStatusColor(
    status: DownloadItem['status'],
): 'green' | 'red' | 'orange' | 'blue' | 'gray' {
    if (status === 'completed') return 'green'
    if (status === 'failed' || status === 'canceled') return 'red'
    if (status === 'skipped') return 'gray'
    return 'blue'
}

function canCancel(job: DownloadJob) {
    return ['queued', 'running', 'waiting_for_user'].includes(job.status)
}

function canRetry(job: DownloadJob) {
    return job.status === 'failed' || job.status === 'interrupted' || job.status === 'canceled'
}

function showProgress(job: DownloadJob) {
    return ['queued', 'running', 'canceling', 'waiting_for_user'].includes(job.status)
}

function jobPercent(job: DownloadJob) {
    if (isMainTrackComplete(job)) return 100
    return Math.min(99, (installProgressFraction(job) ?? 0) * 100)
}

function hasDeterminateProgress(job: DownloadJob) {
    return installProgressFraction(job) !== null
}

function isFinished(job: DownloadJob) {
    return job.status === 'succeeded'
}

function isMainTrackComplete(job: DownloadJob) {
    if (isFinished(job)) return true
    const progress = effectiveInstallProgress(job)
    if (job.phase === 'downloading_content' && progress?.total === 0) return true
    return hasDeterminateInstallProgress(progress) && progress.current >= progress.total
}

function progressColor(job: DownloadJob) {
    return isMainTrackComplete(job) ? 'green' : 'brand'
}

function progressText(job: DownloadJob) {
    if (isMainTrackComplete(job)) return statusLabel('completed')
    const textSource = installProgressTextSource(job)
    if (textSource.type === 'bytes')
        return `${formatBytes(textSource.current)} / ${formatBytes(textSource.total)}`
    if (textSource.type === 'items') return `${textSource.current} / ${textSource.total}`
    return phaseLabel(job.phase)
}

function downloadTelemetry(job: DownloadJob) {
    const summary = job.summary
    const sampled = displayedTelemetry(job)
    const metrics: string[] = []
    if (summary.source) {
        metrics.push(
            formatMessage(messages.downloadSource, {
                source: downloadSourceLabel(summary.source),
            }),
        )
    }
    if (sampled?.speed_bytes_per_second && sampled.speed_bytes_per_second > 0) {
        metrics.push(
            formatMessage(messages.downloadSpeed, {
                speed: formatBytes(sampled.speed_bytes_per_second),
            }),
        )
    }
    if (sampled?.eta_seconds != null) {
        metrics.push(formatDownloadEta(sampled.eta_seconds))
    }
    return metrics
}

function downloadSourceLabel(source: string) {
    return source === 'official'
        ? formatMessage(messages.downloadSourceOfficial)
        : formatMessage(messages.downloadSourceAlternate)
}

const STATUS_ORDER: Record<string, number> = {
    failed: 0,
    skipped: 1,
    waiting_for_user: 2,
    downloading: 3,
    verifying: 3,
    writing: 3,
    queued: 4,
    canceled: 5,
    completed: 6,
}
function reorderJobItems(job: DownloadJob): DownloadItem[] {
    return [...job.items].sort(
        (a, b) => (STATUS_ORDER[a.status] ?? 9) - (STATUS_ORDER[b.status] ?? 9),
    )
}

function formatDownloadEta(seconds: number) {
    const clamped = Math.max(0, Math.round(seconds))
    if (clamped < 60) {
        return formatMessage(messages.downloadEtaSeconds, { seconds: clamped })
    }
    const minutes = Math.floor(clamped / 60)
    if (minutes < 60) {
        return formatMessage(messages.downloadEtaMinutes, { minutes })
    }
    return formatMessage(messages.downloadEtaHours, {
        hours: Math.floor(minutes / 60),
        minutes: minutes % 60,
    })
}

/** The preview's items never need a manual CurseForge download, so their error shows as is. */
function itemError(item: DownloadItem) {
    return item.error ?? ''
}

function itemProgress(item: DownloadItem) {
    if (!item.bytes_total) return formatMessage(messages.notAvailable)
    return `${formatBytes(item.bytes_downloaded)} / ${formatBytes(item.bytes_total)}`
}

function itemAttempts(item: DownloadItem) {
    if (item.attempt == null || item.max_attempts == null)
        return formatMessage(messages.notAvailable)
    return formatMessage(messages.attemptProgress, {
        attempt: item.attempt,
        maxAttempts: item.max_attempts,
    })
}

function formatDate(value: string) {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
        new Date(value),
    )
}

function selectTab(index: number) {
    focusState.value = stopDownloadFocusAutoFollow(focusState.value)
    tab.value = index === 0 ? 'active' : 'history'
}

function focusedJobElement(jobId: string): HTMLElement | null {
    return (
        [...document.querySelectorAll<HTMLElement>('[data-install-job-id]')].find(
            (element) => element.dataset.installJobId === jobId,
        ) ?? null
    )
}

watch(
    [focusedJobId, jobs],
    async ([jobId, list]) => {
        if (focusState.value.jobId !== jobId) {
            focusState.value = createDownloadFocusState(jobId)
        }
        const effect = reconcileDownloadFocus(
            focusState.value,
            jobId ? (list.find((job) => job.job_id === jobId) ?? null) : null,
        )
        focusState.value = effect.state
        if (effect.tab) tab.value = effect.tab
        if (effect.expand && jobId) {
            expanded.value = new Set([...expanded.value, jobId])
        }
        if (effect.scroll && jobId) {
            await nextTick()
            focusedJobElement(jobId)?.scrollIntoView({ behavior: 'smooth', block: 'center' })
        }
    },
    { immediate: true },
)

function toggleExpanded(jobId: string) {
    const next = new Set(expanded.value)
    if (next.has(jobId)) {
        next.delete(jobId)
    } else {
        next.add(jobId)
    }
    expanded.value = next
}

async function withBusy(jobId: string, action: () => Promise<void> | void) {
    busy.value = new Set([...busy.value, jobId])
    try {
        await action()
    } finally {
        const next = new Set(busy.value)
        next.delete(jobId)
        busy.value = next
    }
}

const CANCEL_SETTLE_MS = 350

/** `download_job_cancel`: the job reports `canceling` until its rollback has finished. */
function cancelContentJob(jobId: string) {
    const job = preview.downloads.jobs.value.find((entry) => entry.id === jobId)
    if (!job || domain.canceling.value.has(jobId)) return
    const snapshot = { ...job, speed: 0 }
    domain.canceling.value = new Map(domain.canceling.value).set(jobId, snapshot)
    preview.downloads.cancel(jobId)
    return new Promise<void>((resolve) =>
        preview.clock.after(CANCEL_SETTLE_MS, () => {
            const next = new Map(domain.canceling.value)
            next.delete(jobId)
            domain.canceling.value = next
            resolve()
        }),
    )
}

async function cancel(job: DownloadJob) {
    await withBusy(job.job_id, () =>
        job.kind === 'install_content'
            ? cancelContentJob(job.job_id)
            : localJobs.cancel(job.job_id),
    )
}

/** `retry_job_as_new`: a new job for the same request; the old record stays in history. */
function retryContentJob(jobId: string) {
    const history = preview.downloads.history.value
    const index = history.findIndex((entry) => entry.id === jobId)
    const record = history[index]
    if (!record) return
    const before = new Set(preview.downloads.jobs.value.map((entry) => entry.id))
    preview.downloads.retry(jobId)
    const restored = [...preview.downloads.history.value]
    restored.splice(index, 0, record)
    preview.downloads.history.value = restored
    const started = preview.downloads.jobs.value.find((entry) => !before.has(entry.id))
    if (started) {
        domain.createdAt.value = new Map(domain.createdAt.value).set(
            started.id,
            new Date().toISOString(),
        )
    }
}

async function retry(job: DownloadJob) {
    await withBusy(job.job_id, () =>
        job.kind === 'install_content' ? retryContentJob(job.job_id) : localJobs.retry(job.job_id),
    )
}

async function remove(job: DownloadJob) {
    await withBusy(job.job_id, () => {
        if (job.kind === 'install_content') {
            preview.downloads.history.value = preview.downloads.history.value.filter(
                (entry) => entry.id !== job.job_id,
            )
        } else {
            localJobs.remove(job.job_id)
        }
    })
}

/** `download_job_support_details`: the backend's plain-text install report. */
function supportDetails(job: DownloadJob) {
    const result =
        job.status === 'running'
            ? `running while ${job.phase.replace(/_/g, ' ')}`
            : job.status === 'failed'
              ? 'failed while downloading content (network_error)'
              : job.status === 'canceled'
                ? 'canceled while downloading content'
                : job.status.replace(/_/g, ' ')
    const lines = [
        `Install report: ${jobTitle(job)}`,
        `Result: ${result}`,
        `Job ID: ${job.job_id}`,
        `Request: "${job.kind}"`,
        `Status: "${job.status}"`,
        `Current phase: ${job.phase.replace(/_/g, ' ')}`,
    ]
    if (job.error) {
        lines.push(
            '',
            'Failure',
            `Code: ${job.status === 'canceled' ? 'canceled' : 'network_error'}`,
            'Phase: downloading content',
            `Message: ${job.error.message}`,
        )
    }
    lines.push('', 'Raw snapshot', JSON.stringify(job, null, 2))
    return lines.join('\n')
}

async function copyDiagnostics(job: DownloadJob) {
    await withBusy(job.job_id, async () => {
        await navigator.clipboard?.writeText(supportDetails(job)).catch(() => undefined)
    })
}

function clearHistory() {
    const canceling = domain.canceling.value
    preview.downloads.history.value = preview.downloads.history.value.filter((job) =>
        canceling.has(job.id),
    )
    localJobs.clearHistory()
}
</script>

<template>
    <div class="box-border flex min-h-full w-full flex-col gap-3 p-6">
        <div class="flex flex-wrap items-center justify-between gap-3">
            <div data-onboarding-id="downloads-tabs">
                <NavTabs
                    :active-index="tab === 'active' ? 0 : 1"
                    :links="downloadTabs"
                    mode="local"
                    @tab-click="selectTab"
                />
            </div>
            <Button type="colored" color="brand" @click="router.push('/create')"
                ><PlusIcon />
                {{ formatMessage(messages.newDownload) }}
            </Button>
        </div>

        <div class="flex flex-wrap items-center gap-2">
            <StyledInput
                v-model="query"
                :icon="SearchIcon"
                :placeholder="formatMessage(messages.search)"
                clearable
                wrapper-class="flex-1 min-w-0"
            />
            <Combobox
                v-model="provider"
                width="custom"
                class="w-44"
                :options="
                    providerOptions.map((value) => ({ value, label: providerFilterLabel(value) }))
                "
            />
            <Combobox
                v-if="tab === 'history'"
                v-model="historyStatus"
                width="custom"
                class="w-44"
                :options="
                    historyStatusOptions.map((value) => ({
                        value,
                        label: historyStatusLabel(value),
                    }))
                "
            />
            <Button
                v-if="tab === 'history' && historyJobs.length"
                class="ml-auto"
                type="outlined"
                @click="clearHistoryModal?.show()"
                ><TrashIcon />
                {{ formatMessage(messages.clearHistory) }}
            </Button>
        </div>

        <div
            v-if="domain.initializationState.value === 'loading'"
            class="flex flex-col gap-3"
            aria-hidden="true"
        >
            <Card v-for="index in 2" :key="index" class="!p-4">
                <div class="flex items-center gap-3">
                    <div class="size-12 shrink-0 animate-pulse rounded-xl bg-surface-4" />
                    <div class="flex flex-1 flex-col gap-2">
                        <div class="h-4 w-1/2 animate-pulse rounded bg-surface-4" />
                        <div class="h-3 w-1/3 animate-pulse rounded bg-surface-4" />
                    </div>
                </div>
                <div class="mt-4 h-2 w-full animate-pulse rounded bg-surface-4" />
            </Card>
        </div>
        <div
            v-else-if="visibleJobs.length"
            role="tabpanel"
            :aria-labelledby="'nav-tab-' + tab"
            class="flex flex-col gap-3"
        >
            <Card
                v-for="job in visibleJobs"
                :key="job.job_id"
                :data-install-job-id="job.job_id"
                class="!p-0"
            >
                <div class="flex flex-wrap items-center gap-4 p-4">
                    <img
                        v-if="job.display?.icon"
                        :src="job.display.icon"
                        alt=""
                        class="size-12 rounded-xl object-cover"
                        loading="lazy"
                        decoding="async"
                    />
                    <div
                        v-else
                        class="flex size-12 items-center justify-center rounded-xl bg-brand-highlight text-brand"
                    >
                        <DownloadIcon />
                    </div>
                    <div class="min-w-48 flex-grow">
                        <div class="flex flex-wrap items-center gap-2">
                            <h2
                                class="m-0 truncate text-lg font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ jobTitle(job) }}
                            </h2>
                            <TagItem>
                                <component :is="jobTypeIcon(job)" />
                                {{ jobTypeLabel(job) }}
                            </TagItem>
                            <Badge
                                :color="statusColor(job.status)"
                                :type="statusLabel(job.status)"
                            />
                            <Badge
                                v-if="job.instance_deleted"
                                color="gray"
                                :type="formatMessage(messages.instanceDeleted)"
                            />
                        </div>
                        <div
                            class="mt-1 flex flex-wrap items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                        >
                            <span>{{ jobPhaseLabel(job) }}</span>
                            <BulletDivider />
                            <span>{{ formatDate(job.finished ?? job.modified) }}</span>
                            <template v-if="targetInstanceName(job)">
                                <BulletDivider />
                                <span>{{
                                    formatMessage(messages.instanceTarget, {
                                        instance: targetInstanceName(job),
                                    })
                                }}</span>
                            </template>
                        </div>
                        <div
                            v-if="downloadTelemetry(job).length"
                            class="mt-1 flex flex-wrap items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                        >
                            <template
                                v-for="(metric, index) in downloadTelemetry(job)"
                                :key="`${index}-${metric}`"
                            >
                                <BulletDivider v-if="index > 0" />
                                <span>{{ metric }}</span>
                            </template>
                        </div>
                    </div>
                    <div class="flex flex-wrap items-center gap-2">
                        <Button
                            v-if="canCancel(job)"
                            type="outlined"
                            color="red"
                            size="2xs"
                            :disabled="busy.has(job.job_id)"
                            @click="cancel(job)"
                            ><XIcon />{{ formatMessage(messages.cancel) }}
                        </Button>
                        <Button
                            v-if="canRetry(job)"
                            type="colored"
                            color="brand"
                            size="2xs"
                            :disabled="busy.has(job.job_id)"
                            @click="retry(job)"
                            ><RefreshCwIcon />{{ formatMessage(messages.retry) }}
                        </Button>
                        <Button
                            v-if="job.error"
                            type="outlined"
                            size="2xs"
                            :disabled="busy.has(job.job_id)"
                            @click="copyDiagnostics(job)"
                            ><ClipboardCopyIcon />{{ formatMessage(messages.copyDiagnostics) }}
                        </Button>
                        <Button
                            v-if="
                                job.instance_id &&
                                !job.instance_deleted &&
                                job.status !== 'waiting_for_user'
                            "
                            type="outlined"
                            size="2xs"
                            @click="
                                router.push(`/instance/${encodeURIComponent(job.instance_id!)}`)
                            "
                            ><ExternalIcon />{{ formatMessage(messages.openInstance) }}
                        </Button>
                        <Button
                            v-if="
                                job.items.length > 0 ||
                                job.error ||
                                job.rollback_error ||
                                job.instance_id
                            "
                            type="quiet"
                            size="2xs"
                            @click="toggleExpanded(job.job_id)"
                            ><ChevronDownIcon
                                :class="expanded.has(job.job_id) ? 'rotate-180' : ''"
                            />
                            {{
                                expanded.has(job.job_id)
                                    ? formatMessage(messages.hideDetails)
                                    : formatMessage(messages.details)
                            }}
                        </Button>
                        <Button
                            v-if="tab === 'history'"
                            v-tooltip="formatMessage(messages.deleteRecord)"
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            :label="formatMessage(messages.deleteRecord)"
                            :disabled="busy.has(job.job_id)"
                            @click="remove(job)"
                            ><TrashIcon />
                        </Button>
                    </div>
                </div>

                <div v-if="showProgress(job)" class="px-4 pb-4">
                    <ProgressBar
                        full-width
                        :progress="jobPercent(job)"
                        :max="100"
                        :color="progressColor(job)"
                        :label="progressText(job)"
                        :waiting="
                            !isFinished(job) &&
                            (job.status === 'queued' || !hasDeterminateProgress(job))
                        "
                        :show-progress="isFinished(job) || hasDeterminateProgress(job)"
                    >
                        <template #progress-icon>
                            <CheckCircleIcon
                                v-if="isMainTrackComplete(job)"
                                class="size-5 text-green"
                                aria-hidden="true"
                            />
                            <SpinnerIcon v-else class="size-5 animate-spin" aria-hidden="true" />
                        </template>
                    </ProgressBar>
                </div>

                <div
                    v-if="expanded.has(job.job_id)"
                    class="border-0 border-t border-solid border-divider p-4"
                >
                    <dl
                        class="m-0 mb-4 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 text-sm"
                    >
                        <dt class="text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.jobId) }}
                        </dt>
                        <dd class="m-0 break-all font-mono">{{ job.job_id }}</dd>
                        <template v-if="job.instance_id">
                            <dt class="text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.instanceId) }}
                            </dt>
                            <dd class="m-0 break-all font-mono">{{ job.instance_id }}</dd>
                        </template>
                    </dl>
                    <Admonition
                        v-if="job.rollback_error || job.error"
                        class="mb-4"
                        type="critical"
                        :header="
                            formatMessage(
                                job.rollback_error
                                    ? messages.cleanupIncomplete
                                    : messages.errorDetails,
                            )
                        "
                    >
                        {{ job.rollback_error?.message ?? job.error?.message }}
                    </Admonition>
                    <Table
                        v-if="job.items.length"
                        :key="`${job.job_id}-details`"
                        :columns="itemColumns"
                        :data="reorderJobItems(job)"
                        row-key="id"
                        table-min-width="42rem"
                        virtualized
                        class="max-h-80 overflow-y-auto"
                    >
                        <template #cell-name="{ row }">
                            <div class="flex min-w-0 items-center gap-2 py-0.5">
                                <div class="flex min-w-0 flex-1 items-center gap-2">
                                    <span
                                        class="min-w-0 flex-1 truncate font-medium text-[var(--color-text-primary)]"
                                        >{{ row.name }}</span
                                    >
                                    <Button
                                        type="quiet"
                                        size="2xs"
                                        circular
                                        icon-only
                                        :label="
                                            formatMessage(messages.openFileDetails, {
                                                name: row.name,
                                            })
                                        "
                                        @click.stop="showFileDetails(job, row)"
                                        ><InfoIcon
                                    /></Button>
                                    <span
                                        v-if="row.source"
                                        class="max-w-24 shrink-0 truncate text-xs text-[var(--color-text-tertiary)]"
                                    >
                                        {{ downloadSourceLabel(row.source) }}
                                    </span>
                                </div>
                            </div>
                        </template>
                        <template #cell-status="{ row }">
                            <Badge
                                :color="itemStatusColor(row.status)"
                                :type="statusLabel(row.status)"
                            />
                        </template>
                        <template #cell-attempts="{ row }">
                            <span>{{ itemAttempts(row) }}</span>
                        </template>
                        <template #cell-progress="{ row }">
                            <span>{{ itemProgress(row) }}</span>
                        </template>
                    </Table>
                    <EmptyState
                        v-else
                        type="no-documents"
                        :heading="formatMessage(messages.noFileDetailsTitle)"
                        :description="formatMessage(messages.noFileDetails)"
                    />
                </div>
            </Card>
        </div>

        <Card v-else role="tabpanel" :aria-labelledby="'nav-tab-' + tab" class="flex flex-1">
            <EmptyState
                class="my-auto"
                :type="query ? 'no-search-result' : 'no-tasks'"
                :heading="formatMessage(query ? messages.noResultsTitle : messages.emptyTitle)"
                :description="
                    formatMessage(query ? messages.noResultsDescription : messages.emptyDescription)
                "
            />
        </Card>
    </div>

    <PreviewConfirmModal
        ref="clearHistoryModal"
        :danger="true"
        :title="formatMessage(messages.clearHistoryTitle)"
        :description="formatMessage(messages.confirmClear)"
        :proceed-label="formatMessage(messages.clearHistory)"
        @proceed="clearHistory"
    />
    <PreviewModal
        ref="fileDetailsModal"
        :header="formatMessage(messages.fileDetails)"
        max-width="42rem"
        scrollable
    >
        <div v-if="selectedFile" class="flex min-w-0 flex-col gap-4">
            <h3
                class="m-0 break-words text-lg font-semibold text-[var(--color-text-primary)] [overflow-wrap:anywhere]"
            >
                {{ selectedFile.name }}
            </h3>
            <Admonition
                v-if="selectedFile.error"
                type="critical"
                :header="formatMessage(messages.errorDetails)"
            >
                <p class="m-0 whitespace-pre-wrap break-words [overflow-wrap:anywhere]">
                    {{ itemError(selectedFile) }}
                </p>
            </Admonition>
            <dl class="m-0 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 text-sm">
                <template v-for="field in selectedFileFields" :key="field.label">
                    <dt class="text-[var(--color-text-tertiary)]">{{ field.label }}</dt>
                    <dd class="m-0 break-all font-mono">{{ field.value }}</dd>
                </template>
            </dl>
            <Button type="outlined" class="self-start" @click="copyFileDetails"
                ><ClipboardCopyIcon />{{ formatMessage(messages.copyFileDetails) }}</Button
            >
        </div>
    </PreviewModal>
</template>
