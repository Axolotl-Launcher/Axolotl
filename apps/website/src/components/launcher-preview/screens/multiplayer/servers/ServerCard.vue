<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { serverSetupStatus } from '../composables/useServerInstalls.ts'
import type { ServerView } from '../composables/useServers.ts'
import ServerIcon from './ServerIcon.vue'
import ServerStatusBadge from './ServerStatusBadge.vue'
import ServerTerminateButton from './ServerTerminateButton.vue'

const props = defineProps<{ server: ServerView; variant: 'standard' | 'library' }>()
const emit = defineEmits<{ open: []; 'start-stop': []; resume: [] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    manage: { id: 'app.servers.card.manage', defaultMessage: 'Manage {name}' },
    start: { id: 'app.servers.action.start', defaultMessage: 'Start' },
    stop: { id: 'app.servers.action.stop', defaultMessage: 'Stop' },
    stopping: { id: 'app.servers.action.stopping', defaultMessage: 'Stopping...' },
    continueDownload: {
        id: 'app.servers.action.continue-download',
        defaultMessage: 'Continue download',
    },
    retryDownload: { id: 'app.servers.action.retry-download', defaultMessage: 'Retry download' },
    port: { id: 'app.servers.card.port', defaultMessage: 'Port {port}' },
})
const action = computed(() => {
    if (props.server.running || props.server.starting)
        return {
            label: props.server.stopping ? messages.stopping : messages.stop,
            icon: StopCircleIcon,
            event: 'start-stop' as const,
            disabled: !!props.server.stopping,
            color: 'red' as const,
        }
    switch (serverSetupStatus(props.server)) {
        case 'installing':
            return null
        case 'interrupted':
            return {
                label: messages.continueDownload,
                icon: DownloadIcon,
                event: 'resume' as const,
                disabled: false,
                color: undefined,
            }
        case 'failed':
            return {
                label: messages.retryDownload,
                icon: RefreshCwIcon,
                event: 'resume' as const,
                disabled: false,
                color: undefined,
            }
        default:
            return {
                label: messages.start,
                icon: PlayIcon,
                event: 'start-stop' as const,
                disabled: false,
                color: undefined,
            }
    }
})
</script>

<template>
    <article
        data-onboarding-id="server-card"
        class="group relative flex min-w-0 flex-col rounded-xl border border-solid border-surface-4 bg-surface-2 transition-colors hover:border-surface-5"
    >
        <button
            type="button"
            class="absolute inset-0 cursor-pointer rounded-xl border-0 bg-transparent focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
            :aria-label="formatMessage(messages.manage, { name: server.name })"
            @click="emit('open')"
        />
        <div
            v-if="variant === 'library'"
            class="pointer-events-none m-3 mb-0 flex aspect-video items-center justify-center rounded-lg bg-surface-3"
        >
            <ServerIcon
                :icon-path="server.iconPath"
                :server-type="server.serverType"
                :server-id="server.id"
                size="72px"
            />
        </div>
        <div class="pointer-events-none flex min-w-0 flex-1 items-start gap-3 p-4">
            <ServerIcon
                v-if="variant === 'standard'"
                :icon-path="server.iconPath"
                :server-type="server.serverType"
                :server-id="server.id"
                size="40px"
            />
            <div class="min-w-0 flex-1">
                <h3
                    class="m-0 line-clamp-2 break-all text-base font-semibold leading-6 text-[var(--color-text-primary)]"
                    :title="server.name"
                >
                    {{ server.name }}
                </h3>
                <div
                    class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-[var(--color-text-tertiary)]"
                >
                    <span class="capitalize">{{ server.serverType }}</span>
                    <span aria-hidden="true">·</span>
                    <span>{{ server.gameVersion }}</span>
                </div>
                <p
                    v-if="server.port"
                    class="mb-0 mt-2 text-xs tabular-nums text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.port, { port: server.port }) }}
                </p>
            </div>
        </div>
        <div
            class="flex min-w-0 flex-wrap items-center justify-between gap-3 border-0 border-t border-solid border-surface-4 px-4 py-3"
        >
            <ServerStatusBadge :server="server" class="pointer-events-none" />
            <div class="relative ml-auto flex shrink-0 items-center gap-2">
                <Button
                    v-if="action"
                    class="relative shrink-0"
                    :type="action.color ? 'quiet' : 'outlined'"
                    :color="action.color"
                    size="sm"
                    :disabled="action.disabled"
                    @click="emit(action.event)"
                >
                    <component :is="action.icon" />{{ formatMessage(action.label) }}
                </Button>
                <ServerTerminateButton v-if="server.stopping" :server-id="server.id" />
            </div>
        </div>
    </article>
</template>
