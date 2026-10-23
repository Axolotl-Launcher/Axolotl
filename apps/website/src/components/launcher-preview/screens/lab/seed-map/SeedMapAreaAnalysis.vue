<script setup lang="ts">
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import {
    analyzeAreaOres,
    areaChunkCount,
    type AreaLootMatch,
    type AreaLootResult,
    type AreaOreResult,
    type AreaProgress,
    searchAreaLoot,
    SEED_MAP_AREA_LOOT_MAX_CHUNKS,
    SEED_MAP_AREA_ORE_MAX_CHUNKS,
    type SeedMapArea,
    type SeedMapAreaContext,
} from './area-search.ts'
import type { SeedMapFeatureKind } from './features.ts'
import type { SeedMapOreKind } from './ores.ts'
import { SeedMapPreviewLimit, type SeedMapPreviewLimitReason } from './preview-data.ts'
import SeedMapPreviewNotice from './SeedMapPreviewNotice.vue'
import { createSeedMapLabels } from './structure-labels.ts'

const props = defineProps<{
    area: SeedMapArea
    context: SeedMapAreaContext
    ores: SeedMapOreKind[]
    yRange: readonly [number, number]
    featureMask: number
    featureLabel: (kind: SeedMapFeatureKind) => string
    oreLabel: (kind: SeedMapOreKind) => string
}>()
const emit = defineEmits<{ close: []; locate: [match: AreaLootMatch] }>()
const { formatMessage, locale } = useVIntl()
const labels = computed(() =>
    createSeedMapLabels((message, values) => formatMessage(message, values), locale.value),
)
const messages = defineMessages({
    title: { id: 'app.lab.seed-map.area.title', defaultMessage: 'Area analysis' },
    close: { id: 'app.lab.seed-map.area.close', defaultMessage: 'Close area analysis' },
    ores: { id: 'app.lab.seed-map.area.ores', defaultMessage: 'Minerals' },
    loot: { id: 'app.lab.seed-map.area.loot', defaultMessage: 'Find loot' },
    height: { id: 'app.lab.seed-map.area.height', defaultMessage: 'Height range' },
    minY: { id: 'app.lab.seed-map.area.min-y', defaultMessage: 'Minimum Y' },
    maxY: { id: 'app.lab.seed-map.area.max-y', defaultMessage: 'Maximum Y' },
    analyze: { id: 'app.lab.seed-map.area.analyze', defaultMessage: 'Count mineral blocks' },
    search: { id: 'app.lab.seed-map.area.search', defaultMessage: 'Search container loot' },
    query: { id: 'app.lab.seed-map.area.query', defaultMessage: 'Item name or minecraft:item_id' },
    cancel: { id: 'app.lab.seed-map.area.cancel', defaultMessage: 'Cancel' },
    progress: {
        id: 'app.lab.seed-map.area.progress',
        defaultMessage: 'Processing {done} / {total}',
    },
    discover: {
        id: 'app.lab.seed-map.area.discover',
        defaultMessage: 'Finding structures {done} / {total}',
    },
    oreLimit: {
        id: 'app.lab.seed-map.area.ore-limit',
        defaultMessage:
            'Candidate block counts only; terrain replacements are not verified. Regular and deepslate ores are combined; overlapping vein candidates count once. These are not guaranteed mining yields.',
    },
    lootLimit: {
        id: 'app.lab.seed-map.area.loot-limit',
        defaultMessage:
            'Searches supported containers inside this X/Z area at all heights, including structures starting up to 512 blocks outside it. More distant structure starts and unsupported loot tables are not searched.',
    },
    tooLarge: {
        id: 'app.lab.seed-map.area.too-large',
        defaultMessage: 'Select at most {max} chunks to analyze this area.',
    },
    unavailable: {
        id: 'app.lab.seed-map.area.unavailable',
        defaultMessage: 'Mineral prediction is unavailable for this version or dimension.',
    },
    invalidY: {
        id: 'app.lab.seed-map.area.invalid-y',
        defaultMessage: 'Enter a valid height range between {min} and {max}.',
    },
    partial: {
        id: 'app.lab.seed-map.area.partial',
        defaultMessage:
            'Results are incomplete: a prediction was partial, a query failed, or a result limit was reached.',
    },
    canceled: {
        id: 'app.lab.seed-map.area.canceled',
        defaultMessage: 'Canceled. Start again to analyze the selected area.',
    },
    failed: {
        id: 'app.lab.seed-map.area.failed',
        defaultMessage: 'Analysis failed. Try again with this area.',
    },
    noOres: {
        id: 'app.lab.seed-map.area.no-ores',
        defaultMessage: 'No mineral candidates were found in this area and height range.',
    },
    noLoot: {
        id: 'app.lab.seed-map.area.no-loot',
        defaultMessage: 'No matching loot was found in the searched containers.',
    },
    checked: {
        id: 'app.lab.seed-map.area.checked',
        defaultMessage: '{count} containers searched · {unsupported} unsupported · {failed} failed',
    },
    missing: {
        id: 'app.lab.seed-map.area.missing',
        defaultMessage: 'Some loot subtables are missing; the search is incomplete.',
    },
    estimated: { id: 'app.lab.seed-map.area.estimated', defaultMessage: 'Estimated loot' },
    positionEstimated: {
        id: 'app.lab.seed-map.area.position-estimated',
        defaultMessage: 'Chest coordinates and inclusion in this area are approximate.',
    },
    locate: { id: 'app.lab.seed-map.area.locate', defaultMessage: 'Locate chest' },
    snapshot: { id: 'app.lab.seed-map.area.snapshot', defaultMessage: 'Results for “{query}”' },
    mineral: { id: 'app.lab.seed-map.area.mineral', defaultMessage: 'Mineral' },
    blocks: { id: 'app.lab.seed-map.area.blocks', defaultMessage: 'Candidate blocks' },
    container: { id: 'app.lab.seed-map.area.container', defaultMessage: 'Chest {number}' },
    chunks: {
        id: 'app.lab.seed-map.area.chunks',
        defaultMessage: '{count} chunks · {width} × {depth} blocks',
    },
})
const tab = ref<'ores' | 'loot'>('ores')
const minY = ref<string | number | undefined>(props.yRange[0])
const maxY = ref<string | number | undefined>(props.yRange[1])
const query = ref('')
const committedQuery = ref('')
const oreResult = ref<AreaOreResult>()
const oreResultY = ref<[number, number]>()
const lootResult = ref<AreaLootResult>()
const busy = ref(false)
const status = ref<'idle' | 'canceled' | 'failed'>('idle')
/** Set when the analysis reached past the preview's data. */
const previewLimit = ref<SeedMapPreviewLimitReason | null>(null)
const progress = ref<AreaProgress>({ done: 0, total: 0 })
let active: AbortController | undefined
const chunkCount = computed(() => areaChunkCount(props.area))
const maxChunks = computed(() =>
    tab.value === 'ores' ? SEED_MAP_AREA_ORE_MAX_CHUNKS : SEED_MAP_AREA_LOOT_MAX_CHUNKS,
)
const tooLarge = computed(() => chunkCount.value > maxChunks.value)
const validY = computed(
    () =>
        Number.isInteger(Number(minY.value)) &&
        Number.isInteger(Number(maxY.value)) &&
        String(minY.value ?? '').trim() !== '' &&
        String(maxY.value ?? '').trim() !== '' &&
        Number(minY.value) >= props.yRange[0] &&
        Number(maxY.value) <= props.yRange[1] &&
        Number(minY.value) <= Number(maxY.value),
)
function cancel() {
    active?.abort()
    active = undefined
    busy.value = false
    status.value = 'canceled'
}
async function run() {
    if (
        busy.value ||
        tooLarge.value ||
        (tab.value === 'ores' && (!validY.value || !props.ores.length)) ||
        (tab.value === 'loot' && !query.value.trim())
    )
        return
    const controller = new AbortController()
    active = controller
    busy.value = true
    status.value = 'idle'
    progress.value = { done: 0, total: 0 }
    const update = (value: AreaProgress) => {
        if (active === controller) progress.value = value
    }
    try {
        if (tab.value === 'ores') {
            const range: [number, number] = [Number(minY.value), Number(maxY.value)]
            const result = await analyzeAreaOres(
                props.context,
                props.area,
                props.ores,
                range[0],
                range[1],
                controller.signal,
                update,
            )
            if (active === controller) {
                oreResult.value = result
                oreResultY.value = range
            }
        } else {
            const text = query.value.trim()
            const search = text.toLocaleLowerCase()
            const result = await searchAreaLoot(
                props.context,
                props.area,
                props.featureMask,
                (id) =>
                    id.toLocaleLowerCase().includes(search) ||
                    labels.value.item(id).toLocaleLowerCase().includes(search),
                controller.signal,
                update,
            )
            if (active === controller) {
                lootResult.value = result
                committedQuery.value = text
            }
        }
    } catch (error) {
        if (active !== controller) return
        status.value = 'failed'
        previewLimit.value = error instanceof SeedMapPreviewLimit ? error.reason : null
    } finally {
        if (active === controller) {
            busy.value = false
            active = undefined
        }
    }
}
watch(
    () => JSON.stringify([props.context, props.area]),
    () => {
        cancel()
        oreResult.value = undefined
        lootResult.value = undefined
        status.value = 'idle'
    },
)
onBeforeUnmount(cancel)
</script>

<template>
    <section
        class="flex min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-y-auto p-4"
        data-testid="seed-map-area-analysis"
        :aria-busy="busy"
    >
        <div class="flex items-center justify-between gap-2">
            <strong>{{ formatMessage(messages.title) }}</strong>
            <Button
                type="transparent"
                size="icon"
                :aria-label="formatMessage(messages.close)"
                @click="emit('close')"
                ><XIcon
            /></Button>
        </div>
        <div class="text-xs text-[var(--color-text-tertiary)]">
            X {{ area.minX }}…{{ area.maxX }} · Z {{ area.minZ }}…{{ area.maxZ }}
            <p class="mb-0 mt-1">
                {{
                    formatMessage(messages.chunks, {
                        count: chunkCount,
                        width: area.maxX - area.minX + 1,
                        depth: area.maxZ - area.minZ + 1,
                    })
                }}
            </p>
        </div>
        <div class="flex gap-2" role="tablist">
            <Button
                v-for="value in ['ores', 'loot'] as const"
                :key="value"
                role="tab"
                :aria-selected="tab === value"
                :disabled="busy"
                :type="tab === value ? 'colored' : 'outlined'"
                @click="tab = value"
                >{{ formatMessage(messages[value]) }}</Button
            >
        </div>
        <p v-if="tooLarge" class="m-0 text-sm" role="alert">
            {{ formatMessage(messages.tooLarge, { max: maxChunks }) }}
        </p>
        <template v-if="tab === 'ores'">
            <label class="flex flex-col gap-2 text-sm">
                {{ formatMessage(messages.height) }}
                <div class="grid grid-cols-2 gap-2">
                    <StyledInput
                        v-model="minY"
                        type="number"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.minY)"
                    />
                    <StyledInput
                        v-model="maxY"
                        type="number"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.maxY)"
                    />
                </div>
            </label>
            <p v-if="!validY" class="m-0 text-xs" role="alert">
                {{ formatMessage(messages.invalidY, { min: yRange[0], max: yRange[1] }) }}
            </p>
            <p v-if="!ores.length" class="m-0 text-sm">{{ formatMessage(messages.unavailable) }}</p>
            <Button :disabled="busy || tooLarge || !validY || !ores.length" @click="run">{{
                formatMessage(messages.analyze)
            }}</Button>
            <template v-if="oreResult && !busy">
                <strong class="text-sm">Y {{ oreResultY?.[0] }}…{{ oreResultY?.[1] }}</strong>
                <p v-if="oreResult.truncated" class="m-0 text-sm" role="alert">
                    {{ formatMessage(messages.partial) }}
                </p>
                <table v-if="oreResult.counts.length" class="w-full text-sm">
                    <thead>
                        <tr>
                            <th class="text-left">{{ formatMessage(messages.mineral) }}</th>
                            <th class="text-right">{{ formatMessage(messages.blocks) }}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr v-for="row in oreResult.counts" :key="row.ore">
                            <td class="py-1">{{ oreLabel(row.ore) }}</td>
                            <td class="text-right tabular-nums">
                                {{ row.count.toLocaleString() }}
                            </td>
                        </tr>
                    </tbody>
                </table>
                <p v-if="!oreResult.counts.some((row) => row.count)" class="m-0 text-sm">
                    {{ formatMessage(messages.noOres) }}
                </p>
            </template>
        </template>
        <template v-else>
            <StyledInput
                v-model="query"
                :disabled="busy"
                :placeholder="formatMessage(messages.query)"
                :aria-label="formatMessage(messages.query)"
                @keydown.enter="run"
            />
            <Button :disabled="busy || tooLarge || !query.trim()" @click="run">{{
                formatMessage(messages.search)
            }}</Button>
            <template v-if="lootResult && !busy">
                <strong class="text-sm">{{
                    formatMessage(messages.snapshot, { query: committedQuery })
                }}</strong>
                <p class="m-0 text-xs">
                    {{
                        formatMessage(messages.checked, {
                            count: lootResult.containers,
                            unsupported: lootResult.unsupported,
                            failed: lootResult.failed,
                        })
                    }}
                </p>
                <SeedMapPreviewNotice
                    v-if="lootResult.previewLimit"
                    :reason="lootResult.previewLimit"
                />
                <p v-if="lootResult.unresolved" class="m-0 text-xs">
                    {{ formatMessage(messages.missing) }}
                </p>
                <p v-if="!lootResult.matches.length" class="m-0 text-sm">
                    {{ formatMessage(messages.noLoot) }}
                </p>
                <div
                    v-for="match in lootResult.matches"
                    :key="`${match.feature.kind}:${match.feature.x}:${match.feature.z}:${match.container.id}`"
                    class="rounded-md border border-surface-4 bg-surface-3 p-3 text-sm"
                >
                    <strong
                        >{{ featureLabel(match.feature.kind) }} ·
                        {{
                            formatMessage(messages.container, { number: match.container.id + 1 })
                        }}</strong
                    >
                    <p class="my-1 text-xs">
                        X {{ match.container.x }} · Z {{ match.container.z }}
                    </p>
                    <p v-if="match.estimated" class="my-1 text-xs">
                        {{ formatMessage(messages.estimated) }}
                    </p>
                    <p v-if="match.positionEstimated" class="my-1 text-xs">
                        {{ formatMessage(messages.positionEstimated) }}
                    </p>
                    <ul class="my-2 pl-4">
                        <li v-for="(item, index) in match.items" :key="index" :title="item.id">
                            {{ labels.item(item.id) }} ×{{ item.count
                            }}<span
                                v-for="enchantment in item.enchantments"
                                :key="enchantment.id"
                                class="block text-xs text-[var(--color-text-tertiary)]"
                                >{{ labels.enchantment(enchantment.id) }}
                                {{ labels.level(enchantment.level) }}</span
                            ><span v-if="item.effect" class="block text-xs">{{
                                labels.effect(item.effect)
                            }}</span>
                        </li>
                    </ul>
                    <Button size="2xs" type="outlined" @click="emit('locate', match)">{{
                        formatMessage(messages.locate)
                    }}</Button>
                </div>
            </template>
        </template>
        <div v-if="busy" class="flex flex-col gap-2 text-sm" role="status">
            <progress
                class="h-2 w-full accent-[var(--color-purple)]"
                :max="Math.max(1, progress.total)"
                :value="progress.done"
                :aria-label="formatMessage(messages.title)"
            />
            <div class="flex items-center justify-between gap-2">
                <span>{{
                    formatMessage(
                        progress.phase === 'structures' ? messages.discover : messages.progress,
                        { done: Math.floor(progress.done), total: progress.total },
                    )
                }}</span>
                <Button type="outlined" size="2xs" @click="cancel">{{
                    formatMessage(messages.cancel)
                }}</Button>
            </div>
        </div>
        <SeedMapPreviewNotice v-if="status === 'failed' && previewLimit" :reason="previewLimit" />
        <p v-else-if="status !== 'idle'" class="m-0 text-sm" role="status">
            {{ formatMessage(messages[status]) }}
        </p>
    </section>
</template>
