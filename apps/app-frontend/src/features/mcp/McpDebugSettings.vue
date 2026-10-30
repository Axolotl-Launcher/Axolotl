<script setup lang="ts">
import { Button, Toggle, useVIntl } from '@modrinth/ui'

import SettingsSection from '@/components/ui/settings/SettingsSection.vue'

import { mcpMessages as messages } from './messages'
import { useMcpPreview } from './preview'
import { useMcpRuntime } from './runtime'

const {
    connectedClient,
    pendingClient,
    fullAccess,
    setFullAccess,
    simulateConnection,
    disconnect,
} = useMcpPreview()
const runtime = useMcpRuntime()
const { formatMessage } = useVIntl()
</script>

<template>
    <SettingsSection
        :title="formatMessage(messages.debugTitle)"
        :description="formatMessage(messages.debugDescription)"
    >
        <div class="flex flex-col gap-4 p-4">
            <div class="flex items-center gap-2">
                <Toggle
                    id="mcp-debug-full-access"
                    :model-value="fullAccess"
                    @update:model-value="setFullAccess"
                />
                <label for="mcp-debug-full-access">{{
                    formatMessage(messages.debugFullAccess)
                }}</label>
            </div>
            <span role="status" class="text-sm font-medium text-[var(--color-text-primary)]">
                {{
                    formatMessage(
                        connectedClient
                            ? messages.debugActive
                            : pendingClient
                              ? messages.debugPending
                              : messages.debugIdle,
                    )
                }}
            </span>
            <div class="flex flex-wrap gap-2">
                <Button
                    :disabled="
                        !!connectedClient || !!pendingClient || !!runtime.state.value.connection
                    "
                    @click="simulateConnection(formatMessage(messages.debugClient))"
                >
                    {{ formatMessage(messages.debugConnect) }}
                </Button>
                <Button :disabled="!connectedClient" @click="disconnect">
                    {{ formatMessage(messages.debugFinish) }}
                </Button>
                <Button :disabled="!connectedClient && !pendingClient" @click="disconnect">
                    {{ formatMessage(messages.debugInterrupt) }}
                </Button>
            </div>
        </div>
    </SettingsSection>
</template>
