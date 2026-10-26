<script setup lang="ts">
import { ExternalIcon, ShieldCheckIcon, ShieldIcon } from '@modrinth/assets'
import { Button, Checkbox, NewModal, Toggle, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { mcpMessages as messages } from '@/features/mcp/messages'
import { useMcpPreview } from '@/features/mcp/preview'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { fullAccess, setFullAccess } = useMcpPreview()
const warningModal = ref<InstanceType<typeof NewModal>>()
const acknowledged = ref(false)
const selectedClient = ref(0)

const clients = [
    {
        name: 'Codex',
        instructions: messages.codexStep,
        code: 'codex mcp add axolotl --url "MCP_URL"',
        url: 'https://developers.openai.com/codex/mcp/',
    },
    {
        name: 'Claude Code',
        instructions: messages.claudeStep,
        code: 'claude mcp add --transport http axolotl "MCP_URL"',
        url: 'https://code.claude.com/docs/en/mcp',
    },
    {
        name: 'Cursor',
        instructions: messages.cursorStep,
        code: JSON.stringify({ mcpServers: { axolotl: { url: 'MCP_URL' } } }, null, 2),
        url: 'https://cursor.com/docs/mcp',
    },
    {
        name: 'VS Code',
        instructions: messages.vscodeStep,
        code: null,
        url: 'https://code.visualstudio.com/docs/agent-customization/mcp-servers',
    },
] as const

function changeFullAccess(enabled: boolean) {
    if (!enabled) {
        setFullAccess(false)
        return
    }
    acknowledged.value = false
    warningModal.value?.show()
}

function enableFullAccess() {
    if (!acknowledged.value) return
    setFullAccess(true)
    warningModal.value?.hide()
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-6">
        <div class="flex flex-col gap-2">
            <p class="m-0 text-base leading-relaxed text-[var(--color-text-primary)]">
                {{ formatMessage(messages.introduction) }}
            </p>
        </div>

        <SettingsSection
            :title="formatMessage(messages.accessTitle)"
            title-id="settings-target-mcp-access"
        >
            <SettingsRow inline-control>
                <template #label>
                    <label for="mcp-full-access">{{ formatMessage(messages.fullAccess) }}</label>
                </template>
                <template #description>
                    <span id="mcp-full-access-description">{{
                        formatMessage(messages.fullAccessDescription)
                    }}</span>
                </template>
                <template #control>
                    <Toggle
                        id="mcp-full-access"
                        :model-value="fullAccess"
                        aria-describedby="mcp-full-access-description"
                        @update:model-value="changeFullAccess"
                    />
                </template>
            </SettingsRow>
            <div class="flex items-start gap-3 p-4">
                <component
                    :is="fullAccess ? ShieldIcon : ShieldCheckIcon"
                    aria-hidden="true"
                    class="mt-0.5 size-5 shrink-0"
                    :class="fullAccess ? 'text-orange' : 'text-[var(--color-text-tertiary)]'"
                />
                <div class="flex min-w-0 flex-col gap-2">
                    <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(fullAccess ? messages.fullMode : messages.standardMode) }}
                    </h3>
                    <p
                        v-if="fullAccess"
                        class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.fullModeNotice) }}
                    </p>
                    <ul
                        v-else
                        class="m-0 flex flex-col gap-1 pl-4 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
                    >
                        <li>{{ formatMessage(messages.manualApproval) }}</li>
                        <li>{{ formatMessage(messages.noDeletion) }}</li>
                        <li>{{ formatMessage(messages.toolsOnly) }}</li>
                    </ul>
                    <p class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.disconnectHint) }}
                    </p>
                </div>
            </div>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.setupTitle)"
            :description="formatMessage(messages.setupDescription)"
            title-id="settings-target-mcp-clients"
        >
            <div class="flex min-w-0 flex-col gap-4 p-4">
                <div
                    role="group"
                    :aria-label="formatMessage(messages.clientLabel)"
                    class="flex flex-wrap gap-2"
                >
                    <Button
                        v-for="(client, index) in clients"
                        :key="client.name"
                        :type="selectedClient === index ? 'highlight' : 'base'"
                        :aria-pressed="selectedClient === index"
                        @click="selectedClient = index"
                    >
                        {{ client.name }}
                    </Button>
                </div>
                <div class="flex min-w-0 flex-col gap-3">
                    <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ clients[selectedClient].name }}
                    </h3>
                    <p class="m-0 text-sm leading-relaxed text-[var(--color-text-primary)]">
                        {{ formatMessage(clients[selectedClient].instructions) }}
                    </p>
                    <pre
                        v-if="clients[selectedClient].code"
                        class="m-0 overflow-x-auto rounded-lg border border-solid border-surface-4 bg-surface-2 p-3 text-sm leading-relaxed text-[var(--color-text-primary)]"
                    ><code>{{ clients[selectedClient].code }}</code></pre>
                    <p class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.afterSetup) }}
                    </p>
                    <a
                        :href="clients[selectedClient].url"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex w-fit items-center gap-1.5 text-sm font-medium text-[var(--color-text-primary)] underline decoration-[var(--surface-5)] underline-offset-4 hover:decoration-current"
                    >
                        {{ formatMessage(messages.officialGuide) }}
                        <ExternalIcon aria-hidden="true" class="size-4" />
                    </a>
                </div>
            </div>
        </SettingsSection>
    </div>

    <NewModal
        ref="warningModal"
        :header="formatMessage(messages.warningTitle)"
        width="min(32rem, calc(100vw - 2rem))"
        fade="warning"
        :on-hide="() => (acknowledged = false)"
    >
        <div class="flex flex-col gap-4">
            <p class="m-0 text-base leading-relaxed text-[var(--color-text-primary)]">
                {{ formatMessage(messages.warningDescription) }}
            </p>
            <p class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.warningSession) }}
            </p>
            <Checkbox v-model="acknowledged" :label="formatMessage(messages.acknowledge)" />
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button @click="warningModal?.hide()">{{ formatMessage(messages.cancel) }}</Button>
                <Button
                    type="colored"
                    color="orange"
                    :disabled="!acknowledged"
                    data-testid="mcp-enable-full-access"
                    @click="enableFullAccess"
                    >{{ formatMessage(messages.enableFullAccess) }}</Button
                >
            </div>
        </template>
    </NewModal>
</template>
