<script setup lang="ts">
import { CheckIcon, CopyIcon, PlayIcon, StopCircleIcon } from '@modrinth/assets'
import { Button, Combobox, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
    diagnosticReport,
    networkTargets,
    resultStatus,
    useNetworkDiagnostics,
} from './network-diagnostics'
import { networkMessages as messages } from './network-messages'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { mode, probes, busy, hasResults, start, stop } = useNetworkDiagnostics()
const copyState = ref<'idle' | 'copied' | 'failed'>('idle')
const options = computed(() => [
    { value: 'launcher', label: formatMessage(messages.launcher) },
    { value: 'direct', label: formatMessage(messages.direct) },
])

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
    const known = {
        timeout: messages.timeout,
        connection: messages.connection,
        body: messages.body,
        request: messages.request,
        empty_body: messages.empty_body,
    }
    return formatMessage(known[error as keyof typeof known] ?? messages.request)
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
            :title="formatMessage(messages.title)"
            title-id="settings-target-network-tests"
        >
            <div class="flex flex-wrap items-end justify-between gap-4 p-4">
                <div class="flex min-w-0 flex-col gap-2">
                    <label id="network-test-mode-label" class="text-sm font-semibold">{{
                        formatMessage(messages.mode)
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
                            formatMessage(copyState === 'copied' ? messages.copied : messages.copy)
                        }}
                    </Button>
                    <Button v-if="busy" @click="stop"
                        ><StopCircleIcon aria-hidden="true" />{{
                            formatMessage(messages.stop)
                        }}</Button
                    >
                    <Button v-else type="colored" color="brand" @click="run()"
                        ><PlayIcon aria-hidden="true" />{{ formatMessage(messages.runAll) }}</Button
                    >
                </div>
            </div>
            <p
                v-if="copyState === 'failed'"
                role="alert"
                class="m-0 px-4 pb-4 text-sm text-[var(--color-red)]"
            >
                {{ formatMessage(messages.copyFailed) }}
            </p>
            <p
                class="m-0 border-t border-surface-4 px-4 py-3 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.proxyHelp) }}
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
                            >{{ formatMessage(messages[target.status as keyof typeof messages])
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
                            :aria-label="formatMessage(messages.runTarget, { name: target.name })"
                            @click="run(target.id)"
                            >{{ formatMessage(messages.run) }}</Button
                        >
                    </div>
                </header>

                <div class="px-4 pb-4">
                    <p
                        v-if="target.state === 'failed'"
                        role="alert"
                        class="mb-0 text-sm text-[var(--color-red)]"
                    >
                        {{ formatMessage(messages.requestFailed) }}
                    </p>
                    <template v-if="target.result">
                        <p
                            v-if="target.state !== 'done'"
                            class="mb-0 text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.stale) }}
                        </p>
                        <dl
                            class="m-0 mt-4 grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3"
                        >
                            <div>
                                <dt class="text-xs text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.response) }}
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
                                    {{ formatMessage(messages.peer) }}
                                </dt>
                                <dd class="m-0 mt-1 break-all font-mono text-sm">
                                    {{ target.result.peer_ip ?? '—' }}
                                </dd>
                            </div>
                            <div v-if="target.id === 'modrinth_cdn'">
                                <dt class="text-xs text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.sample) }}
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
                            {{ formatMessage(messages.restrictedHelp) }}
                        </p>
                        <details class="network-details mt-4 border-t border-surface-4 pt-3">
                            <summary class="cursor-pointer text-sm font-medium">
                                {{ formatMessage(messages.details) }}
                            </summary>
                            <dl
                                class="m-0 mt-3 grid grid-cols-1 gap-x-6 gap-y-3 text-sm sm:grid-cols-2"
                            >
                                <div class="min-w-0 sm:col-span-2">
                                    <dt class="text-[var(--color-text-tertiary)]">
                                        {{ formatMessage(messages.resolved) }}
                                    </dt>
                                    <dd class="m-0 mt-1 break-all font-mono">
                                        {{ target.result.resolved_ips.join(', ') || '—' }}
                                    </dd>
                                    <dd
                                        v-if="target.result.dns_error"
                                        class="m-0 mt-1 text-[var(--color-red)]"
                                    >
                                        {{ formatMessage(messages.dnsFailed) }}
                                    </dd>
                                </div>
                                <div
                                    v-for="field in [
                                        [
                                            formatMessage(messages.dnsTime),
                                            `${target.result.dns_ms} ms`,
                                        ],
                                        [
                                            formatMessage(messages.dnsMode),
                                            formatMessage(
                                                target.result.dns_mode === 'doh'
                                                    ? messages.doh
                                                    : messages.systemDns,
                                            ),
                                        ],
                                        [
                                            formatMessage(messages.proxy),
                                            formatMessage(
                                                target.result.proxy_mode === 'custom'
                                                    ? messages.customProxy
                                                    : target.result.proxy_mode === 'system'
                                                      ? messages.systemProxy
                                                      : messages.direct,
                                            ),
                                        ],
                                        [
                                            formatMessage(messages.status),
                                            target.result.status ?? '—',
                                        ],
                                        [
                                            formatMessage(messages.protocol),
                                            target.result.http_version ?? '—',
                                        ],
                                        ['X-Served-By', target.result.served_by ?? '—'],
                                        ['X-Cache', target.result.cache ?? '—'],
                                        [
                                            formatMessage(messages.tls),
                                            formatMessage(
                                                target.result.tls_verification
                                                    ? messages.tlsOn
                                                    : messages.tlsOff,
                                            ),
                                        ],
                                        [
                                            formatMessage(messages.tested),
                                            new Date(target.result.tested_at).toLocaleString(),
                                        ],
                                        [
                                            formatMessage(messages.launcherVersion),
                                            target.result.launcher_version,
                                        ],
                                        [formatMessage(messages.platform), target.result.platform],
                                    ]"
                                    :key="String(field[0])"
                                    class="min-w-0"
                                >
                                    <dt class="text-[var(--color-text-tertiary)]">
                                        {{ field[0] }}
                                    </dt>
                                    <dd class="m-0 mt-1 break-all">{{ field[1] }}</dd>
                                </div>
                                <div class="min-w-0 sm:col-span-2">
                                    <dt class="text-[var(--color-text-tertiary)]">
                                        {{ formatMessage(messages.url) }}
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
