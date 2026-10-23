<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { serverSetupStatus } from '../composables/useServerInstalls.ts'
import type { ServerView } from '../composables/useServers.ts'
import { SERVER_STATUS_META } from './server-status.ts'

const props = defineProps<{ server: ServerView }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    stopping: { id: 'app.servers.action.stopping', defaultMessage: 'Stopping...' },
    downloading: { id: 'app.servers.status.downloading', defaultMessage: 'Downloading' },
    interrupted: {
        id: 'app.servers.status.download-interrupted',
        defaultMessage: 'Download interrupted',
    },
    failed: { id: 'app.servers.status.download-failed', defaultMessage: 'Download failed' },
})
const status = computed(() => {
    if (props.server.stopping) return { label: messages.stopping, color: 'text-orange' }
    switch (serverSetupStatus(props.server)) {
        case 'installing':
            return { label: messages.downloading, color: 'text-orange' }
        case 'interrupted':
            return { label: messages.interrupted, color: 'text-orange' }
        case 'failed':
            return { label: messages.failed, color: 'text-red' }
        default:
            return SERVER_STATUS_META[props.server.status]
    }
})
</script>

<template>
    <span class="inline-flex min-w-0 items-center gap-2 text-sm font-medium" :class="status.color">
        <span aria-hidden="true" class="size-1.5 shrink-0 rounded-full bg-current" />
        <span class="truncate">{{ formatMessage(status.label) }}</span>
    </span>
</template>
