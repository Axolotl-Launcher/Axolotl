<script setup lang="ts">
import { AlertCircleIcon, ClipboardCopyIcon, SearchIcon, SpinnerIcon } from '@modrinth/assets'
import {
    Button,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import {
    findTrialChambers,
    trialChamberAvailable,
    type TrialChamberScan,
} from '@/lab/trial-chamber/client'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()

const messages = defineMessages({
    title: { id: 'app.lab.trial-chamber.title', defaultMessage: 'Trial chambers' },
    description: {
        id: 'app.lab.trial-chamber.description',
        defaultMessage:
            'Find trial chambers for a world seed and count what they contain. The search assembles each chamber from its structure data, so it reports the spawners and chambers that actually generate.',
    },
    seedLabel: { id: 'app.lab.trial-chamber.seed', defaultMessage: 'World seed' },
    seedPlaceholder: {
        id: 'app.lab.trial-chamber.seed-placeholder',
        defaultMessage: 'World seed, numeric or text',
    },
    radiusLabel: { id: 'app.lab.trial-chamber.radius', defaultMessage: 'Search radius' },
    radiusHint: {
        id: 'app.lab.trial-chamber.radius-hint',
        defaultMessage: 'In blocks around the origin. A wider radius scans for longer.',
    },
    threadsLabel: { id: 'app.lab.trial-chamber.threads', defaultMessage: 'Threads' },
    threadsHint: {
        id: 'app.lab.trial-chamber.threads-hint',
        defaultMessage: 'More threads scan faster but need more memory. Eight suits 4 GB.',
    },
    scan: { id: 'app.lab.trial-chamber.scan', defaultMessage: 'Scan' },
    scanning: { id: 'app.lab.trial-chamber.scanning', defaultMessage: 'Scanning…' },
    noJavaTitle: {
        id: 'app.lab.trial-chamber.no-java-title',
        defaultMessage: 'Java not found',
    },
    noJavaBody: {
        id: 'app.lab.trial-chamber.no-java-body',
        defaultMessage:
            'This tool needs a Java 25 runtime. Install JDK 25, or point JAVA_HOME at one, then reload this page.',
    },
    emptyTitle: {
        id: 'app.lab.trial-chamber.empty-title',
        defaultMessage: 'No chambers found yet',
    },
    emptyBody: {
        id: 'app.lab.trial-chamber.empty-body',
        defaultMessage: 'Enter a seed and run a scan to see where the trial chambers land.',
    },
    resultsTitle: {
        id: 'app.lab.trial-chamber.results',
        defaultMessage: 'Chambers, most spawners first',
    },
    columnRank: { id: 'app.lab.trial-chamber.column-rank', defaultMessage: '#' },
    columnCenterX: { id: 'app.lab.trial-chamber.column-center-x', defaultMessage: 'Center X' },
    columnCenterZ: { id: 'app.lab.trial-chamber.column-center-z', defaultMessage: 'Center Z' },
    columnChambers: {
        id: 'app.lab.trial-chamber.column-chambers',
        defaultMessage: 'Chambers',
    },
    columnSpawners: {
        id: 'app.lab.trial-chamber.column-spawners',
        defaultMessage: 'Spawners',
    },
    copyTp: { id: 'app.lab.trial-chamber.copy-tp', defaultMessage: 'Copy /tp' },
    copied: { id: 'app.lab.trial-chamber.copied', defaultMessage: 'Copied' },
    best: { id: 'app.lab.trial-chamber.best', defaultMessage: 'Most spawners: {count}' },
    elapsed: { id: 'app.lab.trial-chamber.elapsed', defaultMessage: 'Scanned in {ms} ms' },
    failed: { id: 'app.lab.trial-chamber.failed', defaultMessage: 'The scan failed: {reason}' },
    doneTitle: {
        id: 'app.lab.trial-chamber.done-title',
        defaultMessage: 'Trial chamber scan finished',
    },
    doneBody: {
        id: 'app.lab.trial-chamber.done-body',
        defaultMessage: 'Found {count} chambers for seed {seed}.',
    },
    failedTitle: {
        id: 'app.lab.trial-chamber.failed-title',
        defaultMessage: 'Trial chamber scan failed',
    },
})

const seed = ref('')
const radius = ref(30000)
const threads = ref(8)
const scanning = ref(false)
const error = ref<string | null>(null)
const result = ref<TrialChamberScan | null>(null)
const available = ref<boolean | null>(null)
const copiedKey = ref<string | null>(null)

onMounted(async () => {
    try {
        available.value = await trialChamberAvailable()
    } catch {
        available.value = false
    }
})

const canScan = computed(() => !!seed.value.trim() && !scanning.value && available.value !== false)

async function run() {
    if (!canScan.value) return
    scanning.value = true
    error.value = null
    try {
        const scanned = await findTrialChambers(seed.value.trim(), radius.value, threads.value)
        result.value = scanned
        addNotification({
            title: formatMessage(messages.doneTitle),
            text: formatMessage(messages.doneBody, {
                count: scanned.hits.length,
                seed: scanned.seed,
            }),
            type: 'success',
        }).catch(() => {})
    } catch (cause) {
        error.value = String(cause)
        result.value = null
        addNotification({
            title: formatMessage(messages.failedTitle),
            text: String(cause),
            type: 'error',
        }).catch(() => {})
    } finally {
        scanning.value = false
    }
}

const rows = computed(() => {
    const hits = result.value?.hits ?? []
    return [...hits].sort((a, b) => b.spawnerCount - a.spawnerCount)
})

async function copyTeleport(hit: { centerX: number; centerZ: number }) {
    const command = `/tp ${hit.centerX} ~ ${hit.centerZ}`
    try {
        await navigator.clipboard.writeText(command)
        copiedKey.value = `${hit.centerX}:${hit.centerZ}`
        window.setTimeout(() => {
            if (copiedKey.value === `${hit.centerX}:${hit.centerZ}`) copiedKey.value = null
        }, 1500)
    } catch {
        // Clipboard access can be refused; the coordinates stay selectable.
    }
}
</script>

<template>
    <div class="flex min-h-0 flex-1 flex-col gap-4 p-6">
        <header class="flex flex-col gap-1">
            <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h1>
            <p class="m-0 max-w-3xl text-sm text-[var(--color-text-secondary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </header>

        <section
            v-if="available === false"
            class="flex items-start gap-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-4"
        >
            <AlertCircleIcon class="mt-0.5 size-5 shrink-0 text-[var(--color-orange)]" />
            <div class="flex flex-col gap-1">
                <strong class="text-sm">{{ formatMessage(messages.noJavaTitle) }}</strong>
                <span class="text-sm text-[var(--color-text-secondary)]">
                    {{ formatMessage(messages.noJavaBody) }}
                </span>
            </div>
        </section>

        <!-- Results left, controls right, matching the swamp-hut finder. -->
        <div class="flex min-h-0 flex-1 gap-4">
            <section class="flex min-w-0 flex-1 flex-col gap-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <h2 class="m-0 text-lg font-semibold">
                        {{ formatMessage(messages.resultsTitle) }}
                    </h2>
                    <span v-if="result" class="text-xs text-[var(--color-text-tertiary)]">
                        <template v-if="result.bestSpawners !== null">
                            {{ formatMessage(messages.best, { count: result.bestSpawners }) }} ·
                        </template>
                        {{ formatMessage(messages.elapsed, { ms: result.elapsedMs }) }}
                    </span>
                </div>

                <div
                    v-if="error"
                    class="rounded-lg border border-solid border-surface-4 bg-surface-2 p-4 text-sm"
                >
                    {{ formatMessage(messages.failed, { reason: error }) }}
                </div>

                <div
                    v-else-if="!result"
                    class="flex flex-1 flex-col items-center justify-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-2 p-8 text-center"
                >
                    <strong class="text-sm">{{ formatMessage(messages.emptyTitle) }}</strong>
                    <span class="text-sm text-[var(--color-text-secondary)]">
                        {{ formatMessage(messages.emptyBody) }}
                    </span>
                </div>

                <div v-else class="min-h-0 flex-1 overflow-auto">
                    <table class="w-full border-collapse text-sm">
                        <thead class="sticky top-0 bg-surface-1">
                            <tr class="text-left text-xs text-[var(--color-text-tertiary)]">
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnRank) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnCenterX) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnCenterZ) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnChambers) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnSpawners) }}
                                </th>
                                <th class="py-1 font-medium" />
                            </tr>
                        </thead>
                        <tbody>
                            <tr
                                v-for="hit in rows"
                                :key="`${hit.centerX}:${hit.centerZ}`"
                                class="border-t border-solid border-surface-4"
                            >
                                <td class="py-1 pr-3 text-[var(--color-text-tertiary)]">
                                    {{ hit.rank }}
                                </td>
                                <td class="py-1 pr-3">{{ hit.centerX }}</td>
                                <td class="py-1 pr-3">{{ hit.centerZ }}</td>
                                <td class="py-1 pr-3">{{ hit.chamberCount }}</td>
                                <td class="py-1 pr-3 font-semibold">{{ hit.spawnerCount }}</td>
                                <td class="py-1">
                                    <Button size="2xs" type="quiet" @click="copyTeleport(hit)">
                                        <ClipboardCopyIcon class="size-3.5" />
                                        {{
                                            copiedKey === `${hit.centerX}:${hit.centerZ}`
                                                ? formatMessage(messages.copied)
                                                : formatMessage(messages.copyTp)
                                        }}
                                    </Button>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </section>

            <aside
                class="flex w-80 shrink-0 flex-col gap-3 self-start rounded-lg border border-solid border-surface-4 bg-surface-2 p-4"
            >
                <label class="flex flex-col gap-1">
                    <span class="text-sm font-medium">{{ formatMessage(messages.seedLabel) }}</span>
                    <StyledInput
                        v-model="seed"
                        :placeholder="formatMessage(messages.seedPlaceholder)"
                        :disabled="scanning || available === false"
                        @keydown.enter="run"
                    />
                </label>

                <label class="flex flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.radiusLabel) }}
                    </span>
                    <input
                        v-model.number="radius"
                        type="number"
                        min="128"
                        max="1000000"
                        step="1000"
                        class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                        :disabled="scanning || available === false"
                    />
                </label>
                <span class="text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.radiusHint) }}
                </span>

                <label class="flex flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.threadsLabel) }}
                    </span>
                    <input
                        v-model.number="threads"
                        type="number"
                        min="1"
                        max="64"
                        class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                        :disabled="scanning || available === false"
                    />
                </label>
                <span class="text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.threadsHint) }}
                </span>

                <Button type="colored" :disabled="!canScan" @click="run">
                    <SpinnerIcon v-if="scanning" class="size-4 animate-spin" />
                    <SearchIcon v-else class="size-4" />
                    {{ scanning ? formatMessage(messages.scanning) : formatMessage(messages.scan) }}
                </Button>
            </aside>
        </div>
    </div>
</template>
