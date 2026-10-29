<script setup lang="ts">
import { ClipboardCopyIcon } from '@modrinth/assets'
import {
    Button,
    Combobox,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, ref, watch } from 'vue'

import StructureFinderLayout from '@/components/lab/seed-map/StructureFinderLayout.vue'
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
        defaultMessage:
            'Phase one stops at this height. Lower values take longer to find candidates.',
    },
    phase2Label: {
        id: 'app.lab.low-y-swamp-hut.phase2-max-y',
        defaultMessage: 'Final height threshold',
    },
    unavailableBody: {
        id: 'app.lab.low-y-swamp-hut.unavailable-body',
        defaultMessage:
            'This tool calls the LowYSwampHut search program, which is not installed here. Place `lysh` on your PATH and reload this page.',
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
    presetNormal: {
        id: 'app.lab.low-y-swamp-hut.preset-normal',
        defaultMessage: 'Default',
    },
    presetLargeBiomes: {
        id: 'app.lab.low-y-swamp-hut.preset-large-biomes',
        defaultMessage: 'Large biomes',
    },
    presetSingleBiome: {
        id: 'app.lab.low-y-swamp-hut.preset-single-biome',
        defaultMessage: 'Single biome',
    },
})

const VERSION_OPTIONS = ['26.2', '1.21', '1.20.1', '1.19.2', '1.18.2'] as const
const PRESET_OPTIONS = ['NORMAL', 'LARGE_BIOMES', 'SINGLE_BIOME'] as const

const versionOptions = computed(() =>
    VERSION_OPTIONS.map((value) => ({ value, label: value })),
)

function presetLabel(value: (typeof PRESET_OPTIONS)[number]) {
    if (value === 'LARGE_BIOMES') return formatMessage(messages.presetLargeBiomes)
    if (value === 'SINGLE_BIOME') return formatMessage(messages.presetSingleBiome)
    return formatMessage(messages.presetNormal)
}

const presetOptions = computed(() =>
    PRESET_OPTIONS.map((value) => ({ value, label: presetLabel(value) })),
)

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

const canScan = computed(() => !!seed.value.trim() && !scanning.value && available.value !== false)

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
        addNotification({
            title: formatMessage(messages.scanDoneTitle),
            text: formatMessage(messages.scanDoneBody, {
                count: scanned.hits.length,
                seed: scanned.seed,
            }),
            type: 'success',
        })
    } catch (cause) {
        error.value = String(cause)
        result.value = null
        addNotification({
            title: formatMessage(messages.scanFailedTitle),
            text: String(cause),
            type: 'error',
        })
    } finally {
        scanning.value = false
    }
}

// A finished scan describes the inputs it ran with. Changing any of them makes
// the table describe a search the user is no longer looking at, so drop it
// rather than let old coordinates pass for the new ones. The version, preset and
// Y gates change the terrain the program reproduces, so they count as well.
watch([seed, regions, threads, version, preset, maxY, phase2MaxY], () => {
    result.value = null
})

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
    <StructureFinderLayout
        :title="formatMessage(messages.title)"
        :description="formatMessage(messages.description)"
        :available="available"
        :unavailable-body="formatMessage(messages.unavailableBody)"
        :scanning="scanning"
        :error="error"
        :has-result="!!result"
        :can-scan="canScan"
        @scan="run"
    >
        <template #summary>
            <span v-if="result" class="text-xs text-[var(--color-text-tertiary)]">
                <template v-if="result.lowestY !== null">
                    {{ formatMessage(messages.lowest, { y: result.lowestY }) }} ·
                </template>
                {{ formatMessage(messages.elapsed, { ms: result.elapsedMs }) }}
            </span>
        </template>

        <template #columns>
            <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnX) }}</th>
            <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnZ) }}</th>
            <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnY) }}</th>
            <th class="py-1 pr-3 font-medium">{{ formatMessage(messages.columnDistance) }}</th>
            <th class="py-1 font-medium" />
        </template>

        <template #rows>
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
        </template>

        <template #parameters>
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
                    <StyledInput
                        v-model.number="regions"
                        type="number"
                        :min="1"
                        :max="4096"
                        wrapper-class="w-full"
                        :disabled="scanning || available === false"
                    />
                </label>
                <label class="flex flex-1 flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.threadsLabel) }}
                    </span>
                    <StyledInput
                        v-model.number="threads"
                        type="number"
                        :min="0"
                        :max="64"
                        wrapper-class="w-full"
                        :disabled="scanning || available === false"
                    />
                </label>
            </div>
            <span class="text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.regionsHint) }}
                {{ formatMessage(messages.threadsHint) }}
            </span>

            <label class="flex flex-col gap-1">
                <span class="text-sm font-medium">{{ formatMessage(messages.versionLabel) }}</span>
                <Combobox
                    v-model="version"
                    :options="versionOptions"
                    width="custom"
                    class="w-full"
                    :disabled="scanning || available === false"
                />
            </label>

            <label class="flex flex-col gap-1">
                <span class="text-sm font-medium">{{ formatMessage(messages.presetLabel) }}</span>
                <Combobox
                    v-model="preset"
                    :options="presetOptions"
                    width="custom"
                    class="w-full"
                    :disabled="scanning || available === false"
                />
            </label>

            <div class="flex gap-2">
                <label class="flex flex-1 flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.maxYLabel) }}
                    </span>
                    <StyledInput
                        v-model.number="maxY"
                        type="number"
                        :min="-64"
                        :max="320"
                        wrapper-class="w-full"
                        :disabled="scanning || available === false"
                    />
                </label>
                <label class="flex flex-1 flex-col gap-1">
                    <span class="text-sm font-medium">
                        {{ formatMessage(messages.phase2Label) }}
                    </span>
                    <StyledInput
                        v-model.number="phase2MaxY"
                        type="number"
                        :min="-64"
                        :max="320"
                        wrapper-class="w-full"
                        :disabled="scanning || available === false"
                    />
                </label>
            </div>
            <span class="text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.maxYHint) }}
            </span>
        </template>
    </StructureFinderLayout>
</template>
