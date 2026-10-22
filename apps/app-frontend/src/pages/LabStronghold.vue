<script setup lang="ts">
import { ClipboardCopyIcon, CompassIcon, PlusIcon, TrashIcon } from '@modrinth/assets'
import { Button, Combobox, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'

import type { Chunk, Input } from '@/lab/stronghold/core'
import {
    appendMeasurements,
    type Measurement,
    MeasurementInputError,
    parseMeasurements,
} from '@/lab/stronghold/measurements'
import { strongholdMessages as messages } from '@/lab/stronghold/messages'
import { contributions } from '@/lab/stronghold/sectors'
import StrongholdPlot from '@/lab/stronghold/StrongholdPlot.vue'

const { formatMessage } = useVIntl()
const observations = ref<Measurement[]>([])
const version = ref<Input['version']>('post119')
const sigma = ref<string | number>('5')
const radius = ref<string | number>('10000')
const topK = ref<string | number>('10')
const teleportY = ref<string | number>('100')
const selected = ref<Chunk>()
const mode = ref<'paste' | 'manual'>('paste')
const command = ref('')
const x = ref<string | number>('')
const z = ref<string | number>('')
const angle = ref<string | number>('')
const inputError = ref('')
const copyFeedback = ref('')
const busy = ref(false)
const failed = ref(false)
const result = ref<{ predictions: Chunk[] } | null>(null)
const heatmap = shallowRef(new Float64Array())
const storageKey = 'axolotl.lab.stronghold.sectors.v1'
let worker: Worker | undefined
let generation = 0

// Keep an in-progress game session when navigating to another launcher page.
try {
    const saved = JSON.parse(sessionStorage.getItem(storageKey) ?? 'null')
    if (
        saved &&
        Array.isArray(saved.observations) &&
        ['pre119', 'post119'].includes(saved.version) &&
        Number(saved.sigma) > 0 &&
        Number(saved.sigma) <= 90 &&
        Number(saved.radius) >= 1000 &&
        Number(saved.radius) <= 26000
    ) {
        observations.value = appendMeasurements([], saved.observations)
        version.value = saved.version
        sigma.value = String(saved.sigma)
        radius.value = String(saved.radius)
    }
} catch {
    // Storage is optional; malformed or unavailable session data starts a fresh calculation.
}

const validSigma = computed(
    () =>
        String(sigma.value).trim() !== '' &&
        Number.isFinite(Number(sigma.value)) &&
        Number(sigma.value) > 0 &&
        Number(sigma.value) <= 90 &&
        Number.isFinite(Number(radius.value)) &&
        Number(radius.value) >= 1000 &&
        Number(radius.value) <= 26000,
)
const best = computed(() => result.value?.predictions[0])
const selectedChunk = computed(() => selected.value ?? best.value)
const sectorInput = computed(() => ({
    observations: observations.value.map(({ x, z, angle }) => ({ x, z, angle })),
    version: version.value,
    halfAngle: Number(sigma.value),
    radius: Number(radius.value),
}))
const selectedContributions = computed(() =>
    selectedChunk.value ? contributions(sectorInput.value, selectedChunk.value) : [],
)
const candidates = computed(
    () =>
        result.value?.predictions.slice(0, Math.max(1, Math.min(100, Number(topK.value) || 10))) ??
        [],
)
function supporters(chunk: Chunk) {
    return contributions(sectorInput.value, chunk)
        .map((item) => item.index)
        .join(', ')
}
function selectChunk(chunk: Chunk) {
    selected.value = chunk
    copyFeedback.value = ''
}

const versions = computed(() => [
    { value: 'post119', label: formatMessage(messages.post119) },
    { value: 'pre119', label: formatMessage(messages.pre119) },
])

watch(
    [observations, version, sigma, radius],
    () => {
        const id = ++generation
        worker?.terminate()
        worker = undefined
        result.value = null
        selected.value = undefined
        heatmap.value = new Float64Array()
        failed.value = false
        busy.value = false
        copyFeedback.value = ''
        try {
            sessionStorage.setItem(
                storageKey,
                JSON.stringify({
                    observations: observations.value.map(({ x, z, angle }) => ({ x, z, angle })),
                    version: version.value,
                    sigma: validSigma.value ? sigma.value : '5',
                    radius: validSigma.value ? radius.value : '10000',
                }),
            )
        } catch {
            // Calculation remains usable without storage access.
        }
        if (!observations.value.length || !validSigma.value) return
        busy.value = true
        try {
            if (!worker) {
                worker = new Worker(
                    new URL('../lab/stronghold/calculator.worker.ts', import.meta.url),
                    { type: 'module' },
                )
                worker.onmessage = (
                    event: MessageEvent<{
                        id: number
                        result?: { predictions: Chunk[] }
                        error?: boolean
                        heatmap?: Float64Array
                    }>,
                ) => {
                    if (event.data.id !== generation) return
                    busy.value = false
                    failed.value = !!event.data.error
                    result.value = event.data.result ?? null
                    heatmap.value = event.data.heatmap ?? new Float64Array()
                }
                worker.onerror = () => {
                    busy.value = false
                    failed.value = true
                    worker?.terminate()
                    worker = undefined
                }
            }
            worker.postMessage({
                id,
                input: {
                    version: version.value,
                    halfAngle: Number(sigma.value),
                    radius: Number(radius.value),
                    observations: observations.value.map(({ x, z, angle }) => ({ x, z, angle })),
                },
            })
        } catch {
            busy.value = false
            failed.value = true
        }
    },
    { deep: true, immediate: true },
)

onBeforeUnmount(() => worker?.terminate())
watch([mode, command, x, z, angle], () => {
    inputError.value = ''
})

function addMeasurement() {
    try {
        if (
            mode.value === 'manual' &&
            [x.value, z.value, angle.value].some((value) => !String(value).trim())
        ) {
            throw new MeasurementInputError('format')
        }
        const added =
            mode.value === 'paste'
                ? parseMeasurements(command.value)
                : [{ x: Number(x.value), z: Number(z.value), angle: Number(angle.value) }]
        observations.value = appendMeasurements(observations.value, added)
        command.value = ''
        x.value = z.value = angle.value = ''
        inputError.value = ''
    } catch (error) {
        inputError.value = formatMessage(
            messages[error instanceof MeasurementInputError ? error.reason : 'format'],
        )
    }
}

function clearMeasurements() {
    observations.value = []
    inputError.value = ''
}

function coordinates(chunk: Chunk, nether = false) {
    return nether ? `${chunk.x * 2}, ${chunk.z * 2}` : `${chunk.x * 16 + 4}, ${chunk.z * 16 + 4}`
}

async function copyCoordinates(chunk: Chunk, teleport = false) {
    try {
        const y = Number(teleportY.value)
        if (
            teleport &&
            (!String(teleportY.value).trim() || !Number.isFinite(y) || y < -64 || y > 320)
        )
            return
        await navigator.clipboard.writeText(
            teleport
                ? `/execute in minecraft:overworld run tp @s ${chunk.x * 16 + 4} ${y} ${chunk.z * 16 + 4}`
                : coordinates(chunk),
        )
        copyFeedback.value = formatMessage(messages.copied)
    } catch {
        copyFeedback.value = formatMessage(messages.copyError)
    }
}
</script>

<template>
    <main class="flex min-w-0 w-full flex-col gap-6 p-6 pb-24">
        <header>
            <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h1>
            <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </header>
        <div
            class="grid min-w-0 items-start gap-6 xl:grid-cols-[minmax(20rem,0.9fr)_minmax(0,1.1fr)]"
        >
            <div class="flex min-w-0 flex-col gap-4">
                <section
                    class="flex min-w-0 flex-col gap-4 rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
                >
                    <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.input) }}
                    </h2>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            :type="mode === 'paste' ? 'highlight' : 'transparent'"
                            :aria-pressed="mode === 'paste'"
                            @click="mode = 'paste'"
                            >{{ formatMessage(messages.paste) }}</Button
                        >
                        <Button
                            :type="mode === 'manual' ? 'highlight' : 'transparent'"
                            :aria-pressed="mode === 'manual'"
                            @click="mode = 'manual'"
                            >{{ formatMessage(messages.manual) }}</Button
                        >
                    </div>
                    <form class="flex min-w-0 flex-col gap-3" @submit.prevent="addMeasurement">
                        <template v-if="mode === 'paste'">
                            <p
                                id="stronghold-paste-help"
                                class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.pasteHelp) }}
                            </p>
                            <StyledInput
                                v-model="command"
                                multiline
                                :rows="3"
                                :spellcheck="false"
                                :aria-label="formatMessage(messages.paste)"
                                aria-describedby="stronghold-paste-help"
                                :placeholder="formatMessage(messages.placeholder)"
                                :error="!!inputError"
                                wrapper-class="w-full"
                                input-class="font-mono text-sm"
                            />
                        </template>
                        <div v-else class="grid grid-cols-3 gap-3">
                            <label class="flex min-w-0 flex-col gap-2 text-sm font-semibold"
                                >X<StyledInput
                                    v-model="x"
                                    type="number"
                                    :step="0.000001"
                                    aria-label="X"
                                    wrapper-class="w-full"
                            /></label>
                            <label class="flex min-w-0 flex-col gap-2 text-sm font-semibold"
                                >Z<StyledInput
                                    v-model="z"
                                    type="number"
                                    :step="0.000001"
                                    aria-label="Z"
                                    wrapper-class="w-full"
                            /></label>
                            <label class="flex min-w-0 flex-col gap-2 text-sm font-semibold"
                                >{{ formatMessage(messages.yaw)
                                }}<StyledInput
                                    v-model="angle"
                                    type="number"
                                    :step="0.000001"
                                    :aria-label="formatMessage(messages.yaw)"
                                    wrapper-class="w-full"
                            /></label>
                        </div>
                        <p v-if="inputError" role="alert" class="m-0 text-sm text-red">
                            {{ inputError }}
                        </p>
                        <Button native-type="submit" type="colored" color="brand" class="self-start"
                            ><PlusIcon class="size-4" />{{ formatMessage(messages.add) }}</Button
                        >
                    </form>
                </section>
                <section
                    class="flex min-w-0 flex-col gap-3 rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
                >
                    <div class="flex items-center justify-between gap-3">
                        <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                            {{
                                formatMessage(messages.observations, { count: observations.length })
                            }}
                        </h2>
                        <Button
                            v-if="observations.length"
                            size="sm"
                            type="transparent"
                            @click="clearMeasurements"
                            >{{ formatMessage(messages.clear) }}</Button
                        >
                    </div>
                    <p
                        v-if="!observations.length"
                        class="m-0 py-3 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.noMeasurements) }}
                    </p>
                    <ol
                        v-else
                        class="m-0 max-h-80 list-none overflow-y-auto p-0"
                        data-testid="measurements"
                    >
                        <li
                            v-for="(item, index) in observations"
                            :key="`${item.x}:${item.z}:${item.angle}`"
                            class="flex items-center gap-3 border-0 border-b border-solid border-surface-4 py-3 last:border-b-0"
                        >
                            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                                index + 1
                            }}</span>
                            <div class="min-w-0 flex-1 font-mono text-sm tabular-nums">
                                <div class="break-words">X {{ item.x }} · Z {{ item.z }}</div>
                                <div class="mt-1 text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.yaw) }} {{ item.angle.toFixed(3) }}
                                </div>
                            </div>
                            <Button
                                :icon-only="true"
                                :label="formatMessage(messages.remove, { index: index + 1 })"
                                type="transparent"
                                size="sm"
                                @click="observations = observations.filter((_, i) => i !== index)"
                                ><TrashIcon class="size-4"
                            /></Button>
                        </li>
                    </ol>
                </section>
                <section
                    class="flex min-w-0 flex-col gap-4 rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
                >
                    <label class="flex flex-col gap-2 text-sm font-semibold"
                        >{{ formatMessage(messages.version)
                        }}<Combobox
                            v-model="version"
                            :options="versions"
                            width="full"
                            :aria-label="formatMessage(messages.version)"
                    /></label>
                    <label class="flex flex-col gap-2 text-sm font-semibold">
                        {{ formatMessage(messages.precision) }}
                        <StyledInput
                            v-model="sigma"
                            type="number"
                            :min="0.01"
                            :max="90"
                            :step="0.1"
                            :error="!validSigma"
                            :aria-label="formatMessage(messages.precision)"
                            wrapper-class="w-full"
                        />
                    </label>
                    <label class="flex flex-col gap-2 text-sm font-semibold">
                        {{ formatMessage(messages.searchRadius) }}
                        <StyledInput
                            v-model="radius"
                            type="number"
                            :min="1000"
                            :max="26000"
                            :step="1000"
                            :aria-label="formatMessage(messages.searchRadius)"
                            wrapper-class="w-full"
                        />
                    </label>
                    <p v-if="!validSigma" role="alert" class="m-0 text-sm text-red">
                        {{ formatMessage(messages.sigmaError) }}
                    </p>
                </section>
            </div>
            <section
                class="flex min-w-0 flex-col gap-4 rounded-xl border border-solid border-surface-4 bg-surface-2 p-5"
                aria-live="polite"
                :aria-busy="busy"
                data-testid="stronghold-results"
            >
                <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.results) }}
                </h2>
                <div
                    v-if="!observations.length"
                    class="flex flex-1 flex-col items-center justify-center gap-3 py-16 text-center"
                >
                    <CompassIcon class="size-12 text-[var(--color-text-tertiary)]" />
                    <h3 class="m-0 text-lg font-semibold">{{ formatMessage(messages.waiting) }}</h3>
                    <p
                        class="m-0 max-w-sm text-sm leading-relaxed text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.waitingHelp) }}
                    </p>
                </div>
                <p v-else-if="busy" role="status">{{ formatMessage(messages.calculating) }}</p>
                <div v-else-if="failed || !validSigma || !best" class="py-12">
                    <h3 class="m-0 text-lg font-semibold">
                        {{ formatMessage(messages.noResult) }}
                    </h3>
                    <p class="text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(
                                failed
                                    ? messages.computeError
                                    : !validSigma
                                      ? messages.sigmaError
                                      : messages.noResultHelp,
                            )
                        }}
                    </p>
                </div>
                <template v-else>
                    <label class="flex items-center gap-3 text-sm">
                        Top-K
                        <StyledInput
                            v-model="topK"
                            type="number"
                            :min="1"
                            :max="100"
                            aria-label="Top-K"
                        />
                    </label>
                    <div
                        class="max-h-96 overflow-auto rounded-xl border border-solid border-surface-4"
                    >
                        <table class="w-full border-collapse text-left text-sm">
                            <thead class="bg-surface-3 text-[var(--color-text-tertiary)]">
                                <tr>
                                    <th class="p-3">{{ formatMessage(messages.chunk) }}</th>
                                    <th class="p-3">X/Z</th>
                                    <th class="p-3">{{ formatMessage(messages.probability) }}</th>
                                    <th class="p-3">{{ formatMessage(messages.supporters) }}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr
                                    v-for="candidate in candidates"
                                    :key="`${candidate.x}:${candidate.z}`"
                                    class="border-0 border-t border-solid border-surface-4"
                                    :class="
                                        selectedChunk?.x === candidate.x &&
                                        selectedChunk?.z === candidate.z
                                            ? 'bg-surface-3'
                                            : ''
                                    "
                                >
                                    <td class="p-3">
                                        <Button
                                            size="sm"
                                            type="transparent"
                                            @click="selectChunk(candidate)"
                                            >{{ candidate.x }}, {{ candidate.z }}</Button
                                        >
                                    </td>
                                    <td class="whitespace-nowrap p-3 font-mono">
                                        {{ coordinates(candidate) }}
                                    </td>
                                    <td class="p-3 tabular-nums">
                                        {{ candidate.weight.toFixed(3) }}
                                    </td>
                                    <td class="p-3 font-mono">{{ supporters(candidate) }}</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </template>
                <div
                    v-if="selectedChunk && validSigma && !busy"
                    class="flex flex-col gap-3 rounded-xl border border-solid border-surface-4 bg-surface-3 p-4"
                    data-testid="selected-chunk"
                >
                    <h3 class="m-0 text-base font-semibold">
                        {{ formatMessage(messages.selected) }}
                    </h3>
                    <div class="font-mono" data-testid="best-chunk">
                        {{ selectedChunk.x }}, {{ selectedChunk.z }}
                    </div>
                    <div class="select-text font-mono text-lg">
                        {{ coordinates(selectedChunk) }}
                    </div>
                    <div>
                        {{ formatMessage(messages.probability) }}:
                        {{ selectedChunk.weight.toFixed(3) }}
                    </div>
                    <div>
                        {{ formatMessage(messages.supporters) }}:
                        {{
                            selectedContributions
                                .map((item) => `${item.index} (${item.weight.toFixed(2)})`)
                                .join(', ') || '—'
                        }}
                    </div>
                    <label class="flex items-center gap-3 text-sm"
                        >Y
                        <StyledInput
                            v-model="teleportY"
                            type="number"
                            :min="-64"
                            :max="320"
                            aria-label="Y"
                    /></label>
                    <div class="flex flex-wrap gap-2">
                        <Button size="sm" @click="copyCoordinates(selectedChunk)"
                            ><ClipboardCopyIcon class="size-4" />{{
                                formatMessage(messages.copy)
                            }}</Button
                        >
                        <Button
                            size="sm"
                            :disabled="
                                !String(teleportY).trim() ||
                                !Number.isFinite(Number(teleportY)) ||
                                Number(teleportY) < -64 ||
                                Number(teleportY) > 320
                            "
                            @click="copyCoordinates(selectedChunk, true)"
                            >{{ formatMessage(messages.copyTeleport) }}</Button
                        >
                    </div>
                    <p v-if="copyFeedback" role="status" class="m-0 text-sm">{{ copyFeedback }}</p>
                </div>
            </section>
        </div>
        <StrongholdPlot
            :observations="observations"
            :heatmap="heatmap"
            :best="best"
            :selected="selected"
            :half-angle="Number(sigma)"
            :radius="Number(radius)"
            :target-offset="version === 'pre119' ? 8 : 0"
            @select="selectChunk"
        />
    </main>
</template>
