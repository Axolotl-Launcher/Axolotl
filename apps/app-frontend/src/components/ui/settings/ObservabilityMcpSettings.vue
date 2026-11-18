<script setup lang="ts">
import { Button, Toggle, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { observabilityMcpMessages as messages } from '@/features/observability-mcp/messages'
import { useObservabilityMcpRuntime } from '@/features/observability-mcp/runtime'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { state, loaded, busy, error, configure, release } = useObservabilityMcpRuntime()
onBeforeUnmount(release)

const endpoint = computed(() => state.value.endpoint ?? 'MCP_URL')
const bearer = computed(() => state.value.token ?? 'MCP_TOKEN')
const tokenCopied = ref(false)
const copyError = ref(false)
watch(
    () => state.value.token,
    () => {
        tokenCopied.value = false
        copyError.value = false
    },
)

async function copyToken() {
    if (!state.value.token) return
    copyError.value = false
    try {
        await navigator.clipboard.writeText(state.value.token)
        tokenCopied.value = true
    } catch {
        copyError.value = true
    }
}

const clients = computed(
    () =>
        [
            {
                name: 'Claude Code',
                instructions: messages.claudeStep,
                code: `claude mcp add --transport http observability "${endpoint.value}"${state.value.require_auth ? ` --header "Authorization: Bearer ${bearer.value}"` : ''}`,
                url: 'https://code.claude.com/docs/en/mcp',
            },
            {
                name: 'Codex',
                instructions: messages.codexStep,
                code: [
                    '[mcp_servers.observability]',
                    `url = ${JSON.stringify(endpoint.value)}`,
                    'startup_timeout_sec = 60',
                    ...(state.value.require_auth
                        ? [
                              `http_headers = { Authorization = ${JSON.stringify(`Bearer ${bearer.value}`)} }`,
                          ]
                        : []),
                ].join('\n'),
                url: 'https://developers.openai.com/codex/mcp/',
            },
        ] as const,
)
</script>

<template>
    <div class="flex min-w-0 flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.enabled)"
            title-id="settings-target-observability-mcp-enabled"
        >
            <SettingsRow inline-control>
                <template #label
                    ><label for="observability-mcp-enabled">{{
                        formatMessage(messages.enabled)
                    }}</label></template
                >
                <template #description>{{ formatMessage(messages.enabledDescription) }}</template>
                <template #control
                    ><Toggle
                        id="observability-mcp-enabled"
                        :model-value="state.enabled"
                        :disabled="!loaded || busy"
                        @update:model-value="
                            (enabled) => configure(enabled ?? false, state.require_auth)
                        "
                /></template>
            </SettingsRow>
            <div class="flex flex-col gap-2 px-4 pb-4">
                <p role="status" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{
                        formatMessage(
                            state.running
                                ? messages.running
                                : state.enabled
                                  ? messages.notRunning
                                  : messages.disabled,
                        )
                    }}
                </p>
                <template v-if="state.running">
                    <code class="break-all text-sm text-[var(--color-text-primary)]">{{
                        state.endpoint
                    }}</code>
                    <template v-if="state.require_auth">
                        <label class="flex flex-col gap-1 text-sm text-[var(--color-text-tertiary)]"
                            >{{ formatMessage(messages.token) }}
                            <input
                                type="password"
                                readonly
                                :value="state.token ?? ''"
                                class="rounded-lg border border-solid border-surface-4 bg-surface-2 p-2 text-[var(--color-text-primary)]"
                                @focus="($event.target as HTMLInputElement).select()"
                            />
                        </label>
                        <div class="flex flex-wrap items-center gap-2">
                            <Button :disabled="!state.token" @click="copyToken">{{
                                formatMessage(
                                    tokenCopied ? messages.tokenCopied : messages.copyToken,
                                )
                            }}</Button>
                            <span v-if="copyError" role="alert" class="text-sm text-red">{{
                                formatMessage(messages.copyFailed)
                            }}</span>
                        </div>
                    </template>
                </template>
                <p v-if="error" role="alert" class="m-0 break-words text-sm text-red">
                    {{ error }}
                </p>
            </div>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.requireAuth)"
            title-id="settings-target-observability-mcp-access"
        >
            <SettingsRow inline-control>
                <template #label
                    ><label for="observability-mcp-require-auth">{{
                        formatMessage(messages.requireAuth)
                    }}</label></template
                >
                <template #description>{{
                    formatMessage(messages.requireAuthDescription)
                }}</template>
                <template #control
                    ><Toggle
                        id="observability-mcp-require-auth"
                        :model-value="state.require_auth"
                        :disabled="!loaded || busy"
                        @update:model-value="
                            (required) => configure(state.enabled, required ?? false)
                        "
                /></template>
            </SettingsRow>
            <p class="m-0 px-4 pb-4 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.privacyNotice) }}
            </p>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.debugTitle)"
            title-id="settings-target-observability-mcp-clients"
        >
            <p class="m-0 px-4 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.debugDescription) }}
            </p>
            <p
                v-if="!state.running"
                class="m-0 px-4 pb-4 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.enableFirst) }}
            </p>
            <div v-else class="flex flex-col gap-4 px-4 pb-4">
                <div
                    v-for="client in clients"
                    :key="client.name"
                    class="flex flex-col gap-2 rounded-xl bg-surface-2 p-4"
                >
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        client.name
                    }}</span>
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(client.instructions) }}
                    </p>
                    <code
                        class="break-all whitespace-pre-wrap text-sm text-[var(--color-text-primary)]"
                        >{{ client.code }}</code
                    >
                </div>
            </div>
        </SettingsSection>
    </div>
</template>
