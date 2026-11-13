<template>
    <div
        ref="groupEl"
        class="vue-notification-group"
        :class="{
            'intercom-present': isIntercomPresent,
            'location-left': notificationLocation === 'left',
            'location-right': notificationLocation === 'right',
            'has-sidebar': hasSidebar && !hasModalActive,
        }"
    >
        <transition-group name="notifs">
            <div
                v-for="(item, index) in notifications"
                :key="item.id"
                class="vue-notification-wrapper"
                @mouseenter="stopTimer(item)"
                @mouseleave="setNotificationTimer(item)"
            >
                <div class="flex w-full gap-2 overflow-hidden rounded-lg bg-surface-3 shadow-xl">
                    <div
                        class="w-2"
                        :class="{
                            'bg-red': item.type === 'error',
                            'bg-orange': item.type === 'warning',
                            'bg-green': item.type === 'success',
                            'bg-blue':
                                !item.type || !['error', 'warning', 'success'].includes(item.type),
                        }"
                    ></div>
                    <div
                        class="grid w-full grid-cols-[auto_1fr_auto] items-center gap-x-2 gap-y-1 py-2 pl-1 pr-3"
                    >
                        <div
                            class="flex items-center"
                            :class="{
                                'text-red': item.type === 'error',
                                'text-orange': item.type === 'warning',
                                'text-green': item.type === 'success',
                                'text-blue':
                                    !item.type ||
                                    !['error', 'warning', 'success'].includes(item.type),
                            }"
                        >
                            <IssuesIcon v-if="item.type === 'warning'" class="h-6 w-6" />
                            <CheckCircleIcon v-else-if="item.type === 'success'" class="h-6 w-6" />
                            <XCircleIcon v-else-if="item.type === 'error'" class="h-6 w-6" />
                            <InfoIcon v-else class="h-6 w-6" />
                        </div>
                        <div class="m-0 text-wrap font-bold text-[var(--color-text-primary)]">
                            {{ item.title }}
                        </div>
                        <div class="flex items-center gap-1">
                            <div
                                v-if="item.count && item.count > 1"
                                class="text-xs font-bold text-[var(--color-text-primary)]"
                            >
                                x{{ item.count }}
                            </div>
                            <Button
                                v-tooltip="
                                    item.supportData
                                        ? formatMessage(messages.copySupport)
                                        : formatMessage(messages.copy)
                                "
                                size="2xs"
                                circular
                                icon-only
                                :label="
                                    item.supportData
                                        ? formatMessage(messages.copySupport)
                                        : formatMessage(messages.copy)
                                "
                                @click="copyToClipboard(item)"
                                ><CheckIcon v-if="copied[getCopyKey(item)]" />
                                <CopyIcon v-else />
                            </Button>
                            <Button
                                v-if="item.type === 'error' && onErrorAction"
                                v-tooltip="
                                    errorActionLabel || formatMessage(messages.exportErrorLogs)
                                "
                                size="2xs"
                                circular
                                icon-only
                                :label="errorActionLabel || formatMessage(messages.exportErrorLogs)"
                                :disabled="exporting[item.id]"
                                @click="handleErrorAction(item)"
                                ><DownloadIcon />
                            </Button>
                            <Button
                                v-tooltip="formatMessage(messages.dismiss)"
                                size="2xs"
                                circular
                                icon-only
                                :label="formatMessage(messages.dismiss)"
                                @click="dismissNotification(index)"
                                ><XIcon />
                            </Button>
                        </div>
                        <div></div>
                        <div
                            class="col-span-2 whitespace-pre-line break-words text-sm leading-5 text-[var(--color-text-default)] line-clamp-5"
                        >
                            {{ item.text }}
                        </div>
                        <template v-if="item.errorCode">
                            <div></div>
                            <div
                                class="m-0 text-wrap text-xs font-medium text-[var(--color-text-tertiary)]"
                            >
                                {{ item.errorCode }}
                            </div>
                        </template>
                    </div>
                </div>
            </div>
        </transition-group>
    </div>
</template>

<script setup lang="ts">
import {
    CheckCircleIcon,
    CheckIcon,
    CopyIcon,
    DownloadIcon,
    InfoIcon,
    IssuesIcon,
    XCircleIcon,
    XIcon,
} from '@modrinth/assets'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { useModalStack } from '#ui/composables/modal-stack.ts'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { injectNotificationManager, type WebNotification } from '../../providers'
import Button from '../base/buttons/Button.vue'

const notificationManager = injectNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    copySupport: {
        id: 'notification.panel.copy-support',
        defaultMessage: 'Copy error details for support',
    },
    copy: { id: 'notification.panel.copy', defaultMessage: 'Copy to clipboard' },
    dismiss: { id: 'notification.panel.dismiss', defaultMessage: 'Dismiss' },
    exportErrorLogs: {
        id: 'notification.panel.export-error-logs',
        defaultMessage: 'Export error logs',
    },
})
const notifications = computed<WebNotification[]>(() =>
    [...notificationManager.getNotifications()]
        .filter((notification) => !notification.collapsed)
        .sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0)),
)
const notificationLocation = computed(() => notificationManager.getNotificationLocation())
const groupEl = ref<HTMLElement | null>(null)

const isIntercomPresent = ref<boolean>(false)
const copied = ref<Record<string, boolean>>({})
const exporting = ref<Record<string | number, boolean>>({})

const stopTimer = (n: WebNotification) => notificationManager.stopNotificationTimer(n)
const setNotificationTimer = (n: WebNotification) => notificationManager.setNotificationTimer(n)
const dismissNotification = (n: number) => {
    const notification = notifications.value[n]
    if (notification) notificationManager.removeNotification(notification.id)
}

function createNotifText(notif: WebNotification): string {
    return [notif.title, notif.text, notif.errorCode].filter(Boolean).join('\n')
}

function getCopyKey(notif: WebNotification): string {
    return notif.supportData ? `support-${notif.id}` : createNotifText(notif)
}

function checkIntercomPresence(): void {
    isIntercomPresent.value = !!document.querySelector('.intercom-lightweight-app')
}

let appliedClearance: string | undefined

function updateNotificationClearance(): void {
    const rect = groupEl.value?.getBoundingClientRect()
    const visible = notificationLocation.value === 'right' && rect && rect.height > 0
    // Measure the panel's top edge rather than its height: the panel can be raised
    // by the mobile navbar, notifications' own bottom offset or the intercom bubble.
    const clearance = visible ? `${Math.ceil(window.innerHeight - rect.top + 16)}px` : '0px'
    // Writing the custom property invalidates style for everything that reads it,
    // so skip the write when the measured clearance has not changed.
    if (clearance === appliedClearance) return
    appliedClearance = clearance
    document.documentElement.style.setProperty('--notification-panel-clearance', clearance)
}

function copyToClipboard(notif: WebNotification): void {
    // If supportData is present, copy the full JSON for support; otherwise copy plain text
    const text = notif.supportData
        ? JSON.stringify(notif.supportData, null, 2)
        : createNotifText(notif)

    const key = getCopyKey(notif)
    copied.value[key] = true
    navigator.clipboard.writeText(text)

    setTimeout(() => {
        const { [key]: _, ...rest } = copied.value
        copied.value = rest
    }, 2000)
}

async function handleErrorAction(notification: WebNotification): Promise<void> {
    if (!onErrorAction || exporting.value[notification.id]) return

    exporting.value[notification.id] = true
    try {
        await onErrorAction(notification)
    } finally {
        const { [notification.id]: _, ...rest } = exporting.value
        exporting.value = rest as Record<string | number, boolean>
    }
}

onMounted(() => {
    checkIntercomPresence()

    // Measuring the panel forces a synchronous layout flush, so coalesce every
    // trigger into one read per frame instead of one per observed mutation.
    let frame: number | undefined
    const handleLayoutChange = () => {
        if (frame !== undefined) return
        frame = requestAnimationFrame(() => {
            frame = undefined
            checkIntercomPresence()
            updateNotificationClearance()
        })
    }
    const observer = new MutationObserver(handleLayoutChange)

    observer.observe(document.body, {
        childList: true,
        subtree: true,
    })

    // The panel and body classes (such as floating-action-bar-shown) can move it
    // without resizing it. Observe their attributes, but not descendant attributes
    // that would trigger measurements for unrelated changes throughout the app.
    const layoutObserver = new MutationObserver(handleLayoutChange)
    const layoutAttributes = { attributes: true, attributeFilter: ['class', 'style'] }
    layoutObserver.observe(document.body, layoutAttributes)
    if (groupEl.value) layoutObserver.observe(groupEl.value, layoutAttributes)

    const resizeObserver = new ResizeObserver(handleLayoutChange)
    if (groupEl.value) resizeObserver.observe(groupEl.value)
    groupEl.value?.addEventListener('transitionend', handleLayoutChange)
    window.addEventListener('resize', handleLayoutChange)
    watch(notificationLocation, handleLayoutChange)
    updateNotificationClearance()

    onBeforeUnmount(() => {
        if (frame !== undefined) cancelAnimationFrame(frame)
        observer.disconnect()
        layoutObserver.disconnect()
        resizeObserver.disconnect()
        groupEl.value?.removeEventListener('transitionend', handleLayoutChange)
        window.removeEventListener('resize', handleLayoutChange)
        appliedClearance = undefined
        document.documentElement.style.removeProperty('--notification-panel-clearance')
    })
})

const { hasModal: hasModalActive } = useModalStack()

const {
    hasSidebar = false,
    onErrorAction,
    errorActionLabel = 'Export error logs',
} = defineProps<{
    hasSidebar?: boolean
    onErrorAction?: (notification: WebNotification) => void | Promise<void>
    errorActionLabel?: string
}>()
</script>

<style lang="scss" scoped>
.vue-notification-group {
    position: fixed;
    bottom: 1.5rem;
    z-index: 200;
    width: 450px;
    transition: bottom 0.25s ease-in-out;

    &.location-right {
        right: 1.5rem;
        transition: right 0.25s ease-in-out;

        &.has-sidebar {
            right: 325px;
        }
    }

    &.location-left {
        left: 1.5rem;
    }

    @media screen and (max-width: 500px) {
        width: calc(100% - 0.75rem * 2);
        bottom: 0.75rem;

        &.location-right {
            right: 0.75rem;
            left: auto;
        }

        &.location-left {
            left: 0.75rem;
            right: auto;
        }
    }

    &.intercom-present {
        bottom: 5rem;
    }

    .vue-notification-wrapper {
        width: 100%;
        overflow: hidden;
        margin-bottom: 10px;

        &:last-child {
            margin: 0;
        }
    }

    @media screen and (max-width: 750px) {
        transition: bottom 0.25s ease-in-out;
        bottom: calc(var(--size-mobile-navbar-height) + 10px) !important;
    }
}

.notifs-enter-active,
.notifs-leave-active,
.notifs-move {
    transition: all 0.25s ease-in-out;
}
.notifs-enter-from,
.notifs-leave-to {
    opacity: 0;
}

.notifs-enter-from {
    transform: translateY(100%) scale(0.8);
}

.notifs-leave-to {
    .location-right & {
        transform: translateX(100%) scale(0.8);
    }

    .location-left & {
        transform: translateX(-100%) scale(0.8);
    }
}

body.floating-action-bar-shown .vue-notification-group {
    bottom: calc(90px);
}
</style>
