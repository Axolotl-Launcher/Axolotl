<script setup lang="ts">
import { AlertCircleIcon, SearchIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, defineMessages, StyledInput, useVIntl } from '@modrinth/ui'
import { onMounted, ref } from 'vue'

import { lowYSwampHutAvailable, scanLowYSwampHut } from '@/lab/low-y-swamp-hut/client'
import type { SwampHutScan } from '@/lab/low-y-swamp-hut/types'

const { formatMessage } = useVIntl()

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
    scan: { id: 'app.lab.low-y-swamp-hut.scan', defaultMessage: 'Scan' },
    scanning: { id: 'app.lab.low-y-swamp-hut.scanning', defaultMessage: 'Scanning…' },
    unavailableTitle: {
        id: 'app.lab.low-y-swamp-hut.unavailable-title',
        defaultMessage: 'Search program not found',
    },
    unavailableBody: {
        id: 'app.lab.low-y-swamp-hut.unavailable-body',
        defaultMessage:
            'This tool calls the LowYSwampHut search program, which is not installed here. Place `lysh` on your PATH (or set the path in settings) and reload this page.',
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
})

const seed = ref('')
const regions = ref(16)
const threads = ref(0)
const scanning = ref(false)
const error = ref<string | null>(null)
const result = ref<SwampHutScan | null>(null)
const available = ref<boolean | null>(null)

onMounted(async () => {
    try {
        available.value = await lowYSwampHutAvailable()
    } catch {
        available.value = false
    }
})

async function run() {
    if (!seed.value.trim() || scanning.value) return
    scanning.value = true
    error.value = null
    try {
        result.value = await scanLowYSwampHut(seed.value.trim(), regions.value, threads.value)
    } catch (cause) {
        error.value = String(cause)
        result.value = null
    } finally {
        scanning.value = false
    }
}

function rows(): { x: number; z: number; y: number; distance: number }[] {
    const hits = result.value?.hits ?? []
    return [...hits].sort((a, b) => a.y - b.y)
}
</script>

<template>
    <div class="flex flex-col gap-4 p-6">
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

        <section
            class="flex flex-wrap items-end gap-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-4"
        >
            <label class="flex flex-1 flex-col gap-1" style="min-width: 16rem">
                <span class="text-sm font-medium">{{ formatMessage(messages.seedLabel) }}</span>
                <StyledInput
                    v-model="seed"
                    :placeholder="formatMessage(messages.seedPlaceholder)"
                    :disabled="scanning || available === false"
                    @keydown.enter="run"
                />
            </label>

            <label class="flex flex-col gap-1" style="width: 8rem">
                <span class="text-sm font-medium">{{ formatMessage(messages.regionsLabel) }}</span>
                <input
                    v-model.number="regions"
                    type="number"
                    min="1"
                    max="4096"
                    class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                    :disabled="scanning || available === false"
                />
            </label>

            <label class="flex flex-col gap-1" style="width: 8rem">
                <span class="text-sm font-medium">{{ formatMessage(messages.threadsLabel) }}</span>
                <input
                    v-model.number="threads"
                    type="number"
                    min="0"
                    max="64"
                    class="rounded-md border border-solid border-surface-5 bg-surface-1 px-3 py-2 text-sm"
                    :disabled="scanning || available === false"
                />
            </label>

            <Button
                type="colored"
                :disabled="!seed.trim() || scanning || available === false"
                @click="run"
            >
                <SpinnerIcon v-if="scanning" class="size-4 animate-spin" />
                <SearchIcon v-else class="size-4" />
                {{ scanning ? formatMessage(messages.scanning) : formatMessage(messages.scan) }}
            </Button>
        </section>

        <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.regionsHint) }}
        </p>

        <section
            v-if="error"
            class="rounded-lg border border-solid border-surface-4 bg-surface-2 p-4 text-sm"
        >
            {{ formatMessage(messages.failed, { reason: error }) }}
        </section>

        <section
            v-else-if="!result"
            class="rounded-lg border border-solid border-surface-4 bg-surface-2 p-8 text-center"
        >
            <strong class="block text-sm">{{ formatMessage(messages.emptyTitle) }}</strong>
            <span class="text-sm text-[var(--color-text-secondary)]">
                {{ formatMessage(messages.emptyBody) }}
            </span>
        </section>

        <section v-else class="flex flex-col gap-2">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <h2 class="m-0 text-lg font-semibold">
                    {{ formatMessage(messages.resultsTitle) }}
                </h2>
                <span class="text-xs text-[var(--color-text-tertiary)]">
                    <template v-if="result.lowestY !== null">
                        {{ formatMessage(messages.lowest, { y: result.lowestY }) }} ·
                    </template>
                    {{ formatMessage(messages.elapsed, { ms: result.elapsedMs }) }}
                </span>
            </div>

            <table class="w-full border-collapse text-sm">
                <thead>
                    <tr class="text-left text-xs text-[var(--color-text-tertiary)]">
                        <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnX) }}</th>
                        <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnZ) }}</th>
                        <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnY) }}</th>
                        <th class="py-1 font-medium">
                            {{ formatMessage(messages.columnDistance) }}
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <tr
                        v-for="hit in rows()"
                        :key="`${hit.x}:${hit.z}`"
                        class="border-t border-solid border-surface-4"
                    >
                        <td class="py-1 pr-3">{{ hit.x }}</td>
                        <td class="py-1 pr-3">{{ hit.z }}</td>
                        <td class="py-1 pr-3 font-semibold">{{ hit.y }}</td>
                        <td class="py-1">{{ hit.distance }}</td>
                    </tr>
                </tbody>
            </table>
        </section>
    </div>
</template>
