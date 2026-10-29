<script setup lang="ts">
import { ExternalIcon, ShieldCheckIcon, ShieldIcon } from '@modrinth/assets'
import { Button, Checkbox, NewModal, Toggle, useVIntl } from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { mcpMessages as messages } from '@/features/mcp/messages'
import { useMcpRuntime } from '@/features/mcp/runtime'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { state, loaded, busy, error, configure } = useMcpRuntime()
const fullAccess = computed(() => state.value.full_access)
const endpoint = computed(() => state.value.endpoint ?? 'MCP_URL')
const bearer = computed(() => state.value.token ?? 'MCP_TOKEN')
const authorizationHeaders = computed(() =>
    state.value.require_auth ? { Authorization: `Bearer ${bearer.value}` } : undefined,
)
const warningModal = ref<InstanceType<typeof NewModal>>()
const acknowledged = ref(false)
const selectedClient = ref(0)
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
                name: 'Codex',
                instructions: messages.codexStep,
                code: [
                    '[mcp_servers.axolotl]',
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
            {
                name: 'Claude Code',
                instructions: messages.claudeStep,
                code: `claude mcp add --transport http axolotl "${endpoint.value}"${state.value.require_auth ? ` --header "Authorization: Bearer ${bearer.value}"` : ''}`,
                url: 'https://code.claude.com/docs/en/mcp',
            },
            {
                name: 'Cursor',
                instructions: messages.cursorStep,
                code: JSON.stringify(
                    {
                        mcpServers: {
                            axolotl: {
                                url: endpoint.value,
                                headers: authorizationHeaders.value,
                            },
                        },
                    },
                    null,
                    2,
                ),
                url: 'https://cursor.com/docs/mcp',
            },
            {
                name: 'VS Code',
                instructions: messages.vscodeStep,
                code: JSON.stringify(
                    {
                        servers: {
                            axolotl: {
                                type: 'http',
                                url: endpoint.value,
                                headers: authorizationHeaders.value,
                            },
                        },
                    },
                    null,
                    2,
                ),
                url: 'https://code.visualstudio.com/docs/agent-customization/mcp-servers',
            },
        ] as const,
)

function changeFullAccess(enabled: boolean) {
    if (!enabled) {
        void configure(state.value.enabled, false)
        return
    }
    acknowledged.value = false
    warningModal.value?.show()
}

async function enableFullAccess() {
    if (!acknowledged.value) return
    if (await configure(state.value.enabled, true)) warningModal.value?.hide()
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.title)"
            title-id="settings-target-mcp-enabled"
        >
            <SettingsRow inline-control>
                <template #label
                    ><label for="mcp-enabled">{{
                        formatMessage(messages.enabled)
                    }}</label></template
                >
                <template #description>{{ formatMessage(messages.enabledDescription) }}</template>
                <template #control
                    ><Toggle
                        id="mcp-enabled"
                        :model-value="state.enabled"
                        :disabled="!loaded || busy"
                        @update:model-value="(enabled) => configure(enabled, fullAccess)"
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
                <Button
                    v-if="state.enabled && !state.running"
                    :disabled="busy"
                    @click="configure(true, fullAccess)"
                    >{{ formatMessage(messages.retry) }}</Button
                >
            </div>
        </SettingsSection>
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
                <template #label
                    ><label for="mcp-require-auth">{{
                        formatMessage(messages.requireAuth)
                    }}</label></template
                >
                <template #description>{{
                    formatMessage(messages.requireAuthDescription)
                }}</template>
                <template #control
                    ><Toggle
                        id="mcp-require-auth"
                        :model-value="state.require_auth"
                        :disabled="!loaded || busy"
                        @update:model-value="
                            (required) => configure(state.enabled, fullAccess, required)
                        "
                /></template>
            </SettingsRow>
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
                        :disabled="!loaded || busy"
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
                    <p v-if="!state.running" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.enableFirst) }}
                    </p>
                    <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.connectionHelp) }}
                    </p>
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
        :disable-close="busy"
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
            <p v-if="error" role="alert" class="m-0 text-sm text-red">{{ error }}</p>
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button @click="warningModal?.hide()">{{ formatMessage(messages.cancel) }}</Button>
                <Button
                    type="colored"
                    color="orange"
                    :disabled="!acknowledged || busy"
                    data-testid="mcp-enable-full-access"
                    @click="enableFullAccess"
                    >{{ formatMessage(messages.enableFullAccess) }}</Button
                >
            </div>
        </template>
    </NewModal>
</template>
