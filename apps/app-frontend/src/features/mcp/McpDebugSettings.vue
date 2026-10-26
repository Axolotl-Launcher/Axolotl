<script setup lang="ts">
import { Button, useVIntl } from '@modrinth/ui'

import SettingsSection from '@/components/ui/settings/SettingsSection.vue'

import { mcpMessages as messages } from './messages'
import { useMcpPreview } from './preview'

const { connectedClient, pendingClient, simulateConnection, disconnect } = useMcpPreview()
const { formatMessage } = useVIntl()
</script>

<template>
    <SettingsSection
        :title="formatMessage(messages.debugTitle)"
        :description="formatMessage(messages.debugDescription)"
    >
        <div class="flex flex-col gap-4 p-4">
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
                    :disabled="!!connectedClient || !!pendingClient"
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
