<script setup lang="ts">
import { PlugIcon, ShieldIcon, XIcon } from '@modrinth/assets'
import { useVIntl } from '@modrinth/ui'

import { useMcpConnection } from './connection'
import { mcpMessages as messages } from './messages'

const { connectedClient, fullAccess, disconnect } = useMcpConnection()
const { formatMessage } = useVIntl()
</script>

<template>
    <button
        v-if="connectedClient"
        type="button"
        data-tauri-drag-region-exclude
        data-testid="mcp-connection-status"
        :aria-label="formatMessage(messages.disconnect, { client: connectedClient })"
        :title="formatMessage(messages.disconnect, { client: connectedClient })"
        class="mx-1 inline-flex h-9 shrink-0 cursor-pointer items-center gap-2 whitespace-nowrap rounded-xl border border-solid border-[var(--color-brand)] bg-surface-3 px-2.5 text-sm font-semibold text-[var(--color-text-primary)] hover:bg-surface-4 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
        @click="disconnect"
    >
        <ShieldIcon
            v-if="fullAccess"
            aria-hidden="true"
            data-testid="mcp-full-access-shield"
            class="size-4 shrink-0 text-orange-300"
        />
        <PlugIcon v-else aria-hidden="true" class="size-4 shrink-0 text-brand" />
        <span role="status">{{ formatMessage(messages.connected) }}</span>
        <XIcon aria-hidden="true" class="size-3.5 shrink-0 text-[var(--color-text-tertiary)]" />
    </button>
</template>
