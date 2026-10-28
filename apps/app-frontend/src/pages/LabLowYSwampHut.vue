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

import { lowYSwampHutAvailable, scanLowYSwampHut } from '@/lab/low-y-swamp-hut/client'
import type { SwampHutScan } from '@/lab/low-y-swamp-hut/types'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()

const messages = defineMessages({
    title: { id: 'app.lab.low-y-swamp-hut.title', defaultMessage: 'Low-Y swamp hut' },
    description: {
        id: 'app.lab.low-y-swamp-hut.description',
        defaultMessage:
            'Find the swamp hut that generates lowest for a given world seed. The scan replays vanilla terrain generation, so it runs for the regions you ask for rather than the whole world.',
    },
    seedLabel: { id: 'app.lab.low-y-swamp-hut.seed', defaultMessage: 'World seed' },
    seedPlaceholder: {
        id: 'app.lab.low-y-swamp-hut.seed-placeholder',
        defaultMessage: 'World seed, numeric or text',
    },
    regionsLabel: { id: 'app.lab.low-y-swamp-hut.regions', defaultMessage: 'Regions' },
    regionsHint: {
        id: 'app.lab.low-y-swamp-hut.regions-hint',
        defaultMessage: 'One region is 512 blocks across. More regions means a longer scan.',
    },
    threadsLabel: { id: 'app.lab.low-y-swamp-hut.threads', defaultMessage: 'Threads' },
    threadsHint: {
        id: 'app.lab.low-y-swamp-hut.threads-hint',
        defaultMessage: 'Zero uses one worker per CPU core. Raising it can hit rate limits.',
    },
    versionLabel: { id: 'app.lab.low-y-swamp-hut.version', defaultMessage: 'Game version' },
    presetLabel: { id: 'app.lab.low-y-swamp-hut.preset', defaultMessage: 'World preset' },
    maxYLabel: { id: 'app.lab.low-y-swamp-hut.max-y', defaultMessage: 'Height threshold' },
    maxYHint: {
        id: 'app.lab.low-y-swamp-hut.max-y-hint',
        defaultMessage: 'Phase one stops at this height. Lower values take longer to find candidates.',
    },
    phase2Label: {
        id: 'app.lab.low-y-swamp-hut.phase2-max-y',
        defaultMessage: 'Final height threshold',
    },
    scan: { id: 'app.lab.low-y-swamp-hut.scan', defaultMessage: 'Scan' },
    scanning: { id: 'app.lab.low-y-swamp-hut.scanning', defaultMessage: 'Scanning…' },
    unavailableTitle: {
        id: 'app.lab.low-y-swamp-hut.unavailable-title',
        defaultMessage: 'Search program not found',
    },
    unavailableBody: {
        id: 'app.lab.low-y-swamp-hut.unavailable-body',
        defaultMessage:
            'This tool calls the LowYSwampHut search program, which is not installed here. Place `lysh` on your PATH and reload this page.',
    },
    emptyTitle: {
        id: 'app.lab.low-y-swamp-hut.empty-title',
        defaultMessage: 'No huts found yet',
    },
    emptyBody: {
        id: 'app.lab.low-y-swamp-hut.empty-body',
        defaultMessage: 'Enter a seed and run a scan to see where the swamp huts land.',
    },
    resultsTitle: {
        id: 'app.lab.low-y-swamp-hut.results',
        defaultMessage: 'Huts, lowest first',
    },
    columnX: { id: 'app.lab.low-y-swamp-hut.column-x', defaultMessage: 'X' },
    columnZ: { id: 'app.lab.low-y-swamp-hut.column-z', defaultMessage: 'Z' },
    columnY: { id: 'app.lab.low-y-swamp-hut.column-y', defaultMessage: 'Y' },
    columnDistance: {
        id: 'app.lab.low-y-swamp-hut.column-distance',
        defaultMessage: 'Distance',
    },
    lowest: { id: 'app.lab.low-y-swamp-hut.lowest', defaultMessage: 'Lowest Y: {y}' },
    elapsed: { id: 'app.lab.low-y-swamp-hut.elapsed', defaultMessage: 'Scanned in {ms} ms' },
    failed: {
        id: 'app.lab.low-y-swamp-hut.failed',
        defaultMessage: 'The scan failed: {reason}',
    },
    copyTp: { id: 'app.lab.low-y-swamp-hut.copy-tp', defaultMessage: 'Copy /tp' },
    copied: { id: 'app.lab.low-y-swamp-hut.copied', defaultMessage: 'Copied' },
    scanDoneTitle: {
        id: 'app.lab.low-y-swamp-hut.scan-done-title',
        defaultMessage: 'Swamp hut scan finished',
    },
    scanDoneBody: {
        id: 'app.lab.low-y-swamp-hut.scan-done-body',
        defaultMessage: 'Found {count} huts for seed {seed}.',
    },
    scanFailedTitle: {
        id: 'app.lab.low-y-swamp-hut.scan-failed-title',
        defaultMessage: 'Swamp hut scan failed',
    },
})

const VERSION_OPTIONS = ['26.2', '1.21', '1.20.1', '1.19.2', '1.18.2'] as const
const PRESET_OPTIONS = ['NORMAL', 'LARGE_BIOMES', 'SINGLE_BIOME'] as const

const seed = ref('')
const regions = ref(16)
const threads = ref(0)
const version = ref<(typeof VERSION_OPTIONS)[number]>('26.2')
const preset = ref<(typeof PRESET_OPTIONS)[number]>('NORMAL')
const maxY = ref(-40)
const phase2MaxY = ref(-40)
const scanning = ref(false)
const error = ref<string | null>(null)
const result = ref<SwampHutScan | null>(null)
const available = ref<boolean | null>(null)
const copiedKey = ref<string | null>(null)

onMounted(async () => {
    try {
        available.value = await lowYSwampHutAvailable()
    } catch {
        available.value = false
    }
})

const canScan = computed(
    () => !!seed.value.trim() && !scanning.value && available.value !== false,
)

async function run() {
    if (!canScan.value) return
    scanning.value = true
    error.value = null
    try {
        const scanned = await scanLowYSwampHut({
            seed: seed.value.trim(),
            regions: regions.value,
            threads: threads.value,
            version: version.value,
            preset: preset.value,
            maxY: maxY.value,
            phase2MaxY: phase2MaxY.value,
        })
        result.value = scanned
        // A scan can run for minutes on a weak machine, so the result is announced
        // rather than only drawn: the user is expected to have moved on.
        addNotification({
            title: formatMessage(messages.scanDoneTitle),
            text: formatMessage(messages.scanDoneBody, {
                count: scanned.hits.length,
                seed: scanned.seed,
            }),
            type: 'success',
        }).catch(() => {})
    } catch (cause) {
        error.value = String(cause)
        result.value = null
        addNotification({
            title: formatMessage(messages.scanFailedTitle),
            text: String(cause),
            type: 'error',
        }).catch(() => {})
    } finally {
        scanning.value = false
    }
}

const rows = computed(() => {
    const hits = result.value?.hits ?? []
    return [...hits].sort((a, b) => a.y - b.y)
})

async function copyTeleport(hit: { x: number; z: number; y: number }) {
    const command = `/tp ${hit.x} ${Math.round(hit.y)} ${hit.z}`
    try {
        await navigator.clipboard.writeText(command)
        copiedKey.value = `${hit.x}:${hit.z}`
        window.setTimeout(() => {
            if (copiedKey.value === `${hit.x}:${hit.z}`) copiedKey.value = null
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
                <strong class="text-sm">{{ formatMessage(messages.unavailableTitle) }}</strong>
                <span class="text-sm text-[var(--color-text-secondary)]">
                    {{ formatMessage(messages.unavailableBody) }}
                </span>
            </div>
        </section>

        <!-- Results on the left, controls on the right: the reading order follows
             the number that matters (the lowest Y) into the panel that changes it. -->
        <div class="flex min-h-0 flex-1 gap-4">
            <section class="flex min-w-0 flex-1 flex-col gap-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <h2 class="m-0 text-lg font-semibold">
                        {{ formatMessage(messages.resultsTitle) }}
                    </h2>
                    <span
                        v-if="result"
                        class="text-xs text-[var(--color-text-tertiary)]"
                    >
                        <template v-if="result.lowestY !== null">
                            {{ formatMessage(messages.lowest, { y: result.lowestY }) }} ·
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
                                    {{ formatMessage(messages.columnX) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnZ) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnY) }}
                                </th>
                                <th class="py-1 pr-3 font-medium">
                                    {{ formatMessage(messages.columnDistance) }}
                                </th>
                                <th class="py-1 font-medium" />
                            </tr>
                        </thead>
                        <tbody>
                            <tr
                                v-for="hit in rows"
                                :key="`${hit.x}:${hit.z}`"
                                class="border-t border-solid border-surface-4"
                            >
                                <td class="py-1 pr-3">{{ hit.x }}</td>
                                <td class="py-1 pr-3">{{ hit.z }}</td>
                                <td class="py-1 pr-3 font-semibold">{{ hit.y }}</td>
                                <td class="py-1 pr-3">{{ Math.round(hit.distance) }}</td>
                                <td class="py-1">
                                    <Button size="2xs" type="quiet" @click="copyTeleport(hit)">
                                        <ClipboardCopyIcon class="size-3.5" />
                                        {{
                                            copiedKey === `${hit.x}:${hit.z}`
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

                <div class="flex gap-2">
                    <label class="flex flex-1 flex-col gap-1">
                        <span class="text-sm font-medium">
                            {{ formatMessage(messages.regionsLabel) }}
                        </span>
                        <input
                            v-model.number="regions"
                            type="number"
                            min="1"
                            max="4096"
                            class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                            :disabled="scanning || available === false"
                        />
                    </label>
                    <label class="flex flex-1 flex-col gap-1">
                        <span class="text-sm font-medium">
                            {{ formatMessage(messages.threadsLabel) }}
                        </span>
                        <input
                            v-model.number="threads"
                            type="number"
                            min="0"
                            max="64"
                            class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                            :disabled="scanning || available === false"
                        />
                    </label>
                </div>
                <span class="text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.regionsHint) }}
                    {{ formatMessage(messages.threadsHint) }}
                </span>

                <label class="flex flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.versionLabel) }}
                    </span>
                    <select
                        v-model="version"
                        class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                        :disabled="scanning || available === false"
                    >
                        <option v-for="option in VERSION_OPTIONS" :key="option" :value="option">
                            {{ option }}
                        </option>
                    </select>
                </label>

                <label class="flex flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.presetLabel) }}
                    </span>
                    <select
                        v-model="preset"
                        class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                        :disabled="scanning || available === false"
                    >
                        <option v-for="option in PRESET_OPTIONS" :key="option" :value="option">
                            {{ option }}
                        </option>
                    </select>
                </label>

                <div class="flex gap-2">
                    <label class="flex flex-1 flex-col gap-1">
                        <span class="text-sm font-medium">
                            {{ formatMessage(messages.maxYLabel) }}
                        </span>
                        <input
                            v-model.number="maxY"
                            type="number"
                            min="-64"
                            max="320"
                            class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                            :disabled="scanning || available === false"
                        />
                    </label>
                    <label class="flex flex-1 flex-col gap-1">
                        <span class="text-sm font-medium">
                            {{ formatMessage(messages.phase2Label) }}
                        </span>
                        <input
                            v-model.number="phase2MaxY"
                            type="number"
                            min="-64"
                            max="320"
                            class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                            :disabled="scanning || available === false"
                        />
                    </label>
                </div>
                <span class="text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.maxYHint) }}
                </span>

                <Button type="colored" :disabled="!canScan" @click="run">
                    <SpinnerIcon v-if="scanning" class="size-4 animate-spin" />
                    <SearchIcon v-else class="size-4" />
                    {{
                        scanning ? formatMessage(messages.scanning) : formatMessage(messages.scan)
                    }}
                </Button>
            </aside>
        </div>
    </div>
</template>
