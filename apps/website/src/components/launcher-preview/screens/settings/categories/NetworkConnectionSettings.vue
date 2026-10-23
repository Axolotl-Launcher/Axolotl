<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { useSettingsPage } from '../state'
import {
    createNetworkBackend,
    type NetworkConfig,
    type ProxyConfig,
} from './network/network-backend'
import {
    diagnosticReport,
    networkTargets,
    resultStatus,
    useNetworkDiagnostics,
} from './network/network-diagnostics'

/**
 * Connection tests (apps/app-frontend/src/components/ui/settings/NetworkConnectionSettings.vue).
 * Each service is probed in turn by the simulated `network_test` command, which
 * follows the launcher's current DNS, proxy and TLS settings ("Use launcher
 * proxy settings") or bypasses the proxy ("Direct connection").
 */
const { formatMessage } = useVIntl()
const { settings, preview } = useSettingsPage()

/**
 * `state.download_clients()`: the proxy saved by the resources page
 * (`proxy_get`, `system` until one is chosen), DNS over HTTPS and the TLS switch.
 */
function readNetworkConfig(): NetworkConfig {
    const stored = (settings as unknown as { proxy_config?: Partial<ProxyConfig> }).proxy_config
    const mode = stored?.mode === 'none' || stored?.mode === 'custom' ? stored.mode : 'system'
    return {
        proxy: { mode, url: stored?.url ?? '' },
        dohEnabled: settings.doh_enabled,
        ignoreSslErrors: settings.ignore_ssl_errors,
    }
}

const { mode, probes, busy, hasResults, start, stop } = useNetworkDiagnostics(
    createNetworkBackend(preview.clock, readNetworkConfig),
)
const copyState = ref<'idle' | 'copied' | 'failed'>('idle')
const options = computed(() => [
    { value: 'launcher', label: formatMessage(messages.appSettingsNetworkLauncher) },
    { value: 'direct', label: formatMessage(messages.appSettingsNetworkDirect) },
])

const statusMessages: Record<string, MessageDescriptor> = {
    idle: messages.appSettingsNetworkIdle,
    queued: messages.appSettingsNetworkQueued,
    running: messages.appSettingsNetworkRunning,
    done: messages.appSettingsNetworkSuccess,
    cancelled: messages.appSettingsNetworkCancelled,
    failed: messages.appSettingsNetworkFailed,
    success: messages.appSettingsNetworkSuccess,
    restricted: messages.appSettingsNetworkRestricted,
    http_error: messages.appSettingsNetworkHttpError,
}

const rows = computed(() =>
    networkTargets.map((target) => {
        const probe = probes.value[target.id]
        return {
            ...target,
            ...probe,
            status:
                probe.state === 'done' && probe.result ? resultStatus(probe.result) : probe.state,
        }
    }),
)

function errorMessage(error: string) {
    const known: Record<string, MessageDescriptor> = {
        timeout: messages.appSettingsNetworkErrorTimeout,
        connection: messages.appSettingsNetworkErrorConnection,
        body: messages.appSettingsNetworkErrorBody,
        request: messages.appSettingsNetworkErrorRequest,
        empty_body: messages.appSettingsNetworkErrorEmptyBody,
    }
    return formatMessage(known[error] ?? messages.appSettingsNetworkErrorRequest)
}

async function copy() {
    try {
        await navigator.clipboard.writeText(diagnosticReport(probes.value))
        copyState.value = 'copied'
    } catch {
        copyState.value = 'failed'
    }
}

function run(target?: (typeof networkTargets)[number]['id']) {
    copyState.value = 'idle'
    void start(target)
}
</script>

<template>
    <div class="network-connection flex min-w-0 flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.appSettingsNetworkTitle)"
            title-id="settings-target-network-tests"
        >
            <div class="flex flex-wrap items-end justify-between gap-4 p-4">
                <div class="flex min-w-0 flex-col gap-2">
                    <label id="network-test-mode-label" class="text-sm font-semibold">{{
                        formatMessage(messages.appSettingsNetworkMode)
                    }}</label>
                    <Combobox
                        id="network-test-mode"
                        v-model="mode"
                        aria-labelledby="network-test-mode-label"
                        :options="options"
                        :disabled="busy"
                    />
                </div>
                <div class="flex flex-wrap gap-2">
                    <Button :disabled="!hasResults || busy" @click="copy">
                        <CheckIcon v-if="copyState === 'copied'" aria-hidden="true" />
                        <CopyIcon v-else aria-hidden="true" />
                        {{
                            formatMessage(
                                copyState === 'copied'
                                    ? messages.appSettingsNetworkCopied
                                    : messages.appSettingsNetworkCopy,
                            )
                        }}
                    </Button>
                    <Button v-if="busy" @click="stop"
                        ><StopCircleIcon aria-hidden="true" />{{
                            formatMessage(messages.appSettingsNetworkStop)
                        }}</Button
                    >
                    <Button v-else type="colored" color="brand" @click="run()"
                        ><PlayIcon aria-hidden="true" />{{
                            formatMessage(messages.appSettingsNetworkRunAll)
                        }}</Button
                    >
                </div>
            </div>
            <p
                v-if="copyState === 'failed'"
                role="alert"
                class="m-0 px-4 pb-4 text-sm text-[var(--color-red)]"
            >
                {{ formatMessage(messages.appSettingsNetworkCopyFailed) }}
            </p>
            <p
                class="m-0 border-t border-surface-4 px-4 py-3 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.appSettingsNetworkProxyHelp) }}
            </p>
        </SettingsSection>

        <div class="flex min-w-0 flex-col gap-3">
            <section
                v-for="target in rows"
                :key="target.id"
                :aria-label="target.name"
                class="min-w-0 rounded-[var(--radius-md)] border border-solid border-surface-4 bg-surface-3"
            >
                <header class="flex flex-wrap items-center justify-between gap-3 px-4 pt-4">
                    <div class="min-w-0">
                        <h3 class="m-0 text-base font-semibold">{{ target.name }}</h3>
                        <p
                            class="m-0 mt-1 break-all font-mono text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ target.host }}
                        </p>
                    </div>
                    <div class="flex items-center gap-3">
                        <span
                            :data-status="target.status"
                            role="status"
                            class="text-sm font-medium"
                            :class="{
                                'text-[color:color-mix(in_srgb,var(--color-green)_35%,var(--color-text-primary))]':
                                    target.status === 'success',
                                'text-[var(--color-red)]': ['failed', 'http_error'].includes(
                                    target.status,
                                ),
                                'text-[color:color-mix(in_srgb,var(--color-orange)_35%,var(--color-text-primary))]':
                                    target.status === 'restricted',
                                'text-[var(--color-text-tertiary)]': [
                                    'idle',
                                    'queued',
                                    'cancelled',
                                ].includes(target.status),
                            }"
                            >{{ formatMessage(statusMessages[target.status])
                            }}<template
                                v-if="
                                    target.result?.status &&
                                    ['restricted', 'http_error'].includes(target.status)
                                "
                            >
                                · HTTP {{ target.result.status }}</template
                            ></span
                        >
                        <Button
                            :disabled="busy"
                            :aria-label="
                                formatMessage(messages.appSettingsNetworkRunTarget, {
                                    name: target.name,
                                })
                            "
                            @click="run(target.id)"
                            >{{ formatMessage(messages.appSettingsNetworkRun) }}</Button
                        >
                    </div>
                </header>

                <div class="px-4 pb-4">
                    <p
                        v-if="target.state === 'failed'"
                        role="alert"
                        class="mb-0 text-sm text-[var(--color-red)]"
                    >
                        {{ formatMessage(messages.appSettingsNetworkRequestFailed) }}
                    </p>
                    <template v-if="target.result">
                        <p
                            v-if="target.state !== 'done'"
                            class="mb-0 text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.appSettingsNetworkStale) }}
                        </p>
                        <dl class="m-0 mt-4 grid min-w-0 grid-cols-3 gap-4">
                            <div>
                                <dt class="text-xs text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.appSettingsNetworkResponse) }}
                                </dt>
                                <dd class="m-0 mt-1 font-mono text-base tabular-nums">
                                    {{ target.result.response_ms ?? '—'
                                    }}<span
                                        v-if="target.result.response_ms != null"
                                        class="ml-1 text-xs"
                                        >ms</span
                                    >
                                </dd>
                            </div>
                            <div class="min-w-0">
                                <dt class="text-xs text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.appSettingsNetworkPeer) }}
                                </dt>
                                <dd class="m-0 mt-1 break-all font-mono text-sm">
                                    {{ target.result.peer_ip ?? '—' }}
                                </dd>
                            </div>
                            <div v-if="target.id === 'modrinth_cdn'">
                                <dt class="text-xs text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.appSettingsNetworkSample) }}
                                </dt>
                                <dd class="m-0 mt-1 font-mono text-base tabular-nums">
                                    {{ target.result.sample_mib_per_second?.toFixed(2) ?? '—'
                                    }}<span
                                        v-if="target.result.sample_mib_per_second != null"
                                        class="ml-1 text-xs"
                                        >MiB/s</span
                                    >
                                </dd>
                            </div>
                        </dl>
                        <p
                            v-if="target.result.error"
                            role="alert"
                            class="mb-0 text-sm text-[var(--color-red)]"
                        >
                            {{ errorMessage(target.result.error!) }}
                        </p>
                        <p
                            v-if="target.status === 'restricted'"
                            class="mb-0 text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.appSettingsNetworkRestrictedHelp) }}
                        </p>
                        <details class="network-details mt-4 border-t border-surface-4 pt-3">
                            <summary class="cursor-pointer text-sm font-medium">
                                {{ formatMessage(messages.appSettingsNetworkDetails) }}
                            </summary>
                            <dl class="m-0 mt-3 grid grid-cols-2 gap-x-6 gap-y-3 text-sm">
                                <div class="col-span-2 min-w-0">
                                    <dt class="text-[var(--color-text-tertiary)]">
                                        {{ formatMessage(messages.appSettingsNetworkResolved) }}
                                    </dt>
                                    <dd class="m-0 mt-1 break-all font-mono">
                                        {{ target.result.resolved_ips.join(', ') || '—' }}
                                    </dd>
                                    <dd
                                        v-if="target.result.dns_error"
                                        class="m-0 mt-1 text-[var(--color-red)]"
                                    >
                                        {{ formatMessage(messages.appSettingsNetworkDnsFailed) }}
                                    </dd>
                                </div>
                                <div
                                    v-for="field in [
                                        [
                                            formatMessage(messages.appSettingsNetworkDnsTime),
                                            `${target.result.dns_ms} ms`,
                                        ],
                                        [
                                            formatMessage(messages.appSettingsNetworkDnsMode),
                                            formatMessage(
                                                target.result.dns_mode === 'doh'
                                                    ? messages.appSettingsNetworkDoh
                                                    : messages.appSettingsNetworkSystemDns,
                                            ),
                                        ],
                                        [
                                            formatMessage(messages.appSettingsNetworkProxy),
                                            formatMessage(
                                                target.result.proxy_mode === 'custom'
                                                    ? messages.appSettingsNetworkCustomProxy
                                                    : target.result.proxy_mode === 'system'
                                                      ? messages.appSettingsNetworkSystemProxy
                                                      : messages.appSettingsNetworkDirect,
                                            ),
                                        ],
                                        [
                                            formatMessage(messages.appSettingsNetworkStatus),
                                            target.result.status ?? '—',
                                        ],
                                        [
                                            formatMessage(messages.appSettingsNetworkProtocol),
                                            target.result.http_version ?? '—',
                                        ],
                                        ['X-Served-By', target.result.served_by ?? '—'],
                                        ['X-Cache', target.result.cache ?? '—'],
                                        [
                                            formatMessage(messages.appSettingsNetworkTls),
                                            formatMessage(
                                                target.result.tls_verification
                                                    ? messages.appSettingsNetworkTlsOn
                                                    : messages.appSettingsNetworkTlsOff,
                                            ),
                                        ],
                                        [
                                            formatMessage(messages.appSettingsNetworkTested),
                                            new Date(target.result.tested_at).toLocaleString(),
                                        ],
                                        [
                                            formatMessage(
                                                messages.appSettingsNetworkLauncherVersion,
                                            ),
                                            target.result.launcher_version,
                                        ],
                                        [
                                            formatMessage(messages.appSettingsNetworkPlatform),
                                            target.result.platform,
                                        ],
                                    ]"
                                    :key="String(field[0])"
                                    class="min-w-0"
                                >
                                    <dt class="text-[var(--color-text-tertiary)]">
                                        {{ field[0] }}
                                    </dt>
                                    <dd class="m-0 mt-1 break-all">{{ field[1] }}</dd>
                                </div>
                                <div class="col-span-2 min-w-0">
                                    <dt class="text-[var(--color-text-tertiary)]">
                                        {{ formatMessage(messages.appSettingsNetworkUrl) }}
                                    </dt>
                                    <dd class="m-0 mt-1 break-all font-mono text-xs">
                                        {{ target.result.url }}
                                    </dd>
                                </div>
                            </dl>
                        </details>
                    </template>
                </div>
            </section>
        </div>
    </div>
</template>

<style scoped>
.network-connection {
    animation: network-enter 160ms ease-out;
}
.network-details[open] > dl {
    animation: network-enter 140ms ease-out;
}
@keyframes network-enter {
    from {
        opacity: 0;
        transform: translateY(4px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}
@media (prefers-reduced-motion: reduce) {
    .network-connection,
    .network-details[open] > dl {
        animation: none;
    }
}
</style>
