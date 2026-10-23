<script setup lang="ts">
import BellIcon from '@modrinth/assets/icons/bell.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import LoaderCircleIcon from '@modrinth/assets/icons/loader-circle.svg?component'
import NewspaperIcon from '@modrinth/assets/icons/newspaper.svg?component'
import OnlineIndicatorIcon from '@modrinth/assets/icons/online-indicator.svg?component'
import StarIcon from '@modrinth/assets/icons/star.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import TerminalSquareIcon from '@modrinth/assets/icons/terminal-square.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type {
    PopupNotification,
    PopupNotificationButton,
} from '@modrinth/ui/src/providers/popup-notifications.ts'
import type { WebNotification } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, type Ref, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import { actionBarMessages as messages } from '../lib/messages'
import { usePreview } from '../lib/state'
import type { InstallJob } from '../lib/state/downloads'
import { injectPreviewWindow } from '../lib/window'
import AnnouncementCenter from './AnnouncementCenter.vue'

/**
 * Port of the app's AppActionBar with its DownloadCenterButton: notification
 * history, announcements, the download center and the running-instance pill.
 */
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const router = useRouter()
const { layer } = injectPreviewWindow()
const state = usePreview()
const { downloads, processes, notifications, popups } = state

type NotificationHistoryItem = {
    key: string
    createdAt?: number
    title: string
    text?: string
    type?: 'error' | 'warning' | 'success' | 'info' | 'download'
    read?: boolean
    onClick?: () => void | Promise<void>
    primaryAction?: PopupNotificationButton
    markRead: () => void
    expand: () => void
    dismiss: () => void
}

const notificationCenterShown = ref(false)
const showInstances = ref(false)
const downloadsOpen = ref(false)
const announcementCenter = ref<InstanceType<typeof AnnouncementCenter>>()

const notificationHistory = computed<NotificationHistoryItem[]>(() =>
    [
        ...notifications.getNotifications().map((item: WebNotification) => ({
            key: `web-${item.id}`,
            createdAt: item.createdAt,
            title: item.title ?? formatMessage(messages.notifications),
            text: item.text,
            type: item.type,
            read: item.read,
            markRead: () => notifications.markNotificationRead(item.id),
            expand: () => notifications.expandNotification(item.id),
            dismiss: () => notifications.removeNotification(item.id),
        })),
        ...popups.getNotifications().map((item: PopupNotification) => ({
            key: `popup-${item.id}`,
            createdAt: item.createdAt,
            title: item.title,
            text:
                item.text ??
                (item.progressItems
                    ?.filter((progressItem) => progressItem.text)
                    .map((progressItem) => `${progressItem.title}: ${progressItem.text}`)
                    .join('\n') ||
                    undefined),
            type: item.type,
            read: item.read,
            markRead: () => {
                item.read = true
            },
            onClick: item.onClick,
            primaryAction: item.buttons?.[0],
            expand: () => popups.expandNotification(item.id),
            dismiss: () => popups.removeNotification(item.id),
        })),
    ].sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0)),
)

const hasUnreadNotifications = computed(() =>
    notificationHistory.value.some(
        (item) => !item.read && ['error', 'warning'].includes(item.type ?? ''),
    ),
)

function notificationDotClass(type?: NotificationHistoryItem['type']) {
    if (type === 'error') return 'bg-red'
    if (type === 'warning') return 'bg-orange'
    if (type === 'success' || type === 'download') return 'bg-green'
    return 'bg-blue'
}

async function openNotification(item: NotificationHistoryItem) {
    item.markRead()
    if (item.onClick) await item.onClick()
    else item.expand()
    notificationCenterShown.value = false
}

async function runNotificationAction(item: NotificationHistoryItem) {
    if (!item.primaryAction) return
    item.markRead()
    await item.primaryAction.action()
    notificationCenterShown.value = false
}

function clearNotificationHistory() {
    notifications.clearAllNotifications()
    popups.clearAllNotifications()
}

function runPopoutAction(hide: () => void, action: () => void) {
    hide()
    action()
}

const announcementUnreadCount = computed(() => announcementCenter.value?.unreadCount ?? 0)

/**
 * A job of the app's download manager, which runs content installs and the
 * instance jobs (duplicate, repair) alike, with the backend's phase ids.
 */
interface CenterJob {
    id: string
    title: string
    phase: string
    queued: boolean
    progress: { current: number; total: number; bytes: boolean } | null
    speed: number
    cancel: () => void
}

interface LocalJobView {
    job_id: string
    title: string
    phase: string
    status: string
    progress: { current: number; total: number } | null
}

/** The instance jobs of the Downloads area, once that area has created them. */
const localJobs = computed(
    () =>
        state.domains.get('downloads:local-jobs')?.value as
            { jobs: Ref<LocalJobView[]>; cancel: (jobId: string) => void } | undefined,
)

const CONTENT_PHASES: Record<InstallJob['phase'], string> = {
    queued: 'preparing_instance',
    resolving_project: 'preparing_instance',
    downloading: 'downloading_content',
    installing: 'finalizing',
    completed: 'finalizing',
    failed: 'finalizing',
    canceled: 'finalizing',
}

const LOCAL_ACTIVE = new Set(['queued', 'running', 'canceling', 'waiting_for_user'])

const activeJobs = computed<CenterJob[]>(() => [
    ...downloads.activeJobs.value.map((job) => ({
        id: job.id,
        title: job.project_title,
        phase: CONTENT_PHASES[job.phase],
        queued: job.phase === 'queued',
        progress:
            job.phase === 'downloading'
                ? { current: job.bytes_done, total: job.size, bytes: true }
                : job.phase === 'installing'
                  ? { current: 1, total: 1, bytes: false }
                  : null,
        speed: job.speed ?? 0,
        cancel: () => downloads.cancel(job.id),
    })),
    ...(localJobs.value?.jobs.value ?? [])
        .filter((job) => LOCAL_ACTIVE.has(job.status))
        .map((job) => ({
            id: job.job_id,
            title: job.title,
            phase: job.phase,
            queued: job.status === 'queued',
            progress: job.progress
                ? { ...job.progress, bytes: job.phase === 'downloading_minecraft' }
                : null,
            speed: 0,
            cancel: () => localJobs.value?.cancel(job.job_id),
        })),
])
const hasActivity = computed(() => activeJobs.value.length > 0)
const firstJobSpeed = computed(() => activeJobs.value[0]?.speed ?? 0)
const totalSpeedLabel = computed(() => {
    const speed = activeJobs.value.reduce((total, job) => total + job.speed, 0)
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

/** The app's `phaseDescriptors` (DownloadCenterButton.vue). */
const phaseDescriptors: Record<string, (typeof messages)[keyof typeof messages]> = {
    preparing_instance: messages.preparingInstance,
    resolving_pack: messages.resolvingPack,
    downloading_pack_file: messages.downloadingPack,
    downloading_content: messages.downloadingContent,
    downloading_minecraft: messages.downloadingMinecraft,
    preparing_java: messages.preparingJava,
    verifying: messages.verifying,
    finalizing: messages.finalizing,
}

function phaseLabel(job: CenterJob) {
    return formatMessage(phaseDescriptors[job.phase] ?? messages.unknownPhase)
}

/** `installProgressFraction`: unknown until the job reports a determinate total. */
function progressPercent(job: CenterJob) {
    if (job.queued) return null
    if (!job.progress || job.progress.total <= 0) return null
    return Math.min(100, Math.max(0, (job.progress.current / job.progress.total) * 100))
}

function progressLabel(job: CenterJob) {
    if (!job.progress || job.progress.total <= 0) return ''
    const { current, total, bytes } = job.progress
    return bytes
        ? `${formatBytes(current)} / ${formatBytes(total)}`
        : `${Math.min(current, total)} / ${total}`
}

function openDetails(jobId?: string) {
    void router.push(jobId ? { path: '/downloads', query: { job: jobId } } : '/downloads')
}

function openHistory() {
    void router.push({ path: '/downloads', query: { tab: 'history' } })
}

const runningProcesses = computed(() =>
    processes.processes.value.filter((entry) => entry.stage === 'running'),
)
const selectedProcess = computed(() => {
    const selected = processes.selected.value
    return selected && selected.stage === 'running' ? selected : (runningProcesses.value[0] ?? null)
})

function stop(uuid?: string) {
    processes.stop(uuid)
}

function goToTerminal(instanceId?: string) {
    const id = instanceId ?? selectedProcess.value?.instance.id
    if (id) void router.push(`/instance/${encodeURIComponent(id)}/logs`)
}
</script>

<template>
    <div class="flex min-w-0 items-center gap-2">
        <PopoutMenu
            v-if="notificationHistory.length"
            v-model:open="notificationCenterShown"
            placement="bottom-end"
            :container="layer ?? undefined"
        >
            <template #trigger>
                <Button
                    v-tooltip="formatMessage(messages.notifications)"
                    type="quiet"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.notifications)"
                    class="relative"
                    ><BellIcon />
                    <span
                        v-if="hasUnreadNotifications"
                        class="absolute right-0 top-0 size-2 rounded-full bg-red ring-2 ring-bg-raised"
                    />
                </Button>
            </template>
            <template #menu="{ hide }">
                <div class="w-[22rem] max-w-[calc(1280px-2rem)] p-2">
                    <div class="mb-2 flex items-center justify-between px-2">
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.notifications)
                        }}</span>
                        <button
                            v-if="notificationHistory.length"
                            class="text-xs text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]"
                            @click="runPopoutAction(hide, clearNotificationHistory)"
                        >
                            {{ formatMessage(messages.clearNotifications) }}
                        </button>
                    </div>
                    <div
                        v-if="!notificationHistory.length"
                        class="px-2 py-4 text-center text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.noNotifications) }}
                    </div>
                    <div v-else class="flex max-h-[22rem] flex-col gap-1 overflow-auto">
                        <div
                            v-for="item in notificationHistory"
                            :key="item.key"
                            class="flex items-start gap-2 rounded-lg p-2 hover:bg-surface-4"
                        >
                            <div
                                class="mt-1 size-2 shrink-0 rounded-full"
                                :class="notificationDotClass(item.type)"
                            />
                            <button
                                class="min-w-0 flex-1 text-left"
                                @click="openNotification(item)"
                            >
                                <div
                                    class="truncate text-sm font-medium text-[var(--color-text-primary)]"
                                >
                                    {{ item.title }}
                                </div>
                                <div
                                    v-if="item.text"
                                    class="line-clamp-2 text-xs text-[var(--color-text-tertiary)]"
                                >
                                    {{ item.text }}
                                </div>
                            </button>
                            <button
                                v-if="item.primaryAction"
                                class="shrink-0 rounded-md px-2 py-1 text-xs text-brand hover:bg-surface-4"
                                @click.stop="runNotificationAction(item)"
                            >
                                {{ item.primaryAction.label }}
                            </button>
                            <button
                                v-tooltip="formatMessage(messages.dismissNotification)"
                                class="shrink-0 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]"
                                @click="item.dismiss()"
                            >
                                <XIcon class="size-4" />
                            </button>
                        </div>
                    </div>
                </div>
            </template>
        </PopoutMenu>
        <Button
            v-tooltip="formatMessage(messages.announcements)"
            type="quiet"
            circular
            icon-only
            :aria-label="formatMessage(messages.announcements)"
            class="relative"
            @click="announcementCenter?.open()"
            ><NewspaperIcon />
            <span
                v-if="announcementUnreadCount"
                class="absolute right-0 top-0 size-2 rounded-full bg-red ring-2 ring-bg-raised"
            />
        </Button>
        <PopoutMenu
            v-model:open="downloadsOpen"
            placement="bottom-end"
            :side-offset="0"
            dropdown-class="download-center-menu"
            :container="layer ?? undefined"
        >
            <template #trigger>
                <button
                    type="button"
                    :aria-label="formatMessage(messages.downloads)"
                    :title="activeJobs[0]?.title ?? formatMessage(messages.downloads)"
                    :class="[
                        'inline-flex h-9 min-w-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-xl border border-solid border-transparent px-2.5 text-base font-semibold leading-5 text-[var(--color-text-primary)] transition-colors',
                        activeJobs.length
                            ? 'bg-surface-4 hover:bg-surface-5'
                            : 'w-9 shrink-0 !rounded-full bg-transparent hover:bg-surface-4',
                    ]"
                    :style="triggerStyle"
                >
                    <LoaderCircleIcon v-if="hasActivity" class="size-5 shrink-0 animate-spin" />
                    <DownloadIcon v-else class="size-5 shrink-0" />
                    <template v-if="activeJobs.length">
                        <span class="max-w-[12rem] truncate text-sm font-semibold">
                            {{ activeJobs[0].title }}
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
                            {{ Math.min(activeJobs.length, 99) }}
                        </span>
                    </template>
                </button>
            </template>
            <template #menu="{ hide }">
                <div class="w-[26rem] max-w-[calc(1280px-2rem)] p-2">
                    <div v-if="hasActivity" class="mb-2 flex items-center gap-2 px-2">
                        <span class="text-base font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.downloads) }}
                        </span>
                        <span
                            class="flex size-5 items-center justify-center rounded-full bg-green text-xs font-semibold text-white"
                        >
                            {{ activeJobs.length }}
                        </span>
                        <span class="text-sm text-[var(--color-text-tertiary)]">
                            {{ totalSpeedLabel }}
                        </span>
                    </div>
                    <div
                        v-if="activeJobs.length"
                        class="flex max-h-[24rem] flex-col gap-1 overflow-auto"
                    >
                        <div
                            v-for="job in activeJobs"
                            :key="job.id"
                            class="rounded-lg p-2 hover:bg-surface-4"
                        >
                            <div class="flex items-center gap-2">
                                <span class="size-2 shrink-0 rounded-full bg-green" />
                                <button
                                    class="min-w-0 flex-1 truncate text-left text-sm font-semibold text-[var(--color-text-primary)]"
                                    @click="runPopoutAction(hide, () => openDetails(job.id))"
                                >
                                    {{ job.title }}
                                </button>
                                <button
                                    v-tooltip="formatMessage(messages.cancel)"
                                    :aria-label="formatMessage(messages.cancel)"
                                    class="flex size-6 shrink-0 items-center justify-center rounded-full text-[var(--color-text-tertiary)] hover:bg-surface-5 hover:text-[var(--color-text-primary)]"
                                    @click="job.cancel()"
                                >
                                    <XIcon class="size-4" />
                                </button>
                            </div>
                            <div
                                class="mt-1 flex items-center justify-between gap-2 text-xs text-[var(--color-text-tertiary)]"
                            >
                                <span class="truncate">{{ phaseLabel(job) }}</span>
                                <span class="shrink-0">{{ progressLabel(job) }}</span>
                            </div>
                            <ProgressBar
                                class="mt-2"
                                full-width
                                color="green"
                                :progress="progressPercent(job) ?? 0"
                                :max="100"
                                :waiting="progressPercent(job) === null"
                                :aria-label="job.title || phaseLabel(job)"
                            />
                        </div>
                    </div>
                    <div
                        v-else
                        class="px-2 py-4 text-center text-sm text-[var(--color-text-tertiary)]"
                    >
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
        <div
            class="grid items-center gap-2 rounded-xl border border-solid border-surface-5 px-3 py-1.5 text-sm"
            :class="
                selectedProcess
                    ? runningProcesses.length > 1
                        ? 'grid-cols-[auto_minmax(2rem,1fr)_auto_auto_auto]'
                        : 'grid-cols-[auto_minmax(2rem,1fr)_auto_auto]'
                    : 'grid-cols-[auto_minmax(0,1fr)]'
            "
        >
            <template v-if="selectedProcess">
                <OnlineIndicatorIcon class="shrink-0" />
                <RouterLink
                    v-tooltip="formatMessage(messages.viewInstance)"
                    :to="`/instance/${encodeURIComponent(selectedProcess.instance.id)}`"
                    class="max-w-[12rem] truncate text-[var(--color-text-primary)] hover:underline"
                    :title="selectedProcess.instance.name"
                >
                    {{ selectedProcess.instance.name }}
                </RouterLink>
                <PopoutMenu
                    v-if="runningProcesses.length > 1"
                    v-model:open="showInstances"
                    placement="bottom"
                    :container="layer ?? undefined"
                >
                    <template #trigger>
                        <Button
                            v-tooltip="
                                showInstances
                                    ? formatMessage(messages.hideMoreRunningInstances)
                                    : formatMessage(messages.showMoreRunningInstances)
                            "
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            ><DropdownIcon :class="{ 'rotate-180': !!showInstances }" />
                        </Button>
                    </template>
                    <template #menu="{ hide }">
                        <div class="flex max-h-[24rem] w-[20rem] flex-col gap-2 overflow-auto">
                            <div
                                v-for="process in runningProcesses"
                                :key="process.uuid"
                                class="flex w-full items-center gap-2 rounded-xl bg-surface-4 p-2 text-sm"
                            >
                                <button
                                    v-tooltip.left="
                                        process.uuid === selectedProcess.uuid
                                            ? formatMessage(messages.primaryInstance)
                                            : formatMessage(messages.makePrimaryInstance)
                                    "
                                    class="flex flex-grow items-center gap-2"
                                    :class="{
                                        'transition-transform active:scale-95':
                                            process.uuid !== selectedProcess.uuid,
                                    }"
                                    :disabled="process.uuid === selectedProcess.uuid"
                                    @click="
                                        runPopoutAction(hide, () => processes.select(process.uuid))
                                    "
                                >
                                    <OnlineIndicatorIcon />
                                    <span
                                        class="mr-auto flex items-center gap-2 text-[var(--color-text-primary)]"
                                    >
                                        {{ process.instance.name }}
                                        <StarIcon
                                            v-if="process.uuid === selectedProcess.uuid"
                                            class="text-orange"
                                        />
                                    </span>
                                </button>
                                <button
                                    v-tooltip="formatMessage(messages.stopInstance)"
                                    class="flex active:scale-95"
                                    @click.stop="runPopoutAction(hide, () => stop(process.uuid))"
                                >
                                    <StopCircleIcon class="size-5 text-red" />
                                </button>
                                <button
                                    v-tooltip="formatMessage(messages.viewLogs)"
                                    class="flex active:scale-95"
                                    @click.stop="
                                        runPopoutAction(hide, () =>
                                            goToTerminal(process.instance.id),
                                        )
                                    "
                                >
                                    <TerminalSquareIcon
                                        class="size-5 text-[var(--color-text-tertiary)]"
                                    />
                                </button>
                            </div>
                        </div>
                    </template>
                </PopoutMenu>
                <button
                    v-tooltip="formatMessage(messages.stopInstance)"
                    class="flex shrink-0 active:scale-95"
                    @click="stop(selectedProcess.uuid)"
                >
                    <StopCircleIcon class="size-5 text-red" />
                </button>
                <button
                    v-tooltip="formatMessage(messages.viewLogs)"
                    class="flex shrink-0 active:scale-95"
                    @click="goToTerminal()"
                >
                    <TerminalSquareIcon class="size-5 text-[var(--color-text-tertiary)]" />
                </button>
            </template>
            <template v-else>
                <span class="size-2 shrink-0 rounded-full bg-[var(--color-text-tertiary)]" />
                <span
                    class="max-w-[12rem] truncate text-[var(--color-text-tertiary)]"
                    :title="formatMessage(messages.noInstancesRunning)"
                >
                    {{ formatMessage(messages.noInstancesRunning) }}
                </span>
            </template>
        </div>
        <AnnouncementCenter ref="announcementCenter" />
    </div>
</template>

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
