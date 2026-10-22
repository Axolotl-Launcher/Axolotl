<script setup lang="ts">
import {
    DownloadIcon,
    PlayIcon,
    RefreshCwIcon,
    SpinnerIcon,
    StopCircleIcon,
} from '@modrinth/assets'
import { Button, defineMessages, TagItem, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import {
    isServerStatusVisible,
    SERVER_STATUS_META,
} from '@/components/multiplayer/servers/server-status'
import ServerIcon from '@/components/multiplayer/servers/ServerIcon.vue'
import { serverSetupStatus } from '@/composables/useServerInstalls'
import type { ServerView } from '@/composables/useServers'

const props = defineProps<{
    server: ServerView
    variant: 'standard' | 'library'
}>()

const emit = defineEmits<{
    open: []
    'start-stop': []
    resume: []
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    start: { id: 'app.servers.action.start', defaultMessage: 'Start' },
    stop: { id: 'app.servers.action.stop', defaultMessage: 'Stop' },
    continueDownload: {
        id: 'app.servers.action.continue-download',
        defaultMessage: 'Continue download',
    },
    retryDownload: { id: 'app.servers.action.retry-download', defaultMessage: 'Retry download' },
    downloading: { id: 'app.servers.status.downloading', defaultMessage: 'Downloading' },
    downloadInterrupted: {
        id: 'app.servers.status.download-interrupted',
        defaultMessage: 'Download interrupted',
    },
    downloadFailed: { id: 'app.servers.status.download-failed', defaultMessage: 'Download failed' },
})

const statusMeta = computed(() => SERVER_STATUS_META[props.server.status])

const setupStatus = computed(() => serverSetupStatus(props.server))

/** Setup states take precedence over the runtime status tag. */
const displayTag = computed(() => {
    switch (setupStatus.value) {
        case 'installing':
            return { label: messages.downloading, color: 'text-orange' }
        case 'interrupted':
            return { label: messages.downloadInterrupted, color: 'text-orange' }
        case 'failed':
            return { label: messages.downloadFailed, color: 'text-red' }
        default:
            return isServerStatusVisible(props.server.status)
                ? { label: statusMeta.value.label, color: statusMeta.value.color }
                : null
    }
})

const setupTooltip = computed(() => {
    if (setupStatus.value === 'interrupted') return formatMessage(messages.continueDownload)
    if (setupStatus.value === 'failed') return formatMessage(messages.retryDownload)
    return formatMessage(messages.downloading)
})
</script>

<template>
    <div
        v-if="variant === 'library'"
        data-onboarding-id="server-card"
        role="button"
        tabindex="0"
        class="group relative flex w-full cursor-pointer select-none flex-col items-start justify-end gap-3 overflow-clip rounded-[20px] border border-solid border-surface-4 bg-surface-3 p-3 text-left transition-[border-color,filter,transform] hover:border-surface-5 hover:brightness-110 active:scale-[0.98]"
        @click="emit('open')"
        @keydown.enter="emit('open')"
        @keydown.space.prevent="emit('open')"
    >
        <div
            class="relative flex aspect-square w-full shrink-0 items-center justify-center overflow-clip rounded-2xl bg-surface-2"
        >
            <ServerIcon
                :icon-path="server.iconPath"
                :server-type="server.serverType"
                :server-id="server.id"
                size="96px"
            />
            <TagItem v-if="displayTag" class="absolute left-3 top-3">
                <span :class="'font-semibold ' + displayTag.color">
                    {{ formatMessage(displayTag.label) }}
                </span>
            </TagItem>
            <div class="absolute bottom-1.5 right-1.5" @click.stop @keydown.stop>
                <div
                    v-if="setupStatus === 'installing'"
                    v-tooltip="setupTooltip"
                    class="flex size-10 items-center justify-center rounded-full border border-solid border-surface-5 bg-surface-3"
                >
                    <SpinnerIcon class="size-5 animate-spin text-orange" />
                </div>
                <Button
                    v-else-if="setupStatus === 'interrupted'"
                    v-tooltip="setupTooltip"
                    type="colored"
                    color="brand"
                    size="xl"
                    circular
                    icon-only
                    class="scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('resume')"
                    ><DownloadIcon />
                </Button>
                <Button
                    v-else-if="setupStatus === 'failed'"
                    v-tooltip="setupTooltip"
                    type="colored"
                    color="red"
                    size="xl"
                    circular
                    icon-only
                    class="scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('resume')"
                    ><RefreshCwIcon />
                </Button>
                <Button
                    v-else-if="server.status !== 'running'"
                    v-tooltip="formatMessage(messages.start)"
                    type="colored"
                    color="brand"
                    size="xl"
                    circular
                    icon-only
                    class="scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('start-stop')"
                    ><PlayIcon />
                </Button>
                <Button
                    v-else
                    v-tooltip="formatMessage(messages.stop)"
                    type="colored"
                    color="red"
                    size="xl"
                    circular
                    icon-only
                    class="scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('start-stop')"
                    ><StopCircleIcon />
                </Button>
            </div>
        </div>
        <div class="flex w-full min-w-0 flex-col items-start justify-center gap-1 px-0.5">
            <p
                class="m-0 w-full truncate text-base font-semibold leading-5 text-[var(--color-text-primary)]"
            >
                {{ server.name }}
            </p>
            <p
                class="m-0 w-full truncate text-sm font-medium leading-[18px] text-[var(--color-text-default)]"
            >
                {{ server.serverType }} {{ server.gameVersion }}
            </p>
        </div>
    </div>
    <div
        v-else
        data-onboarding-id="server-card"
        role="button"
        tabindex="0"
        class="group button-base flex w-full cursor-pointer select-none gap-3 rounded-xl border border-solid border-surface-4 bg-surface-2 p-4 text-left transition-[border-color,filter,transform] hover:border-surface-5 hover:brightness-110 active:scale-[0.98]"
        @click="emit('open')"
        @keydown.enter="emit('open')"
        @keydown.space.prevent="emit('open')"
    >
        <div class="relative flex size-12 shrink-0 items-center justify-center">
            <ServerIcon
                :icon-path="server.iconPath"
                :server-type="server.serverType"
                :server-id="server.id"
                size="48px"
                class="transition-all group-hover:brightness-75"
            />
            <div
                class="absolute inset-0 flex items-center justify-center"
                @click.stop
                @keydown.stop
            >
                <div
                    v-if="setupStatus === 'installing'"
                    v-tooltip="setupTooltip"
                    class="flex size-9 origin-bottom scale-75 items-center justify-center rounded-full border border-solid border-surface-5 bg-surface-3 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                >
                    <SpinnerIcon class="size-4 animate-spin text-orange" />
                </div>
                <Button
                    v-else-if="setupStatus === 'interrupted'"
                    v-tooltip="setupTooltip"
                    type="colored"
                    color="brand"
                    size="xl"
                    circular
                    icon-only
                    class="origin-bottom scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('resume')"
                    ><DownloadIcon />
                </Button>
                <Button
                    v-else-if="setupStatus === 'failed'"
                    v-tooltip="setupTooltip"
                    type="colored"
                    color="red"
                    size="xl"
                    circular
                    icon-only
                    class="origin-bottom scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('resume')"
                    ><RefreshCwIcon />
                </Button>
                <Button
                    v-else-if="server.status !== 'running'"
                    v-tooltip="formatMessage(messages.start)"
                    type="colored"
                    color="brand"
                    size="xl"
                    circular
                    icon-only
                    class="origin-bottom scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('start-stop')"
                    ><PlayIcon />
                </Button>
                <Button
                    v-else
                    v-tooltip="formatMessage(messages.stop)"
                    type="colored"
                    color="red"
                    size="xl"
                    circular
                    icon-only
                    class="origin-bottom scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100"
                    @click="emit('start-stop')"
                    ><StopCircleIcon />
                </Button>
            </div>
        </div>
        <div class="min-w-0 flex-1">
            <div class="flex min-w-0 items-center gap-2">
                <p
                    class="m-0 min-w-0 truncate text-base font-bold leading-tight text-[var(--color-text-primary)]"
                >
                    {{ server.name }}
                </p>
                <TagItem v-if="displayTag" class="shrink-0">
                    <span :class="'font-semibold ' + displayTag.color">
                        {{ formatMessage(displayTag.label) }}
                    </span>
                </TagItem>
            </div>
            <p class="m-0 mt-1 truncate text-sm font-semibold text-[var(--color-text-tertiary)]">
                {{ server.serverType }} {{ server.gameVersion }}
            </p>
        </div>
    </div>
</template>
