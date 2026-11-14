<template>
    <div
        class="popup-notification-group"
        role="log"
        :aria-live="notifications.some((item) => item.type === 'error') ? 'assertive' : 'polite'"
        aria-relevant="additions text"
        :class="{
            'has-sidebar': hasSidebar,
            'has-modal': hasModalActive,
        }"
        :style="notificationGroupStyle"
    >
        <transition-group name="popup-notifs">
            <div
                v-for="item in notifications"
                :key="item.id"
                class="popup-notification-wrapper"
                @mouseenter="stopTimer(item)"
                @mouseleave="setNotificationTimer(item)"
            >
                <NotificationToast
                    v-if="item.toast"
                    :type="item.toast.type"
                    :actor-name="item.toast.actorName"
                    :actor-avatar-url="item.toast.actorAvatarUrl"
                    :entity-name="item.toast.entityName"
                    :entity-icon-url="item.toast.entityIconUrl"
                    :status-text="item.toast.statusText"
                    :progress="item.toast.progress"
                    :waiting="item.toast.waiting"
                    :show-progress="item.toast.showProgress"
                    :progress-type="item.toast.progressType"
                    :progress-current="item.toast.progressCurrent"
                    :progress-total="item.toast.progressTotal"
                    @accept="handleToastAction(item, item.toast.onAccept)"
                    @decline="handleToastAction(item, item.toast.onDecline)"
                    @dismiss="handleToastAction(item, item.toast.onDismiss)"
                    @launch="handleToastAction(item, item.toast.onLaunch)"
                    @open-actor="item.toast.onOpenActor?.()"
                    @open-instance="handleToastAction(item, item.toast.onOpenInstance)"
                />
                <div v-else-if="isDownloadNotification(item)" class="flex flex-col gap-4">
                    <div v-for="progressItem in downloadToastItems(item)" :key="progressItem.id">
                        <NotificationToast
                            type="instance-download"
                            :entity-name="progressItem.title || item.title"
                            :entity-icon-url="
                                progressItem.iconUrl ?? item.iconUrl ?? MinecraftServerIcon
                            "
                            :status-text="progressItem.text"
                            :progress="progressItem.progress"
                            :waiting="progressItem.waiting"
                            :show-progress="progressItem.showProgress"
                            :wrap-text="progressItem.wrapText"
                            :progress-type="progressItem.progressType"
                            :progress-current="progressItem.progressCurrent"
                            :progress-total="progressItem.progressTotal"
                            :actions="progressItem.buttons"
                            class="cursor-pointer"
                            @click="handleDownloadClick(item, $event)"
                            @dismiss="handleProgressItemDismiss(item, progressItem)"
                            @action="(index) => handleProgressItemAction(progressItem, index)"
                        />
                    </div>
                </div>
                <div
                    v-else
                    class="flex w-full flex-col gap-3 overflow-hidden rounded-2xl bg-surface-3 shadow-xl border-surface-5 border-solid border p-4"
                >
                    <div class="flex flex-col gap-2 w-full">
                        <div class="flex items-center justify-between gap-2.5">
                            <div class="flex min-w-0 flex-1 items-center gap-2">
                                <component
                                    :is="item.titleLogo"
                                    v-if="item.titleLogo"
                                    class="h-7 w-auto min-w-0 max-w-full text-[var(--color-text-primary)]"
                                />
                                <template v-else>
                                    <div
                                        class="flex items-center"
                                        :class="{
                                            'text-red': item.type === 'error',
                                            'text-orange': item.type === 'warning',
                                            'text-green': item.type === 'download',
                                            'text-[var(--color-text-primary)]':
                                                item.type === 'success',
                                            'text-blue':
                                                !item.type ||
                                                ![
                                                    'error',
                                                    'warning',
                                                    'success',
                                                    'download',
                                                ].includes(item.type),
                                        }"
                                    >
                                        <IssuesIcon
                                            v-if="item.type === 'warning'"
                                            class="h-5 w-5"
                                        />
                                        <DownloadIcon
                                            v-else-if="item.type === 'download'"
                                            class="h-5 w-5"
                                        />
                                        <CheckCircleIcon
                                            v-else-if="item.type === 'success'"
                                            class="h-5 w-5"
                                        />
                                        <XCircleIcon
                                            v-else-if="item.type === 'error'"
                                            class="h-5 w-5"
                                        />
                                        <InfoIcon v-else class="h-5 w-5" />
                                    </div>
                                    <div
                                        class="text-[var(--color-text-primary)] font-semibold m-0 grow"
                                    >
                                        {{ item.title }}
                                    </div>
                                </template>
                            </div>
                            <Button
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                :label="formatMessage(messages.dismissNotification)"
                                @click="dismiss(item.id)"
                                ><XIcon />
                            </Button>
                        </div>
                        <span v-if="item.text" class="text-[var(--color-text-default)]">
                            {{ item.text }}
                        </span>
                        <component
                            :is="item.bodyComponent"
                            v-if="item.bodyComponent"
                            v-bind="item.bodyProps ?? {}"
                        />
                    </div>
                    <div v-if="item.progressItems?.length" class="flex flex-col gap-3">
                        <div
                            v-for="progressItem in item.progressItems"
                            :key="progressItem.id"
                            class="flex flex-col gap-2"
                        >
                            <div class="text-[var(--color-text-primary)] truncate">
                                {{ progressItem.title }}
                            </div>
                            <ProgressBar
                                :progress="progressItem.progress"
                                :max="1"
                                :waiting="progressItem.waiting"
                                :color="progressColorForType(item.type)"
                                :gradient-border="false"
                                full-width
                            />
                            <div
                                v-if="progressItem.text"
                                class="text-sm text-[var(--color-text-tertiary)] truncate"
                            >
                                {{ progressItem.text }}
                            </div>
                        </div>
                    </div>
                    <ProgressBar
                        v-else-if="item.progress != null || item.waiting"
                        :progress="item.progress ?? 0"
                        :max="1"
                        :waiting="item.waiting ?? false"
                        :color="progressColorForType(item.type)"
                        :gradient-border="false"
                        full-width
                    />
                    <div
                        v-if="(item.type === 'error' && onErrorAction) || item.buttons?.length"
                        class="flex gap-1.5"
                    >
                        <Button
                            v-if="item.type === 'error' && onErrorAction"
                            :disabled="exporting[item.id]"
                            @click="handleErrorAction(item)"
                            ><DownloadIcon />
                            {{ errorActionLabel || formatMessage(messages.exportErrorLogs) }}
                        </Button>
                        <Button
                            v-for="(btn, idx) in item.buttons"
                            :key="idx"
                            :type="
                                resolvedButtonColor(btn.color, idx) === 'standard'
                                    ? 'base'
                                    : 'colored'
                            "
                            :color="
                                resolvedButtonColor(btn.color, idx) === 'standard'
                                    ? undefined
                                    : resolvedButtonColor(btn.color, idx)
                            "
                            @click="handleButtonClick(item.id, btn)"
                        >
                            <component :is="btn.icon" v-if="btn.icon" />
                            {{ btn.label }}
                        </Button>
                    </div>
                </div>
            </div>
        </transition-group>
    </div>
</template>

<script setup lang="ts">
import {
    CheckCircleIcon,
    DownloadIcon,
    InfoIcon,
    IssuesIcon,
    MinecraftServerIcon,
    XCircleIcon,
    XIcon,
} from '@modrinth/assets'
import { computed, ref } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { useModalStack } from '../../composables/modal-stack'
import {
    injectPopupNotificationManager,
    type PopupNotification,
    type PopupNotificationButton,
    type PopupNotificationProgressItem,
} from '../../providers'
import Button from '../base/buttons/Button.vue'
import ProgressBar from '../base/ProgressBar.vue'
import NotificationToast from '../notifications/NotificationToast.vue'

const popupNotificationManager = injectPopupNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    exportErrorLogs: {
        id: 'notification.panel.export-error-logs',
        defaultMessage: 'Export error logs',
    },
    dismissNotification: {
        id: 'button.dismiss-notification',
        defaultMessage: 'Dismiss notification',
    },
})
const notifications = computed<PopupNotification[]>(() =>
    [...popupNotificationManager.getNotifications()]
        .filter((notification) => !notification.collapsed)
        .sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0)),
)
const { stackCount } = useModalStack()
const hasModalActive = computed(() => stackCount.value > 0)
const notificationGroupStyle = computed(() => ({
    zIndex: hasModalActive.value ? 100 + stackCount.value * 10 + 8 : 200,
}))
const exporting = ref<Record<string | number, boolean>>({})
const stopTimer = (n: PopupNotification) => popupNotificationManager.stopNotificationTimer(n)
const setNotificationTimer = (n: PopupNotification) =>
    popupNotificationManager.setNotificationTimer(n)
const dismiss = (id: string | number) => popupNotificationManager.removeNotification(id)

function isDownloadNotification(item: PopupNotification) {
    return (
        item.type === 'download' &&
        (!!item.progressItems?.length || item.progress != null || item.waiting)
    )
}

function downloadToastItems(item: PopupNotification): PopupNotificationProgressItem[] {
    const fallbackItem: PopupNotificationProgressItem = {
        id: `${item.id}`,
        title: item.title,
        text: item.text,
        iconUrl: item.iconUrl,
        progress: item.progress ?? 0,
        waiting: item.waiting ?? false,
        showProgress: true,
        progressType: 'percentage',
    }
    const items = item.progressItems?.length ? item.progressItems : [fallbackItem]

    return items
}

function handleDownloadClick(item: PopupNotification, event: MouseEvent) {
    if ((event.target as HTMLElement).closest('button')) return
    void item.onClick?.()
}

async function handleProgressItemDismiss(
    item: PopupNotification,
    progressItem: PopupNotificationProgressItem,
) {
    try {
        await progressItem.onDismiss?.()
    } catch {
        return
    }
    if (item.progressItems?.length) {
        item.progressItems = item.progressItems.filter(
            (candidate) => candidate.id !== progressItem.id,
        )
        if (item.progressItems.length > 0) return
    }
    popupNotificationManager.removeNotification(item.id)
}

async function handleProgressItemAction(
    progressItem: PopupNotificationProgressItem,
    index: number,
) {
    const button = progressItem.buttons?.[index]
    if (button) {
        await handleProgressItemButtonClick(progressItem, button)
    }
}

async function handleProgressItemButtonClick(
    progressItem: PopupNotificationProgressItem,
    btn: PopupNotificationButton,
) {
    await btn.action()
    if (!btn.keepOpen) {
        await progressItem.onDismiss?.()
    }
}

function resolvedButtonColor(color: PopupNotificationButton['color'], index: number) {
    return color ?? (index === 0 ? 'brand' : 'standard')
}

async function handleButtonClick(id: string | number, btn: PopupNotificationButton) {
    await btn.action()
    if (!btn.keepOpen) {
        popupNotificationManager.removeNotification(id)
    }
}

async function handleToastAction(item: PopupNotification, action?: () => void | Promise<void>) {
    try {
        await action?.()
    } catch {
        return
    }
    popupNotificationManager.removeNotification(item.id)
}

async function handleErrorAction(notification: PopupNotification): Promise<void> {
    if (!onErrorAction || exporting.value[notification.id]) return

    exporting.value[notification.id] = true
    try {
        await onErrorAction(notification)
    } finally {
        const { [notification.id]: _, ...rest } = exporting.value
        exporting.value = rest as Record<string | number, boolean>
    }
}

function progressColorForType(type: PopupNotification['type']) {
    if (type === 'error') {
        return 'red'
    } else if (type === 'warning') {
        return 'orange'
    } else if (type === 'download') {
        return 'green'
    } else if (type === 'success') {
        return 'green'
    } else if (type === 'info') {
        return 'blue'
    }
    return 'green'
}

const {
    hasSidebar = false,
    onErrorAction,
    errorActionLabel,
} = defineProps<{
    hasSidebar?: boolean
    onErrorAction?: (notification: PopupNotification) => void | Promise<void>
    errorActionLabel?: string
}>()
</script>

<style scoped>
.popup-notification-group {
    position: fixed;
    top: calc(var(--top-bar-height, 3rem) + 1.5rem);
    right: 1.5rem;
    z-index: 200;
    width: min(420px, calc(100vw - 1.5rem));
    min-width: min(420px, calc(100vw - 1.5rem));
    max-width: min(420px, calc(100vw - 1.5rem));
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    transition:
        opacity 0.2s ease-in-out,
        transform 0.2s ease-in-out;
}

.popup-notification-group.has-sidebar {
    right: calc(var(--right-bar-width, 0px) + 1.5rem);
}

.popup-notification-group.has-modal {
    opacity: 0;
    pointer-events: none;
    transform: translateY(-0.5rem);
}

@media screen and (max-width: 500px) {
    .popup-notification-group {
        right: 0.75rem;
    }
}

.popup-notification-group .popup-notification-wrapper {
    width: 100%;
}

.popup-notifs-enter-active,
.popup-notifs-leave-active,
.popup-notifs-move {
    transition: all 0.3s ease-in-out;
}

.popup-notifs-enter-from {
    opacity: 0;
    transform: translateX(100%) scale(0.8);
}

.popup-notifs-leave-to {
    opacity: 0;
    transform: translateX(100%) scale(0.8);
}
</style>
