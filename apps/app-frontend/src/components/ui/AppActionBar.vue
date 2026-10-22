<template>
    <div class="flex min-w-0 gap-2 items-center">
        <PopoutMenu
            v-if="notificationHistory.length"
            v-model:open="notificationCenterShown"
            placement="bottom-end"
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
                <div class="w-[22rem] max-w-[calc(100vw-2rem)] p-2">
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
                                @click="dismissNotification(item)"
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
            @click="openAnnouncementCenter"
            ><NewspaperIcon />
            <span
                v-if="announcementUnreadCount"
                class="absolute right-0 top-0 size-2 rounded-full bg-red ring-2 ring-bg-raised"
            />
        </Button>
        <DownloadCenterButton />
        <PopoutMenu v-if="activeBackupOperations.length > 0" placement="bottom-end">
            <template #trigger>
                <Button
                    v-tooltip="formatMessage(messages.activeBackups)"
                    type="quiet"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.activeBackups)"
                    class="relative"
                    ><DatabaseBackupIcon />
                    <span
                        class="absolute right-0 top-0 min-w-4 rounded-full bg-brand px-1 text-center text-[10px] font-bold leading-4 text-white"
                    >
                        {{ Math.min(activeBackupOperations.length, 99) }}
                    </span>
                </Button>
            </template>
            <template #menu="{ hide }">
                <div class="w-[22rem] max-w-[calc(100vw-2rem)] p-2">
                    <div class="mb-2 px-2 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.activeBackups) }}
                    </div>
                    <div class="flex max-h-[22rem] flex-col gap-1 overflow-auto">
                        <button
                            v-for="operation in activeBackupOperations"
                            :key="operation.id"
                            class="flex min-w-0 flex-col gap-1 rounded-lg p-2 text-left hover:bg-surface-4"
                            @click="runPopoutAction(hide, () => openBackupOperation(operation))"
                        >
                            <div class="flex items-center gap-2">
                                <span class="size-2 shrink-0 rounded-full bg-brand" />
                                <span
                                    class="truncate text-sm font-medium text-[var(--color-text-primary)]"
                                >
                                    {{ backupOperationTitle(operation) }}
                                </span>
                            </div>
                            <div
                                class="flex items-center justify-between gap-2 text-xs text-[var(--color-text-tertiary)]"
                            >
                                <span class="truncate">{{
                                    backupInstanceName(operation.instance_id)
                                }}</span>
                                <span class="shrink-0">{{
                                    backupOperationProgress(operation)
                                }}</span>
                            </div>
                        </button>
                    </div>
                </div>
            </template>
        </PopoutMenu>
        <div v-if="offline" class="flex items-center gap-1">
            <UnplugIcon class="text-[var(--color-text-tertiary)]" />
            <span class="text-sm text-[var(--color-text-primary)]">
                {{ formatMessage(messages.offline) }}
            </span>
        </div>
        <AppUpdateButton />
        <div
            class="grid border-solid border-surface-5 text-sm items-center gap-2 py-1.5 px-3 rounded-xl border"
            :class="
                selectedProcess
                    ? currentProcesses.length > 1
                        ? 'grid-cols-[auto_minmax(2rem,1fr)_auto_auto_auto]'
                        : 'grid-cols-[auto_minmax(2rem,1fr)_auto_auto]'
                    : 'grid-cols-[auto_minmax(0,1fr)]'
            "
        >
            <template v-if="selectedProcess">
                <OnlineIndicatorIcon class="shrink-0" />
                <router-link
                    v-tooltip="formatMessage(messages.viewInstance)"
                    :to="`/instance/${encodeURIComponent(selectedProcess.instance.id)}`"
                    class="max-w-[12rem] truncate text-[var(--color-text-primary)] hover:underline"
                    :title="selectedProcess.instance.name"
                >
                    {{ selectedProcess.instance.name }}
                </router-link>
                <PopoutMenu
                    v-if="currentProcesses.length > 1"
                    v-model:open="showInstances"
                    placement="bottom"
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
                        <div class="flex w-[20rem] max-h-[24rem] flex-col gap-2 overflow-auto">
                            <div
                                v-for="process in currentProcesses"
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
                                        'active:scale-95 transition-transform':
                                            process.uuid !== selectedProcess.uuid,
                                    }"
                                    :disabled="process.uuid === selectedProcess.uuid"
                                    @click="runPopoutAction(hide, () => selectProcess(process))"
                                >
                                    <OnlineIndicatorIcon />
                                    <span
                                        class="mr-auto text-[var(--color-text-primary)] flex items-center gap-2"
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
                                    class="active:scale-95 flex"
                                    @click.stop="runPopoutAction(hide, () => stop(process))"
                                >
                                    <StopCircleIcon class="text-red size-5" />
                                </button>
                                <button
                                    v-tooltip="formatMessage(messages.viewLogs)"
                                    class="active:scale-95 flex"
                                    @click.stop="
                                        runPopoutAction(hide, () =>
                                            goToTerminal(process.instance.id),
                                        )
                                    "
                                >
                                    <TerminalSquareIcon
                                        class="text-[var(--color-text-tertiary)] size-5"
                                    />
                                </button>
                            </div>
                        </div>
                    </template>
                </PopoutMenu>
                <button
                    v-tooltip="formatMessage(messages.stopInstance)"
                    class="active:scale-95 flex shrink-0"
                    @click="stop(selectedProcess)"
                >
                    <StopCircleIcon class="text-red size-5" />
                </button>
                <button
                    v-tooltip="formatMessage(messages.viewLogs)"
                    class="active:scale-95 flex shrink-0"
                    @click="goToTerminal()"
                >
                    <TerminalSquareIcon class="text-[var(--color-text-tertiary)] size-5" />
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
    </div>
</template>

<script setup lang="ts">
import {
    BellIcon,
    DatabaseBackupIcon,
    DropdownIcon,
    NewspaperIcon,
    OnlineIndicatorIcon,
    StarIcon,
    StopCircleIcon,
    TerminalSquareIcon,
    UnplugIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    defineMessages,
    injectNotificationManager,
    injectPopupNotificationManager,
    PopoutMenu,
    type PopupNotification,
    type PopupNotificationProgressItem,
    useVIntl,
    type WebNotification,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import AppUpdateButton from '@/components/ui/app-update-button/index.vue'
import DownloadCenterButton from '@/components/ui/DownloadCenterButton.vue'
import { useInstallJobNotifications } from '@/composables/browse/install-job-notifications'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { trackEvent } from '@/helpers/analytics'
import { loading_listener, process_listener } from '@/helpers/events'
import { get_many as getInstances } from '@/helpers/instance'
import {
    type BackupOperation,
    type BackupProgressEvent,
    listBackupOperations,
    listenBackupProgress,
} from '@/helpers/instance-backup'
import { get_all as getRunningProcesses, kill as killProcess } from '@/helpers/process'
import {
    OPEN_REMOTE_ANNOUNCEMENT_CENTER_EVENT,
    REMOTE_ANNOUNCEMENTS_UPDATED_EVENT,
} from '@/helpers/remote-announcements'
import type { LoadingBar } from '@/helpers/state'
import { progress_bars_list } from '@/helpers/state'
import type { GameInstance } from '@/helpers/types'
import { injectDownloadManager } from '@/providers/download-manager'

const notificationManager = injectNotificationManager()
const { addNotification, handleError } = notificationManager
const popupNotificationManager = injectPopupNotificationManager()
const downloadManager = injectDownloadManager()
const { formatMessage } = useVIntl()

type NotificationHistoryItem = {
    key: string
    createdAt?: number
    title: string
    text?: string
    type?: 'error' | 'warning' | 'success' | 'info' | 'download'
    collapsed?: boolean
    read?: boolean
    onClick?: () => void | Promise<void>
    primaryAction?: {
        label: string
        action: () => void | Promise<void>
    }
    markRead: () => void
    expand: () => void
    dismiss: () => void
}

const notificationHistory = computed<NotificationHistoryItem[]>(() =>
    [
        ...notificationManager.getNotifications().map((item: WebNotification) => ({
            key: `web-${item.id}`,
            createdAt: item.createdAt,
            title: item.title ?? formatMessage(messages.notifications),
            text: item.text,
            type: item.type,
            collapsed: item.collapsed,
            read: item.read,
            markRead: () => notificationManager.markNotificationRead(item.id),
            expand: () => notificationManager.expandNotification(item.id),
            dismiss: () => notificationManager.removeNotification(item.id),
        })),
        ...popupNotificationManager.getNotifications().map((item: PopupNotification) => ({
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
            collapsed: item.collapsed,
            read: item.read,
            markRead: () => {
                item.read = true
            },
            onClick: item.onClick,
            primaryAction: item.buttons?.[0],
            expand: () => popupNotificationManager.expandNotification(item.id),
            dismiss: () => popupNotificationManager.removeNotification(item.id),
        })),
    ].sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0)),
)

const hasUnreadNotifications = computed(() =>
    notificationHistory.value.some(
        (item) => !item.read && ['error', 'warning'].includes(item.type ?? ''),
    ),
)

function notificationDotClass(type?: NotificationHistoryItem['type']): string {
    if (type === 'error') return 'bg-red'
    if (type === 'warning') return 'bg-orange'
    if (type === 'success') return 'bg-green'
    if (type === 'download') return 'bg-green'
    return 'bg-blue'
}

function dismissNotification(item: NotificationHistoryItem) {
    item.dismiss()
}

async function openNotification(item: NotificationHistoryItem) {
    item.markRead()
    if (item.onClick) {
        await item.onClick()
    } else {
        item.expand()
    }
    notificationCenterShown.value = false
}

async function runNotificationAction(item: NotificationHistoryItem) {
    if (!item.primaryAction) return
    item.markRead()
    await item.primaryAction.action()
    notificationCenterShown.value = false
}

function clearNotificationHistory() {
    const progressItems = buildDownloadItems()
    // Clearing the history has to drop the remembered dismissal as well, even
    // when there is currently nothing on the list.
    persistDismissedDownloadSignature(null)
    dismissedDownloadSignature.value = null
    if (progressItems.length > 0) {
        dismissed.value = true
        dismissedDownloadSignature.value = buildDownloadSignature(progressItems)
        persistDismissedDownloadSignature(dismissedDownloadSignature.value)
    }
    removeNotification(false)
    notificationManager.clearAllNotifications()
    popupNotificationManager.clearAllNotifications()
}

const router = useRouter()
const route = useRoute()
const isDownloadsPage = computed(
    () => route.path === '/downloads' || route.path.startsWith('/downloads/'),
)

const showInstances = ref(false)
const notificationCenterShown = ref(false)
const announcementUnreadCount = ref(0)

function openAnnouncementCenter() {
    window.dispatchEvent(new CustomEvent(OPEN_REMOTE_ANNOUNCEMENT_CENTER_EVENT))
}

function updateAnnouncementCount(event: Event) {
    const detail = (event as CustomEvent<{ unreadKeys?: string[] }>).detail
    announcementUnreadCount.value = detail?.unreadKeys?.length ?? 0
}

onMounted(() => {
    window.addEventListener(REMOTE_ANNOUNCEMENTS_UPDATED_EVENT, updateAnnouncementCount)
})
onBeforeUnmount(() => {
    window.removeEventListener(REMOTE_ANNOUNCEMENTS_UPDATED_EVENT, updateAnnouncementCount)
})

interface RunningProcess {
    uuid: string
    instance_id: string
    instance: GameInstance
}

interface LoadingEventPayload {
    event: LoadingBar['bar_type']
    loader_uuid: string
    fraction: number | null
    message: string
}

const messages = defineMessages({
    offline: {
        id: 'app.action-bar.offline',
        defaultMessage: 'Offline',
    },
    viewInstance: {
        id: 'app.action-bar.view-instance',
        defaultMessage: 'View instance',
    },
    showMoreRunningInstances: {
        id: 'app.action-bar.show-more-running-instances',
        defaultMessage: 'Show more running instances',
    },
    hideMoreRunningInstances: {
        id: 'app.action-bar.hide-more-running-instances',
        defaultMessage: 'Hide more running instances',
    },
    primaryInstance: {
        id: 'app.action-bar.primary-instance',
        defaultMessage: 'Primary instance',
    },
    makePrimaryInstance: {
        id: 'app.action-bar.make-primary-instance',
        defaultMessage: 'Make primary instance',
    },
    stopInstance: {
        id: 'app.action-bar.stop-instance',
        defaultMessage: 'Stop instance',
    },
    viewLogs: {
        id: 'app.action-bar.view-logs',
        defaultMessage: 'View logs',
    },
    noInstancesRunning: {
        id: 'app.action-bar.no-instances-running',
        defaultMessage: 'No instances running',
    },
    notifications: {
        id: 'app.action-bar.notifications',
        defaultMessage: 'Notifications',
    },
    announcements: {
        id: 'app.action-bar.announcements',
        defaultMessage: 'Announcements',
    },
    clearNotifications: {
        id: 'app.action-bar.notifications.clear',
        defaultMessage: 'Clear all',
    },
    noNotifications: {
        id: 'app.action-bar.notifications.empty',
        defaultMessage: 'No notifications',
    },
    dismissNotification: {
        id: 'app.action-bar.notifications.dismiss',
        defaultMessage: 'Dismiss notification',
    },
    downloadingJava: {
        id: 'app.action-bar.downloading-java',
        defaultMessage: 'Downloading Java {version}',
    },
    downloadingModpack: {
        id: 'app.downloads.phase.downloading-pack-file',
        defaultMessage: 'Downloading modpack',
    },
    downloads: {
        id: 'app.action-bar.downloads',
        defaultMessage: 'Downloads',
    },
    viewActiveDownloads: {
        id: 'app.action-bar.view-active-downloads',
        defaultMessage: 'View active downloads',
    },
    activeBackups: {
        id: 'app.action-bar.active-backups',
        defaultMessage: 'Active backups',
    },
    backup: {
        id: 'app.action-bar.backup',
        defaultMessage: 'Backing up',
    },
    restore: {
        id: 'app.action-bar.restore',
        defaultMessage: 'Restoring backup',
    },
    restorePreview: {
        id: 'app.action-bar.restore-preview',
        defaultMessage: 'Analyzing restore',
    },
    repositoryMove: {
        id: 'app.action-bar.repository-move',
        defaultMessage: 'Moving backup repository',
    },
    repository: {
        id: 'app.action-bar.repository',
        defaultMessage: 'Backup repository',
    },
    backupTask: { id: 'app.action-bar.backup-task.backup', defaultMessage: 'Backup' },
    restoreTask: { id: 'app.action-bar.backup-task.restore', defaultMessage: 'Backup restore' },
    restorePreviewTask: {
        id: 'app.action-bar.backup-task.restore-preview',
        defaultMessage: 'Restore analysis',
    },
    repositoryMoveTask: {
        id: 'app.action-bar.backup-task.repository-move',
        defaultMessage: 'Backup repository move',
    },
    taskCompleted: {
        id: 'app.action-bar.backup-task.completed',
        defaultMessage: '{operation} completed',
    },
    taskCancelled: {
        id: 'app.action-bar.backup-task.cancelled',
        defaultMessage: '{operation} cancelled',
    },
    taskFailed: {
        id: 'app.action-bar.backup-task.failed',
        defaultMessage: '{operation} failed',
    },
    backupStage: {
        id: 'app.action-bar.backup-stage',
        defaultMessage: '{stage}',
    },
    stageQueued: { id: 'app.action-bar.backup-stage.queued', defaultMessage: 'Queued' },
    stageScanning: { id: 'app.action-bar.backup-stage.scanning', defaultMessage: 'Scanning files' },
    stageHashing: {
        id: 'app.action-bar.backup-stage.hashing',
        defaultMessage: 'Calculating hashes',
    },
    stageSaving: { id: 'app.action-bar.backup-stage.saving', defaultMessage: 'Saving backup' },
    stageValidating: {
        id: 'app.action-bar.backup-stage.validating',
        defaultMessage: 'Validating backup',
    },
    stageMaterializing: {
        id: 'app.action-bar.backup-stage.materializing',
        defaultMessage: 'Applying files',
    },
    stageApplying: { id: 'app.action-bar.backup-stage.applying', defaultMessage: 'Applying files' },
    exportingModpack: {
        id: 'app.action-bar.exporting-modpack',
        defaultMessage: 'Exporting modpack',
    },
})

const currentProcesses = ref<RunningProcess[]>([])
const selectedProcess = ref<RunningProcess | undefined>()
const activeBackupOperations = ref<BackupOperation[]>([])
const backupInstanceNames = ref<Record<string, string>>({})
const notifiedBackupOperations = new Set<string>()
const eventActiveBackupOperations = new Set<string>()
let backupRefreshGeneration = 0

function backupOperationMessage(operationType: BackupOperation['operation_type']) {
    switch (operationType) {
        case 'restore':
            return messages.restore
        case 'restore_preview':
            return messages.restorePreview
        case 'repository_move':
            return messages.repositoryMove
        default:
            return messages.backup
    }
}

function backupResultOperationMessage(operationType: BackupOperation['operation_type']) {
    switch (operationType) {
        case 'restore':
            return messages.restoreTask
        case 'restore_preview':
            return messages.restorePreviewTask
        case 'repository_move':
            return messages.repositoryMoveTask
        default:
            return messages.backupTask
    }
}

function backupInstanceName(instanceId: string): string {
    if (instanceId === '__backup_repository__') return formatMessage(messages.repository)
    return backupInstanceNames.value[instanceId] ?? instanceId
}

function backupOperationTitle(operation: BackupOperation): string {
    return formatMessage(backupOperationMessage(operation.operation_type))
}

function backupOperationProgress(operation: BackupOperation): string {
    if (!operation.total_bytes) {
        const stageMessage =
            {
                queued: messages.stageQueued,
                scanning: messages.stageScanning,
                hashing: messages.stageHashing,
                saving: messages.stageSaving,
                validating: messages.stageValidating,
                materializing: messages.stageMaterializing,
                applying: messages.stageApplying,
            }[operation.state] ?? messages.backupStage
        return formatMessage(
            stageMessage,
            stageMessage === messages.backupStage ? { stage: operation.state } : undefined,
        )
    }
    const percent = Math.round((operation.processed_bytes / operation.total_bytes) * 100)
    return `${Math.min(100, Math.max(0, percent))}%`
}

function openBackupOperation(operation: BackupOperation) {
    if (operation.operation_type === 'repository_move') {
        router.push('/settings#storage-backups')
        return
    }
    router.push(`/instance/${encodeURIComponent(operation.instance_id)}`)
}

async function refreshBackupOperations() {
    const generation = ++backupRefreshGeneration
    const operations = await listBackupOperations(undefined, true).catch((error) => {
        handleError(error)
        return []
    })
    if (generation !== backupRefreshGeneration) return
    const listedOperations = operations.filter((operation) =>
        ['create', 'restore', 'restore_preview', 'repository_move'].includes(
            operation.operation_type,
        ),
    )
    const eventOperations = activeBackupOperations.value.filter(
        (operation) =>
            eventActiveBackupOperations.has(operation.id) &&
            !listedOperations.some((listed) => listed.id === operation.id),
    )
    activeBackupOperations.value = [...listedOperations, ...eventOperations]
    const instanceIds = Array.from(
        new Set(
            activeBackupOperations.value
                .filter((operation) => operation.operation_type !== 'repository_move')
                .map((operation) => operation.instance_id),
        ),
    )
    if (!instanceIds.length) {
        backupInstanceNames.value = {}
        return
    }
    const instances = await getInstances(instanceIds).catch((error) => {
        handleError(error)
        return []
    })
    if (generation !== backupRefreshGeneration) return
    backupInstanceNames.value = Object.fromEntries(
        instances.map((instance) => [instance.id, instance.name]),
    )
}

async function notifyBackupResult(event: BackupProgressEvent) {
    if (!event.finalState || notifiedBackupOperations.has(event.operationId)) return
    notifiedBackupOperations.add(event.operationId)
    let instanceName = event.instanceId ? backupInstanceNames.value[event.instanceId] : undefined
    if (event.instanceId && !instanceName) {
        const instances = await getInstances([event.instanceId]).catch(() => [])
        instanceName = instances[0]?.name
    }
    const operation = formatMessage(backupResultOperationMessage(event.operationType))
    const resultMessage =
        event.finalState === 'completed'
            ? messages.taskCompleted
            : event.finalState === 'cancelled'
              ? messages.taskCancelled
              : messages.taskFailed
    addNotification({
        title: formatMessage(resultMessage, { operation }),
        text: [instanceName, event.finalState === 'failed' ? event.message : undefined]
            .filter(Boolean)
            .join(': '),
        type:
            event.finalState === 'completed'
                ? 'success'
                : event.finalState === 'cancelled'
                  ? 'info'
                  : 'error',
    })
}

function applyBackupProgress(event: BackupProgressEvent) {
    if (!['create', 'restore', 'restore_preview', 'repository_move'].includes(event.operationType))
        return
    if (event.finalState || ['completed', 'cancelled', 'failed'].includes(event.stage)) {
        eventActiveBackupOperations.delete(event.operationId)
        activeBackupOperations.value = activeBackupOperations.value.filter(
            (operation) => operation.id !== event.operationId,
        )
        void notifyBackupResult(event)
        void refreshBackupOperations()
        return
    }
    const state: BackupOperation['state'] =
        event.stage === 'copying' || event.stage === 'restoring' ? 'materializing' : event.stage
    eventActiveBackupOperations.add(event.operationId)
    const existing = activeBackupOperations.value.find(
        (operation) => operation.id === event.operationId,
    )
    if (existing) {
        existing.state = state
        existing.processed_bytes = event.processedBytes
        existing.total_bytes = event.totalBytes
    } else {
        activeBackupOperations.value.push({
            id: event.operationId,
            operation_type: event.operationType,
            instance_id: event.instanceId ?? '__backup_repository__',
            snapshot_id: event.snapshotId,
            state,
            processed_bytes: event.processedBytes,
            total_bytes: event.totalBytes,
            cancellable: ['scanning', 'hashing'].includes(event.stage),
            cancel_requested: false,
            created_at: Date.now(),
            updated_at: Date.now(),
        })
    }
}

const refresh = async () => {
    const processes = ((await getRunningProcesses().catch((error) => {
        handleError(error)
        return []
    })) ?? []) as Array<{ uuid: string; instance_id: string }>
    const instanceIds = processes.map((process) => process.instance_id)
    const instances: GameInstance[] = await getInstances(instanceIds).catch((error) => {
        handleError(error)
        return []
    })

    currentProcesses.value = processes
        .map((process) => {
            const instance = instances.find((item) => process.instance_id === item.id)
            if (!instance) {
                return null
            }
            return {
                ...process,
                instance,
            }
        })
        .filter((process): process is RunningProcess => process !== null)
    if (!selectedProcess.value || !currentProcesses.value.includes(selectedProcess.value)) {
        selectedProcess.value = currentProcesses.value[0]
    }
}

const unlistenBackup = await listenBackupProgress(applyBackupProgress)
await refresh()
await refreshBackupOperations()

const { offline } = useNetworkStatus()

const unlistenProcess = await process_listener(async () => {
    await refresh()
})

const stop = async (process: RunningProcess) => {
    try {
        await killProcess(process.uuid).catch(handleError)

        trackEvent('InstanceStop', {
            loader: process.instance.loader,
            game_version: process.instance.game_version,
            source: 'AppBar',
        })
    } catch (e) {
        console.error(e)
    }
    await refresh()
}

function goToTerminal(instanceId?: string) {
    const selectedInstanceId = instanceId ?? selectedProcess.value?.instance.id
    if (!selectedInstanceId) {
        return
    }
    router.push(`/instance/${encodeURIComponent(selectedInstanceId)}/logs`)
}

const currentLoadingBars = ref<LoadingBar[]>([])
const currentLoadingBarIconUrls = ref<Record<string, string | null>>({})
const notificationId = ref<string | number | null>(null)

// Dismissing the download notification has to survive a restart: the jobs it
// describes (such as an install interrupted by closing the launcher) are still
// stored, so the notification would otherwise be rebuilt on every launch.
const DISMISSED_DOWNLOAD_NOTIFICATION_KEY = 'axolotl:dismissed-download-notification-v1'

function loadDismissedDownloadSignature(): string | null {
    try {
        const value = localStorage.getItem(DISMISSED_DOWNLOAD_NOTIFICATION_KEY)
        return value && value.length > 0 ? value : null
    } catch {
        return null
    }
}

function persistDismissedDownloadSignature(signature: string | null): void {
    try {
        if (signature === null) {
            localStorage.removeItem(DISMISSED_DOWNLOAD_NOTIFICATION_KEY)
        } else {
            localStorage.setItem(DISMISSED_DOWNLOAD_NOTIFICATION_KEY, signature)
        }
    } catch {
        // Dismissal stays session-only when storage is unavailable.
    }
}

const dismissedDownloadSignature = ref<string | null>(loadDismissedDownloadSignature())
const dismissed = ref(dismissedDownloadSignature.value !== null)
const loadingBarProgress = new Map<string, number>()

function getLoadingBarKey(loadingBar: LoadingBar): string {
    return `${loadingBar.loading_bar_uuid ?? loadingBar.id}`
}

function getLoadingProgress(loadingBar: LoadingBar): number {
    if (!loadingBar.total || loadingBar.total <= 0) {
        return 0
    }
    const key = getLoadingBarKey(loadingBar)
    const progress = Math.max(0, Math.min(1, (loadingBar.current ?? 0) / loadingBar.total))
    return Math.max(progress, loadingBarProgress.get(key) ?? 0)
}

function getLoadingText(loadingBar: LoadingBar): string {
    return loadingBar.message ?? ''
}

function getDisplayIconUrl(icon: string | null | undefined): string | null {
    if (!icon) {
        return null
    }
    if (/^(https?:|data:|blob:|asset:|tauri:)/.test(icon)) {
        return icon
    }
    return convertFileSrc(icon)
}

function getNotification(): PopupNotification | null {
    if (!notificationId.value) {
        return null
    }
    const notification = popupNotificationManager
        .getNotifications()
        .find((notification) => notification.id === notificationId.value)
    return notification ?? null
}

function collapseNotification(): void {
    if (!notificationId.value) {
        return
    }
    popupNotificationManager.collapseNotification(notificationId.value)
}

function removeNotification(markDismissed = true): void {
    if (!notificationId.value) {
        return
    }
    if (markDismissed) {
        dismissed.value = true
        dismissedDownloadSignature.value = buildDownloadSignature(buildDownloadItems())
        persistDismissedDownloadSignature(dismissedDownloadSignature.value)
    }
    popupNotificationManager.removeNotification(notificationId.value)
    notificationId.value = null
}

function buildDownloadItems(): PopupNotificationProgressItem[] {
    return [
        ...installJobNotifications.progressItems.value,
        ...currentLoadingBars.value.map((bar) => {
            const isPackDownload = bar.bar_type?.type === 'pack_download'
            return {
                id: getLoadingBarKey(bar),
                title: bar.title ?? '',
                text: getLoadingText(bar),
                iconUrl: currentLoadingBarIconUrls.value[getLoadingBarKey(bar)] ?? null,
                progress: getLoadingProgress(bar),
                waiting: !bar.total || bar.total <= 0,
                // Pack downloads report file counts, so prefer count UI over raw percentage.
                progressType: isPackDownload ? 'count' : 'percentage',
                progressCurrent:
                    bar.total && bar.total > 0 ? getLoadingProgress(bar) * bar.total : bar.current,
                progressTotal: bar.total,
            }
        }),
    ]
}

const hasDownloadNotificationItems = computed(
    () => installJobNotifications.hasItems.value || currentLoadingBars.value.length > 0,
)

function buildDownloadSignature(items: PopupNotificationProgressItem[]): string {
    const itemSignature = items
        .map((item) => item.id)
        .sort()
        .join('|')
    // Job state is part of the signature so that a job which keeps its id but
    // changes status or phase is treated as new information instead of matching
    // a dismissal that was remembered from an earlier launch.
    const stateSignature = installJobNotifications.stateSignature.value
    return stateSignature ? `${itemSignature}|state:${stateSignature}` : itemSignature
}

function updateNotification(resummon = false): void {
    const shouldResummon = resummon && !isDownloadsPage.value
    if (shouldResummon) {
        dismissed.value = false
        dismissedDownloadSignature.value = null
        persistDismissedDownloadSignature(null)
    }

    const progressItems = buildDownloadItems()
    const signature = buildDownloadSignature(progressItems)

    if (!hasDownloadNotificationItems.value) {
        removeNotification(false)
        dismissed.value = false
        dismissedDownloadSignature.value = null
        persistDismissedDownloadSignature(null)
        return
    }

    if (dismissed.value && dismissedDownloadSignature.value !== signature) {
        dismissed.value = false
        dismissedDownloadSignature.value = null
        persistDismissedDownloadSignature(null)
    }

    if (notificationId.value && !getNotification()) {
        notificationId.value = null
        dismissed.value = true
        dismissedDownloadSignature.value = signature
        persistDismissedDownloadSignature(signature)
    }

    if (dismissed.value && !shouldResummon) {
        return
    }

    let notif = getNotification()
    if (notif?.collapsed && shouldResummon) {
        notif.collapsed = false
    }
    if (notif) {
        notif.title = installJobNotifications.hasItems.value
            ? installJobNotifications.title.value
            : formatMessage(messages.downloads)
        notif.text = undefined
        notif.progressItems = progressItems
        notif.buttons = installJobNotifications.buttons.value
        notif.onClick = hasDownloadNotificationItems.value ? goToDownloads : undefined
        notif.progress = undefined
        notif.waiting = undefined
        notif.autoCloseMs =
            progressItems.length > 0 && progressItems.every((item) => item.showProgress === false)
                ? 30 * 1000
                : null
        if (!notif.collapsed) popupNotificationManager.setNotificationTimer(notif)
    } else {
        notif = popupNotificationManager.addPopupNotification({
            title: installJobNotifications.hasItems.value
                ? installJobNotifications.title.value
                : formatMessage(messages.downloads),
            type: 'download',
            autoCloseMs: null,
            progressItems,
            buttons: installJobNotifications.buttons.value,
            onClick: hasDownloadNotificationItems.value ? goToDownloads : undefined,
        })
        notificationId.value = notif.id
        if (isDownloadsPage.value) {
            popupNotificationManager.collapseNotification(notif.id)
        }
        if (
            progressItems.length > 0 &&
            progressItems.every((item) => item.showProgress === false)
        ) {
            notif.autoCloseMs = 30 * 1000
            popupNotificationManager.setNotificationTimer(notif)
        }
    }
}

function formatLoadingBars(loadingBar: LoadingBar): LoadingBar {
    const formatted = { ...loadingBar }
    if (formatted.bar_type?.type === 'java_download') {
        formatted.title = formatMessage(messages.downloadingJava, {
            version: formatted.bar_type.version,
        })
    }
    if (formatted.bar_type?.type === 'pack_file_download') {
        formatted.message = formatMessage(messages.downloadingModpack)
    }
    if (formatted.bar_type?.instance_id) {
        formatted.title = formatted.bar_type.instance_name ?? formatted.bar_type.instance_id
    }
    if (formatted.bar_type?.type === 'zip_extract') {
        formatted.title = formatMessage(messages.exportingModpack)
    }
    if (formatted.bar_type?.pack_name) {
        formatted.title = formatted.bar_type.pack_name
    }
    return formatted
}

function isVisibleLoadingBar(loadingBar: LoadingBar): boolean {
    return (
        loadingBar.bar_type?.type !== 'launcher_update' &&
        [
            'java_download',
            'pack_file_download',
            'pack_download',
            'minecraft_download',
            'copy_instance',
            'zip_extract',
        ].includes(loadingBar.bar_type?.type ?? '')
    )
}

function applyLoadingEvent(payload: LoadingEventPayload): boolean {
    const key = payload.loader_uuid
    const index = currentLoadingBars.value.findIndex((bar) => getLoadingBarKey(bar) === key)

    if (payload.fraction === null) {
        if (index >= 0) {
            currentLoadingBars.value.splice(index, 1)
            const { [key]: _removedIcon, ...remainingIcons } = currentLoadingBarIconUrls.value
            currentLoadingBarIconUrls.value = remainingIcons
        }
        loadingBarProgress.delete(key)
        return false
    }

    const loadingBar = formatLoadingBars({
        loading_bar_uuid: payload.loader_uuid,
        message: payload.message,
        current: payload.fraction,
        total: 1,
        bar_type: payload.event,
    })
    if (!isVisibleLoadingBar(loadingBar)) return false

    loadingBarProgress.set(key, getLoadingProgress(loadingBar))

    if (index >= 0) {
        currentLoadingBars.value.splice(index, 1, loadingBar)
    } else {
        currentLoadingBars.value.push(loadingBar)
    }
    currentLoadingBarIconUrls.value[key] = getDisplayIconUrl(payload.event?.icon)
    return index < 0
}

async function refreshLoadingBars() {
    const bars: Record<string, LoadingBar> = await progress_bars_list().catch((error) => {
        handleError(error)
        return {}
    })

    currentLoadingBars.value = Object.values(bars)
        .map(formatLoadingBars)
        .filter(isVisibleLoadingBar)

    const activeKeys = new Set(currentLoadingBars.value.map(getLoadingBarKey))
    for (const key of loadingBarProgress.keys()) {
        if (!activeKeys.has(key)) loadingBarProgress.delete(key)
    }
    for (const bar of currentLoadingBars.value) {
        const progress = getLoadingProgress(bar)
        if (bar.total && bar.total > 0) loadingBarProgress.set(getLoadingBarKey(bar), progress)
    }

    const instanceIds = Array.from(
        new Set(
            currentLoadingBars.value
                .map((bar) => bar.bar_type?.instance_id)
                .filter((instanceId): instanceId is string => !!instanceId),
        ),
    )
    const instances = instanceIds.length
        ? await getInstances(instanceIds).catch((error) => {
              handleError(error)
              return []
          })
        : []
    const instanceIconUrls = new Map(
        instances.map((instance) => [instance.id, getDisplayIconUrl(instance.icon_path)]),
    )
    currentLoadingBarIconUrls.value = Object.fromEntries(
        currentLoadingBars.value.map((bar) => {
            const barIconUrl = getDisplayIconUrl(bar.bar_type?.icon)
            const instanceIconUrl = bar.bar_type?.instance_id
                ? instanceIconUrls.get(bar.bar_type.instance_id)
                : null
            return [getLoadingBarKey(bar), barIconUrl ?? instanceIconUrl ?? null]
        }),
    )

    currentLoadingBars.value.sort((a, b) => {
        const aKey = `${a.loading_bar_uuid ?? a.id ?? ''}`
        const bKey = `${b.loading_bar_uuid ?? b.id ?? ''}`
        return aKey.localeCompare(bKey)
    })

    updateNotification()
}

const installJobNotifications = await useInstallJobNotifications({
    router,
    manager: downloadManager,
    handleError,
    onChange: updateNotification,
})

await refreshLoadingBars()

let newBarDuringWindow = false
let loadingNotificationTimer: ReturnType<typeof setTimeout> | null = null

const unlistenLoading = await loading_listener((payload: LoadingEventPayload) => {
    const isNewBar = applyLoadingEvent(payload)
    if (isNewBar) {
        newBarDuringWindow = true
    }
    if (loadingNotificationTimer !== null) {
        return
    }
    loadingNotificationTimer = setTimeout(() => {
        loadingNotificationTimer = null
        if (newBarDuringWindow) {
            newBarDuringWindow = false
            if (isDownloadsPage.value) {
                updateNotification()
            } else {
                removeNotification()
                updateNotification(true)
            }
        } else {
            updateNotification()
        }
    }, 250)
})

function goToDownloads() {
    router.push('/downloads')
}

watch(
    () => route.path,
    () => {
        if (isDownloadsPage.value) {
            collapseNotification()
        }
        updateNotification()
    },
)

function runPopoutAction(hide: () => void, action: () => void) {
    action()
    hide()
}

function selectProcess(process: RunningProcess) {
    selectedProcess.value = process
}

onBeforeUnmount(() => {
    if (loadingNotificationTimer !== null) {
        clearTimeout(loadingNotificationTimer)
        loadingNotificationTimer = null
    }
    removeNotification(false)
    dismissed.value = false
    dismissedDownloadSignature.value = null
    unlistenProcess()
    unlistenLoading()
    unlistenBackup()
    installJobNotifications.dispose()
})
</script>
