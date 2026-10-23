<script setup lang="ts">
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import LayersIcon from '@modrinth/assets/icons/layers.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import {
    SEED_MAP_BIOME_NAMES,
    SEED_MAP_BIOMES,
    type SeedMapBiomeCategory,
    seedMapBiomeGroups,
    seedMapBiomeMessage,
    type SeedMapDimension,
} from './core.ts'

const props = defineProps<{
    dimension: SeedMapDimension
    enabled: boolean
    highlightedBiomes: number[]
    availableBiomeIds?: number[]
    availableDimensions?: SeedMapDimension[]
    /** Container used for the popover while the map is in browser fullscreen. */
    container?: HTMLElement | string | boolean
}>()

const emit = defineEmits<{
    'update:dimension': [dimension: SeedMapDimension]
    'update:enabled': [enabled: boolean]
    'update:highlightedBiomes': [biomes: number[]]
}>()

const messages = defineMessages({
    biomeHighlight: { id: 'app.lab.seed-map.biome-highlight', defaultMessage: 'Biome highlight' },
    chooseBiome: { id: 'app.lab.seed-map.choose-biome', defaultMessage: 'Choose biome' },
    selectedBiomes: {
        id: 'app.lab.seed-map.selected-biomes',
        defaultMessage: '{count, plural, one {# biome} other {# biomes}} selected',
    },
    selectionCount: {
        id: 'app.lab.seed-map.biome-selection-count',
        defaultMessage: '{selected} of {total}',
    },
    search: { id: 'app.lab.seed-map.search-biomes', defaultMessage: 'Search biomes' },
    selectAll: { id: 'app.lab.seed-map.select-all', defaultMessage: 'Select all' },
    invert: { id: 'app.lab.seed-map.invert-selection', defaultMessage: 'Invert' },
    clear: { id: 'app.lab.seed-map.clear', defaultMessage: 'Clear' },
    noMatches: {
        id: 'app.lab.seed-map.no-matching-biomes',
        defaultMessage: 'No matching biomes',
    },
    overworld: { id: 'app.lab.seed-map.dimension.overworld', defaultMessage: 'Overworld' },
    nether: { id: 'app.lab.seed-map.dimension.nether', defaultMessage: 'Nether' },
    end: { id: 'app.lab.seed-map.dimension.end', defaultMessage: 'The End' },
    groupBeach: { id: 'app.lab.seed-map.biome-group.beach', defaultMessage: 'Beaches' },
    groupCave: { id: 'app.lab.seed-map.biome-group.cave', defaultMessage: 'Caves' },
    groupDesert: { id: 'app.lab.seed-map.biome-group.desert', defaultMessage: 'Desert' },
    groupForest: { id: 'app.lab.seed-map.biome-group.forest', defaultMessage: 'Forests' },
    groupIce: { id: 'app.lab.seed-map.biome-group.ice', defaultMessage: 'Icy biomes' },
    groupJungle: { id: 'app.lab.seed-map.biome-group.jungle', defaultMessage: 'Jungles' },
    groupMesa: { id: 'app.lab.seed-map.biome-group.mesa', defaultMessage: 'Badlands' },
    groupMountains: { id: 'app.lab.seed-map.biome-group.mountains', defaultMessage: 'Mountains' },
    groupMushroom: { id: 'app.lab.seed-map.biome-group.mushroom', defaultMessage: 'Mushroom' },
    groupOcean: { id: 'app.lab.seed-map.biome-group.ocean', defaultMessage: 'Oceans' },
    groupPlains: { id: 'app.lab.seed-map.biome-group.plains', defaultMessage: 'Plains' },
    groupRiver: { id: 'app.lab.seed-map.biome-group.river', defaultMessage: 'Rivers' },
    groupSavanna: { id: 'app.lab.seed-map.biome-group.savanna', defaultMessage: 'Savannas' },
    groupSwamp: { id: 'app.lab.seed-map.biome-group.swamp', defaultMessage: 'Swamps' },
    groupTaiga: { id: 'app.lab.seed-map.biome-group.taiga', defaultMessage: 'Taiga' },
    groupNether: { id: 'app.lab.seed-map.biome-group.nether', defaultMessage: 'Nether' },
    groupEnd: { id: 'app.lab.seed-map.biome-group.end', defaultMessage: 'The End' },
})

const { formatMessage } = useVIntl()
const search = ref('')

const categoryMessages: Record<SeedMapBiomeCategory, (typeof messages)[keyof typeof messages]> = {
    beach: messages.groupBeach,
    cave: messages.groupCave,
    desert: messages.groupDesert,
    forest: messages.groupForest,
    ice: messages.groupIce,
    jungle: messages.groupJungle,
    mesa: messages.groupMesa,
    mountains: messages.groupMountains,
    mushroom: messages.groupMushroom,
    ocean: messages.groupOcean,
    plains: messages.groupPlains,
    river: messages.groupRiver,
    savanna: messages.groupSavanna,
    swamp: messages.groupSwamp,
    taiga: messages.groupTaiga,
    nether: messages.groupNether,
    end: messages.groupEnd,
}

const dimensionMessages: Record<SeedMapDimension, (typeof messages)[keyof typeof messages]> = {
    overworld: messages.overworld,
    nether: messages.nether,
    end: messages.end,
}

const availableBiomes = computed(() =>
    SEED_MAP_BIOMES.filter(
        (biome) =>
            (!props.availableBiomeIds || props.availableBiomeIds.includes(biome.id)) &&
            (!props.availableDimensions ||
                biome.dimensions.some((dimension) =>
                    props.availableDimensions?.includes(dimension),
                )),
    ),
)
const currentBiomeIds = computed(() =>
    availableBiomes.value
        .filter((biome) => biome.dimensions.includes(props.dimension))
        .map((biome) => biome.id),
)
const allBiomeIds = computed(() => availableBiomes.value.map((biome) => biome.id))
const activeBiomes = computed(() =>
    props.highlightedBiomes.filter((biome) => currentBiomeIds.value.includes(biome)),
)
const groups = computed(() =>
    seedMapBiomeGroups()
        .map((group) => ({
            ...group,
            biomes: group.biomes.filter((biome) => allBiomeIds.value.includes(biome.id)),
        }))
        .filter((group) => group.biomes.length > 0),
)
const visibleGroups = computed(() => {
    const query = search.value.trim().toLocaleLowerCase()
    if (!query) return groups.value
    return groups.value
        .map((group) => ({
            ...group,
            biomes: group.biomes.filter((biome) => {
                const referenceName = SEED_MAP_BIOME_NAMES[biome.id] ?? ''
                return `${biomeLabel(biome.id)} ${referenceName}`
                    .toLocaleLowerCase()
                    .includes(query)
            }),
        }))
        .filter((group) => group.biomes.length > 0)
})

const enabledModel = computed({
    get: () => props.enabled,
    set: (enabled: boolean) => {
        if (enabled && activeBiomes.value.length === 0 && currentBiomeIds.value[0] !== undefined) {
            emit('update:highlightedBiomes', [...props.highlightedBiomes, currentBiomeIds.value[0]])
        }
        emit('update:enabled', enabled)
    },
})

function biomeLabel(biome: number) {
    const name = SEED_MAP_BIOME_NAMES[biome]
    if (!name) return formatMessage(messages.chooseBiome)
    const descriptor = seedMapBiomeMessage(name)
    return descriptor ? formatMessage(descriptor) : name
}

function biomeSlug(name: string) {
    return name.toLocaleLowerCase().replaceAll(' ', '-')
}

function biomeImageSource(biome: number) {
    const name = SEED_MAP_BIOME_NAMES[biome]
    return name ? `/launcher-preview/seed-map-assets/biomes/${biomeSlug(name)}.webp` : ''
}

function toggleBiome(biome: number, selected: boolean) {
    const definition = SEED_MAP_BIOMES.find((item) => item.id === biome)
    if (!definition) return
    const next = selected
        ? [...new Set([...props.highlightedBiomes, biome])]
        : props.highlightedBiomes.filter((item) => item !== biome)
    emit('update:highlightedBiomes', next)
    emit('update:enabled', next.length > 0)
    const targetDimension = definition.dimensions[0]
    if (selected && targetDimension && targetDimension !== props.dimension) {
        emit('update:dimension', targetDimension)
    }
}

function clearBiomes() {
    emit('update:highlightedBiomes', [])
    emit('update:enabled', false)
}

function selectAllBiomes() {
    emit('update:highlightedBiomes', [
        ...new Set([...props.highlightedBiomes, ...allBiomeIds.value]),
    ])
    emit('update:enabled', true)
}

function invertBiomes() {
    const selected = new Set(props.highlightedBiomes)
    const next = [
        ...props.highlightedBiomes.filter((biome) => !allBiomeIds.value.includes(biome)),
        ...allBiomeIds.value.filter((biome) => !selected.has(biome)),
    ]
    emit('update:highlightedBiomes', next)
    emit('update:enabled', next.length > 0)
}
</script>

<template>
    <div class="biome-cluster">
        <label class="biome-toggle-pill">
            <span>{{ formatMessage(messages.biomeHighlight) }}</span>
            <Toggle v-model="enabledModel" small />
        </label>
        <PopoutMenu
            class="biome-picker-trigger relative inline-flex h-9 min-w-0 flex-1 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-xl border border-solid border-surface-5 bg-transparent px-2.5 text-base font-semibold text-[var(--color-text-primary)] transition-[background-color,color,filter,transform] duration-150 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-5"
            :aria-label="formatMessage(messages.chooseBiome)"
            dropdown-class="seed-map-biome-popout"
            :container="props.container"
            placement="top-start"
        >
            <LayersIcon />
            <span>
                {{
                    props.highlightedBiomes.length === 0
                        ? formatMessage(messages.chooseBiome)
                        : props.highlightedBiomes.length === 1
                          ? biomeLabel(props.highlightedBiomes[0])
                          : formatMessage(messages.selectedBiomes, {
                                count: props.highlightedBiomes.length,
                            })
                }}
            </span>
            <ChevronDownIcon />
            <template #menu>
                <div
                    class="flex max-h-[min(30rem,calc(800px-2rem))] min-h-0 w-[min(30rem,calc(1280px-1.5rem))] flex-col gap-[0.65rem] overflow-hidden"
                >
                    <div
                        class="biome-picker-heading flex items-center justify-between gap-3 p-[0.1rem_0.2rem] text-[var(--color-text-primary)] max-sm:flex-col max-sm:items-start"
                    >
                        <div>
                            <strong>{{ formatMessage(messages.chooseBiome) }}</strong>
                            <small>
                                {{
                                    formatMessage(messages.selectionCount, {
                                        selected: props.highlightedBiomes.length,
                                        total: allBiomeIds.length,
                                    })
                                }}
                            </small>
                        </div>
                        <div
                            class="flex flex-none items-center gap-[0.15rem] max-sm:w-full max-sm:flex-wrap"
                        >
                            <Button type="quiet" size="2xs" @click="selectAllBiomes">
                                {{ formatMessage(messages.selectAll) }}
                            </Button>
                            <Button type="quiet" size="2xs" @click="invertBiomes">
                                {{ formatMessage(messages.invert) }}
                            </Button>
                            <Button
                                type="quiet"
                                size="2xs"
                                :disabled="props.highlightedBiomes.length === 0"
                                @click="clearBiomes"
                            >
                                {{ formatMessage(messages.clear) }}
                            </Button>
                        </div>
                    </div>
                    <StyledInput
                        v-model="search"
                        :icon="SearchIcon"
                        :placeholder="formatMessage(messages.search)"
                        wrapper-class="biome-picker-search"
                    />
                    <div class="biome-picker-groups">
                        <Accordion
                            v-for="group in visibleGroups"
                            :key="group.category"
                            class="biome-picker-group"
                            button-class="biome-picker-group-trigger"
                            content-class="biome-picker-group-options"
                        >
                            <template #title>
                                <strong>{{
                                    formatMessage(categoryMessages[group.category])
                                }}</strong>
                                <span
                                    class="biome-picker-dimension"
                                    :class="{ active: group.dimension === props.dimension }"
                                >
                                    {{ formatMessage(dimensionMessages[group.dimension]) }}
                                </span>
                                <small>
                                    {{
                                        group.biomes.filter((biome) =>
                                            props.highlightedBiomes.includes(biome.id),
                                        ).length
                                    }}/{{ group.biomes.length }}
                                </small>
                            </template>
                            <div class="grid grid-cols-2 gap-[0.45rem] max-sm:grid-cols-1">
                                <Checkbox
                                    v-for="biome in group.biomes"
                                    :key="biome.id"
                                    :model-value="props.highlightedBiomes.includes(biome.id)"
                                    :description="biomeLabel(biome.id)"
                                    @update:model-value="toggleBiome(biome.id, $event)"
                                >
                                    <img
                                        class="biome-picker-icon"
                                        :src="biomeImageSource(biome.id)"
                                        alt=""
                                        aria-hidden="true"
                                        :style="{ '--biome-color': biome.color }"
                                    />
                                    <span class="biome-picker-option-label">{{
                                        biomeLabel(biome.id)
                                    }}</span>
                                </Checkbox>
                            </div>
                        </Accordion>
                        <p v-if="visibleGroups.length === 0" class="biome-picker-empty">
                            {{ formatMessage(messages.noMatches) }}
                        </p>
                    </div>
                </div>
            </template>
        </PopoutMenu>
    </div>
</template>

<style scoped>
.biome-cluster {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.5rem;
}

.biome-toggle-pill {
    display: flex;
    height: 2.5rem;
    flex: 0 0 auto;
    align-items: center;
    gap: 0.5rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-4);
    padding: 0 0.6rem;
    color: var(--color-text-primary);
    font-size: 0.75rem;
    font-weight: 700;
    white-space: nowrap;
}

.biome-picker-trigger {
    min-width: 0;
    flex: 1;
}

.biome-picker-trigger :deep(button) {
    width: 100%;
    min-width: 0;
    height: 2.5rem;
}

.biome-picker-trigger :deep(button > span) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.biome-picker-trigger :deep(button > svg:last-child) {
    margin-left: auto;
    flex: 0 0 auto;
}

.biome-picker-heading > div:first-child {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 0.1rem;
}

.biome-picker-heading small {
    color: var(--color-text-tertiary);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
}

.biome-picker-search {
    width: 100%;
}

.biome-picker-groups {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding-right: 0.2rem;
}

.biome-picker-group {
    min-width: 0;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    padding: 0.25rem;
}

.biome-picker-groups :deep(.biome-picker-group-trigger) {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.5rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    padding: 0.5rem;
    color: var(--color-text-primary);
    cursor: pointer;
    text-align: left;
}

.biome-picker-groups :deep(.biome-picker-group-trigger:hover) {
    background: var(--surface-4);
}

.biome-picker-groups :deep(.biome-picker-group-trigger > div) {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: center;
    gap: 0.5rem;
}

.biome-picker-groups :deep(.biome-picker-group-trigger strong) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.biome-picker-dimension {
    flex: 0 0 auto;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    padding: 0.12rem 0.35rem;
    color: var(--color-text-tertiary);
    font-size: 0.65rem;
    font-weight: 700;
}

.biome-picker-dimension.active {
    border-color: var(--color-brand-highlight);
    background: var(--color-brand-highlight);
    color: var(--color-brand);
}

.biome-picker-groups :deep(.biome-picker-group-trigger small) {
    margin-left: auto;
    color: var(--color-text-tertiary);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
}

.biome-picker-groups :deep(.biome-picker-group-options) {
    padding: 0.25rem 0.35rem 0.45rem;
}

.biome-picker-groups :deep(.checkbox-outer) {
    min-width: 0;
    gap: 0.5rem;
    border-radius: var(--radius-sm);
    padding: 0.25rem;
}

.biome-picker-groups :deep(.checkbox-outer:hover) {
    background: var(--surface-4);
}

.biome-picker-groups :deep(.checkbox-outer > span:last-child) {
    overflow: hidden;
    font-size: 0.75rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.biome-picker-icon {
    width: 1.75rem;
    height: 1.75rem;
    flex: 0 0 auto;
    border: 1px solid color-mix(in srgb, var(--biome-color) 70%, var(--surface-5));
    border-radius: var(--radius-sm);
    background: var(--biome-color);
    image-rendering: pixelated;
    object-fit: cover;
}

.biome-picker-option-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.biome-picker-empty {
    margin: 0;
    padding: 1rem;
    color: var(--color-text-tertiary);
    font-size: 0.8rem;
    text-align: center;
}

/* The menu content itself carries `seed-map-biome-popout`, so one rule covers
 * what the old floating-vue pair split between the popper and its inner panel. */
:global(.seed-map-biome-popout) {
    z-index: 10050 !important;
    max-width: calc(1280px - 0.75rem);
    max-height: calc(800px - 0.75rem);
    overflow: hidden;
}
</style>
