<script setup lang="ts">
import CheckCircleIcon from '@modrinth/assets/icons/check-circle.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import XCircleIcon from '@modrinth/assets/icons/x-circle.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { loadMinecraftItemImage } from '../recipe-generator/item-images.ts'
import {
    getSeedMapContainerLoot,
    getSeedMapStructureDetails,
    type SeedMapContainer,
    type SeedMapLoot,
    type SeedMapStructureDetails,
    type SeedMapStructurePiece,
    type SeedMapStructureQuery,
} from './advanced.ts'
import type { SeedMapFeatureKind } from './features.ts'
import { mergeSeedMapLootItems as mergeLootItems } from './loot-items.ts'
import { SeedMapPreviewLimit, type SeedMapPreviewLimitReason } from './preview-data.ts'
import SeedMapLayoutPreview from './SeedMapLayoutPreview.vue'
import SeedMapPreviewNotice from './SeedMapPreviewNotice.vue'
import { seedMapTemplatePieces } from './structure-blocks.ts'
import { createSeedMapLabels, seedMapEffectDuration } from './structure-labels.ts'
import {
    seedMapKnownLayoutChests,
    seedMapLayoutPieces,
    seedMapStructureCapabilities,
    seedMapVolumePieces,
} from './structure-layout.ts'

const props = defineProps<{
    query: SeedMapStructureQuery
    kind: SeedMapFeatureKind
    title: string
    selected?: number
    load?: (query: SeedMapStructureQuery) => Promise<SeedMapStructureDetails>
}>()
const emit = defineEmits<{ close: []; focus: [piece: SeedMapStructurePiece] }>()
const { formatMessage, locale } = useVIntl()
const messages = defineMessages({
    supportedCount: {
        id: 'app.lab.seed-map.details.supported-count',
        defaultMessage: '{count, plural, one {# feature supported} other {# features supported}}',
    },
    unsupportedCount: {
        id: 'app.lab.seed-map.details.unsupported-count',
        defaultMessage:
            '{count, plural, one {# feature not supported} other {# features not supported}}',
    },
    supportedFeatures: {
        id: 'app.lab.seed-map.details.supported-features',
        defaultMessage: 'Supported',
    },
    unsupportedFeatures: {
        id: 'app.lab.seed-map.details.unsupported-features',
        defaultMessage: 'Not supported',
    },
    capabilityVariant: {
        id: 'app.lab.seed-map.details.capability.variant',
        defaultMessage: 'Structure variants',
    },
    capabilityProjection: {
        id: 'app.lab.seed-map.details.capability.projection',
        defaultMessage: '2D map projection',
    },
    capabilityVolume: {
        id: 'app.lab.seed-map.details.capability.volume',
        defaultMessage: '3D piece bounds',
    },
    capabilityContainers: {
        id: 'app.lab.seed-map.details.capability.containers',
        defaultMessage: 'Container list',
    },
    capabilityComplete: {
        id: 'app.lab.seed-map.details.capability.complete',
        defaultMessage: 'Complete piece layout',
    },
    capabilityAbsoluteHeight: {
        id: 'app.lab.seed-map.details.capability.absolute-height',
        defaultMessage: 'Verified world heights',
    },
    capabilityBlocks: {
        id: 'app.lab.seed-map.details.capability.blocks',
        defaultMessage: 'Individual block models',
    },
    none: {
        id: 'app.lab.seed-map.details.capability.none',
        defaultMessage: 'No additional predictions',
    },
    projectionHint: {
        id: 'app.lab.seed-map.details.projection-hint',
        defaultMessage:
            'Zoom to 2 blocks per pixel or closer to see 2D layouts on the map. Select overlapping pieces again to inspect the next piece.',
    },
    relativeHeight: {
        id: 'app.lab.seed-map.details.relative-height',
        defaultMessage:
            'Vertical relationships are relative; the final world height is not predicted.',
    },

    overview: { id: 'app.lab.seed-map.details.overview', defaultMessage: 'Overview' },
    layout: { id: 'app.lab.seed-map.details.layout', defaultMessage: 'Layout' },
    loot: { id: 'app.lab.seed-map.details.loot', defaultMessage: 'Loot' },
    close: { id: 'app.lab.seed-map.details.close', defaultMessage: 'Close structure details' },
    retry: { id: 'app.lab.seed-map.details.retry', defaultMessage: 'Retry' },
    loading: { id: 'app.lab.seed-map.details.loading', defaultMessage: 'Calculating structure…' },
    failed: {
        id: 'app.lab.seed-map.details.failed',
        defaultMessage: 'The calculation failed. Your selection is preserved.',
    },
    unsupported: {
        id: 'app.lab.seed-map.details.unsupported',
        defaultMessage: 'The engine has no additional details for this structure in this version.',
    },
    rotation: { id: 'app.lab.seed-map.details.rotation', defaultMessage: 'Rotation' },
    mirror: { id: 'app.lab.seed-map.details.mirror', defaultMessage: 'Mirror' },
    variant: { id: 'app.lab.seed-map.details.variant', defaultMessage: 'Variant' },
    abandoned: { id: 'app.lab.seed-map.details.abandoned', defaultMessage: 'Abandoned village' },
    giant: { id: 'app.lab.seed-map.details.giant', defaultMessage: 'Giant portal' },
    underground: { id: 'app.lab.seed-map.details.underground', defaultMessage: 'Underground' },
    airPocket: { id: 'app.lab.seed-map.details.air-pocket', defaultMessage: 'Air pocket' },
    basement: { id: 'app.lab.seed-map.details.basement', defaultMessage: 'Basement' },
    cracked: { id: 'app.lab.seed-map.details.cracked', defaultMessage: 'Cracked geode' },
    ship: { id: 'app.lab.seed-map.details.ship', defaultMessage: 'End ship' },
    eyes: { id: 'app.lab.seed-map.details.eyes', defaultMessage: '{count} portal eyes' },
    pieces: { id: 'app.lab.seed-map.details.pieces', defaultMessage: '{count} structure pieces' },
    containers: { id: 'app.lab.seed-map.details.containers', defaultMessage: '{count} containers' },
    container: {
        id: 'app.lab.seed-map.details.named-container',
        defaultMessage: '{name} {number} · {x}, {z}',
    },
    allContainers: {
        id: 'app.lab.seed-map.details.all-containers',
        defaultMessage: 'All loot',
    },
    lootSource: {
        id: 'app.lab.seed-map.details.loot-source',
        defaultMessage: 'From {container}',
    },
    technical: {
        id: 'app.lab.seed-map.details.technical-identifiers',
        defaultMessage: 'Technical identifiers',
    },
    effectDuration: {
        id: 'app.lab.seed-map.details.effect-duration',
        defaultMessage: '{name} · {duration}',
    },
    enchanted: {
        id: 'app.lab.seed-map.details.enchantment-level',
        defaultMessage: '{name} {level}',
    },
    noLayout: {
        id: 'app.lab.seed-map.details.no-layout',
        defaultMessage: 'The engine does not provide spatial layout bounds for this structure.',
    },
    noLoot: {
        id: 'app.lab.seed-map.details.no-loot',
        defaultMessage:
            'No predictable containers are available for this structure in this version.',
    },
    predictedLoot: {
        id: 'app.lab.seed-map.details.predicted-loot',
        defaultMessage: 'Predicted item list; chest slot positions are not reconstructed.',
    },
    estimatedLoot: {
        id: 'app.lab.seed-map.details.estimated-loot',
        defaultMessage: 'The loot seed is estimated; the item list may differ from the game.',
    },
    unresolved: {
        id: 'app.lab.seed-map.details.unresolved',
        defaultMessage: 'Some loot subtables are missing; this list is incomplete.',
    },
    copy: { id: 'app.lab.seed-map.details.copy', defaultMessage: 'Copy predicted loot' },
    copied: { id: 'app.lab.seed-map.details.copied', defaultMessage: 'Copied' },
    height: {
        id: 'app.lab.seed-map.details.height',
        defaultMessage:
            'Some Y coordinates are placeholders. Use the horizontal layout to locate pieces.',
    },
    partial: {
        id: 'app.lab.seed-map.details.partial',
        defaultMessage: 'The engine simulates only part of this structure.',
    },
    terrain: {
        id: 'app.lab.seed-map.details.terrain',
        defaultMessage: 'Terrain and later world changes can affect this prediction.',
    },
    truncated: {
        id: 'app.lab.seed-map.details.truncated',
        defaultMessage: 'The structure reached the piece limit; additional pieces may be missing.',
    },
    estimatedContainers: {
        id: 'app.lab.seed-map.details.estimated-containers',
        defaultMessage:
            'Container coordinates are rough chunk estimates; exact chest positions are unavailable.',
    },
    positionsUnavailable: {
        id: 'app.lab.seed-map.details.positions-unavailable',
        defaultMessage:
            'The engine provides container locations but no piece positions for this structure.',
    },
})

const tab = ref<'overview' | 'layout' | 'loot'>('overview')
const details = ref<SeedMapStructureDetails | null>(null)
const sparseBlocks = ref<SeedMapStructureDetails['blocks']>()
const loot = ref<SeedMapLoot | null>(null)
type LootItem = NonNullable<SeedMapLoot['items']>[number]
type DisplayLootItem = LootItem & { containerId?: number }
const allContainersValue = 'all'
const displayedLootItems = ref<DisplayLootItem[]>([])
const hoverLoot = ref<{ containerId: number; result: SeedMapLoot }>()
const busy = ref(false)
const lootBusy = ref(false)
const failed = ref(false)
const lootFailed = ref(false)
/** Set when the preview's data does not include this structure. */
const previewLimit = ref<SeedMapPreviewLimitReason | null>(null)
const lootPreviewLimit = ref<SeedMapPreviewLimitReason | null>(null)
const containerId = ref<number | typeof allContainersValue>()
const selectedPiece = ref<number>()
const copied = ref(false)
const technicalOpen = ref(false)
const capabilitiesOpen = ref(false)
const labels = computed(() =>
    createSeedMapLabels((message, values) => formatMessage(message, values), locale.value),
)
const variantLabel = computed(() =>
    details.value?.variant
        ? labels.value.variant(props.kind, details.value.variant, details.value.pieces)
        : undefined,
)
const technicalIdentifiers = computed(() =>
    JSON.stringify(
        {
            kind: props.kind,
            variant: details.value?.variant,
            pieces: details.value?.pieces.map((piece) => ({
                id: piece.id,
                engineName: piece.name,
                identifier: labels.value.pieceId(piece.name, details.value?.variant),
            })),
            containers: details.value?.containers,
            loot: loot.value,
        },
        null,
        2,
    ),
)
const itemImages = ref<Record<string, string>>({})
const lootCache = new Map<number, SeedMapLoot>()
let generation = 0
let lootGeneration = 0
let hoverLootGeneration = 0

async function loadDetails() {
    const token = ++generation
    const query = { ...props.query }
    busy.value = true
    failed.value = false
    previewLimit.value = null
    try {
        const result = await (props.load ?? getSeedMapStructureDetails)(query)
        if (token !== generation) return
        details.value = result
        sparseBlocks.value = result.blocks
        selectedPiece.value = props.selected
        containerId.value = result.containers.length ? allContainersValue : undefined
        if (!result.supported) tab.value = 'overview'
        if (tab.value === 'loot') void loadLoot()
    } catch (error) {
        if (token !== generation) return
        failed.value = true
        previewLimit.value = error instanceof SeedMapPreviewLimit ? error.reason : null
    } finally {
        if (token === generation) busy.value = false
    }
}
async function loadSparseBlocks() {
    if (sparseBlocks.value || generation === 0) return
    const token = generation
    try {
        const result = await (props.load ?? getSeedMapStructureDetails)({
            ...props.query,
            includeBlocks: true,
        })
        if (token === generation && result.supported) sparseBlocks.value = result.blocks ?? []
    } catch {
        if (token === generation) sparseBlocks.value = undefined
    }
}

async function loadLoot() {
    if (!details.value?.supported || containerId.value === undefined || tab.value !== 'loot') return
    const token = ++lootGeneration
    const selection = containerId.value
    const query = { ...props.query }
    lootBusy.value = true
    lootFailed.value = false
    lootPreviewLimit.value = null
    copied.value = false
    try {
        const result =
            selection === allContainersValue
                ? await fetchAllContainerLoot(query)
                : { loot: await fetchContainerLoot(selection, query), items: undefined }
        if (token !== lootGeneration) return
        loot.value = result.loot
        displayedLootItems.value = result.items ?? result.loot.items ?? []
    } catch (error) {
        if (token !== lootGeneration) return
        lootFailed.value = true
        lootPreviewLimit.value = error instanceof SeedMapPreviewLimit ? error.reason : null
    } finally {
        if (token === lootGeneration) lootBusy.value = false
    }
}

async function fetchContainerLoot(container: number, query = props.query) {
    const cached = lootCache.get(container)
    if (cached) return cached
    const result = await getSeedMapContainerLoot({ ...query }, container)
    const mergedResult = result.items ? { ...result, items: mergeLootItems(result.items) } : result
    if (lootCache.size >= 32) lootCache.delete(lootCache.keys().next().value!)
    lootCache.set(container, mergedResult)
    for (const item of mergedResult.items ?? []) {
        if (itemImages.value[item.id]) continue
        void Promise.resolve()
            .then(() => loadMinecraftItemImage(item.id))
            .then((image) => {
                if (image) itemImages.value[item.id] = image.src
            })
            .catch(() => {})
    }
    return mergedResult
}

async function fetchAllContainerLoot(query = props.query) {
    const containers = details.value?.containers ?? []
    const entries = await Promise.all(
        containers.map(async (container) => ({
            container,
            loot: await fetchContainerLoot(container.id, query),
        })),
    )
    const lootResults = entries.map(({ loot }) => loot)
    const items = entries.flatMap(({ container, loot }) =>
        (loot.items ?? []).map((item) => ({ ...item, containerId: container.id })),
    )
    return {
        loot: {
            supported: lootResults.some((result) => result.supported),
            truncated: lootResults.some((result) => result.truncated),
            estimated: lootResults.some((result) => result.estimated),
            items: lootResults.flatMap((result) => result.items ?? []),
            unresolvedTables: [
                ...new Set(lootResults.flatMap((result) => result.unresolvedTables ?? [])),
            ],
            limits: [...new Set(lootResults.flatMap((result) => result.limits))],
        },
        items,
    } satisfies { loot: SeedMapLoot; items: DisplayLootItem[] }
}

async function previewContainerLoot(containerId: number | undefined) {
    const token = ++hoverLootGeneration
    hoverLoot.value = undefined
    if (containerId === undefined || !details.value?.supported) return
    try {
        const result = await fetchContainerLoot(containerId)
        if (token === hoverLootGeneration && result.supported)
            hoverLoot.value = { containerId, result }
    } catch {
        if (token === hoverLootGeneration) hoverLoot.value = undefined
    }
}

async function openContainerLoot(container: number) {
    if (!details.value?.supported) return
    try {
        const result = await fetchContainerLoot(container)
        if (!result.supported) return
        containerId.value = container
        tab.value = 'loot'
        loot.value = result
        displayedLootItems.value = result.items ?? []
    } catch {
        containerId.value = container
        tab.value = 'loot'
    }
}

watch(
    () => JSON.stringify(props.query),
    () => {
        generation++
        lootGeneration++
        hoverLootGeneration++
        details.value = null
        sparseBlocks.value = undefined
        loot.value = null
        displayedLootItems.value = []
        hoverLoot.value = undefined
        lootBusy.value = false
        lootFailed.value = false
        containerId.value = undefined
        selectedPiece.value = undefined
        technicalOpen.value = false
        capabilitiesOpen.value = false
        lootCache.clear()
        void loadDetails()
    },
    { immediate: true },
)
watch([tab, containerId], () => {
    loot.value = null
    displayedLootItems.value = []
    void loadLoot()
})

watch(
    () => props.selected,
    (id) => {
        selectedPiece.value = id
    },
    { immediate: true },
)
const positionedPieces = computed(() => seedMapLayoutPieces(details.value))
const volumePieces = computed(() => seedMapVolumePieces(details.value))
const knownChests = computed(() => seedMapKnownLayoutChests(details.value, props.kind))
const templatePieces = computed(() => seedMapTemplatePieces(details.value, props.query, props.kind))
const previewPieces = computed(() =>
    templatePieces.value.length ? templatePieces.value : volumePieces.value,
)
const layoutList = computed(() =>
    templatePieces.value.length ? templatePieces.value : (details.value?.pieces ?? []),
)
const capabilities = computed(() => {
    if (!details.value) return []
    return Object.entries(
        seedMapStructureCapabilities(details.value, props.query.version, props.kind, props.query),
    ).map(([key, supported]) => ({
        key,
        supported,
        message:
            messages[`capability${key[0].toUpperCase()}${key.slice(1)}` as keyof typeof messages],
    }))
})
const flags = computed(() => {
    const variant = details.value?.variant
    if (!variant) return []
    const values = {
        abandoned: props.kind === 'village' && variant.abandoned,
        giant: props.kind === 'ruined-portal' && variant.giant,
        underground: props.kind === 'ruined-portal' && variant.underground,
        airPocket: props.kind === 'ruined-portal' && variant.airPocket,
        basement: props.kind === 'igloo' && variant.basement,
        cracked: props.kind === 'geode' && variant.cracked,
    }
    return (Object.keys(values) as (keyof typeof values)[]).filter((key) => values[key])
})
const portalEyes = computed(
    () => details.value?.pieces.find((piece) => piece.portalEyes !== undefined)?.portalEyes,
)
const hasShip = computed(() => details.value?.pieces.some((piece) => piece.name.includes('ship')))
function pieceLabel(piece: SeedMapStructurePiece) {
    return labels.value.piece(props.kind, piece.name, details.value?.variant)
}
function effectLabel(effect: string, duration?: number) {
    const name = labels.value.effect(effect)
    const instant = ['instant_health', 'instant_damage', 'saturation'].includes(
        effect.replace(/^minecraft:/, ''),
    )
    const time = duration === undefined || instant ? undefined : seedMapEffectDuration(duration)
    return time ? formatMessage(messages.effectDuration, { name, duration: time }) : name
}
function containerLabel(container: SeedMapContainer) {
    const containers = details.value?.containers ?? []
    const index = containers.findIndex((value) => value.id === container.id)
    return formatMessage(messages.container, {
        number: index + 1,
        name: labels.value.table(container.table ?? 'chests/ancient_city'),
        x: container.x,
        z: container.z,
    })
}
function lootSourceLabel(id: number) {
    const container = details.value?.containers.find((value) => value.id === id)
    return container ? containerLabel(container) : String(id)
}
function focus(piece: SeedMapStructurePiece) {
    if (!piece.positionKnown) return
    selectedPiece.value = piece.id
    emit('focus', piece)
}
onBeforeUnmount(() => {
    generation++
    lootGeneration++
})
async function copyLoot() {
    try {
        await navigator.clipboard.writeText(JSON.stringify(loot.value, null, 2))
        copied.value = true
    } catch {
        lootFailed.value = true
    }
}
</script>

<template>
    <aside
        class="flex size-full min-h-0 min-w-0 flex-col overflow-hidden bg-surface-2"
        :aria-label="title"
        @pointerdown.stop
        @wheel.stop
        @dblclick.stop
    >
        <div
            class="flex shrink-0 items-center justify-between gap-3 border-b border-surface-4 px-4 py-3"
        >
            <div class="min-w-0">
                <strong class="block truncate">{{ title }}</strong
                ><span class="text-xs text-[var(--color-text-tertiary)]"
                    >{{ query.version }} · {{ query.x }}, {{ query.z }}</span
                >
            </div>
            <Button
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.close)"
                @click="emit('close')"
                ><XIcon
            /></Button>
        </div>
        <div
            v-if="details?.supported"
            class="flex shrink-0 gap-1 border-b border-surface-4 p-2"
            role="tablist"
        >
            <Button
                v-for="value in ['overview', 'layout', 'loot'] as const"
                :key="value"
                :type="tab === value ? 'chip-text' : 'quiet'"
                size="2xs"
                role="tab"
                :aria-selected="tab === value"
                @click="tab = value"
                >{{ formatMessage(messages[value]) }}</Button
            >
        </div>
        <div
            class="min-h-0 flex-1 overflow-y-auto p-4"
            :aria-busy="busy || lootBusy"
            data-onboarding-id="seed-map-structure-details"
        >
            <p v-if="busy" class="m-0 text-sm">{{ formatMessage(messages.loading) }}</p>
            <SeedMapPreviewNotice v-if="failed && previewLimit" :reason="previewLimit" />
            <div v-else-if="failed" class="flex flex-col gap-2 text-sm">
                <p class="m-0">{{ formatMessage(messages.failed) }}</p>
                <Button type="outlined" size="2xs" @click="loadDetails"
                    ><RefreshCwIcon />{{ formatMessage(messages.retry) }}</Button
                >
            </div>
            <div class="mb-3 flex flex-col gap-2"><slot name="actions" /></div>
            <div
                v-if="details && !details.supported"
                class="rounded-lg border border-surface-4 bg-surface-3 p-3 text-sm text-[var(--color-text-tertiary)]"
                data-testid="seed-map-details-unavailable"
            >
                <p class="m-0">{{ formatMessage(messages.unsupported) }}</p>
            </div>
            <template v-else-if="details?.supported">
                <div
                    class="mb-4 overflow-hidden rounded-lg border border-surface-4 bg-surface-3 text-xs"
                    data-testid="seed-map-structure-capabilities"
                >
                    <button
                        type="button"
                        class="grid w-full grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-center gap-3 bg-transparent p-3 text-left text-inherit hover:bg-surface-4"
                        :aria-expanded="capabilitiesOpen"
                        @click="capabilitiesOpen = !capabilitiesOpen"
                    >
                        <span
                            v-for="supported in [true, false]"
                            :key="String(supported)"
                            class="flex min-w-0 items-center gap-1.5 font-semibold"
                        >
                            <component
                                :is="supported ? CheckCircleIcon : XCircleIcon"
                                class="size-3.5 shrink-0"
                                aria-hidden="true"
                            />
                            <span>{{
                                formatMessage(
                                    capabilitiesOpen
                                        ? supported
                                            ? messages.supportedFeatures
                                            : messages.unsupportedFeatures
                                        : supported
                                          ? messages.supportedCount
                                          : messages.unsupportedCount,
                                    {
                                        count: capabilities.filter(
                                            (entry) => entry.supported === supported,
                                        ).length,
                                    },
                                )
                            }}</span>
                        </span>
                        <ChevronDownIcon
                            class="size-4 shrink-0"
                            :class="{ 'rotate-180': capabilitiesOpen }"
                            aria-hidden="true"
                        />
                    </button>
                    <div
                        v-if="capabilitiesOpen"
                        class="grid grid-cols-2 gap-3 border-t border-surface-4 p-3"
                        data-testid="seed-map-capability-list"
                    >
                        <div
                            v-for="supported in [true, false]"
                            :key="String(supported)"
                            class="flex flex-col gap-1"
                        >
                            <span
                                v-for="capability in capabilities.filter(
                                    (entry) => entry.supported === supported,
                                )"
                                :key="capability.key"
                                >{{ formatMessage(capability.message) }}</span
                            >
                            <span
                                v-if="supported && !capabilities.some((entry) => entry.supported)"
                                >{{ formatMessage(messages.none) }}</span
                            >
                        </div>
                    </div>
                </div>
                <div v-if="tab === 'overview'" class="flex flex-col gap-3 text-sm">
                    <dl v-if="details.variant" class="m-0 grid grid-cols-2 gap-2">
                        <template v-if="labels.hasRotation(kind)">
                            <dt>{{ formatMessage(messages.rotation) }}</dt>
                            <dd class="m-0">{{ labels.rotation(details.variant.rotation) }}</dd>
                        </template>
                        <template v-if="labels.hasMirror(kind)">
                            <dt>{{ formatMessage(messages.mirror) }}</dt>
                            <dd class="m-0">{{ labels.mirror(details.variant.mirror) }}</dd>
                        </template>
                        <template v-if="variantLabel">
                            <dt>{{ formatMessage(messages.variant) }}</dt>
                            <dd class="m-0 min-w-0">
                                {{ variantLabel.name }}
                                <code
                                    v-if="variantLabel.id"
                                    class="block break-all text-xs text-[var(--color-text-tertiary)]"
                                    >{{ variantLabel.id }}</code
                                >
                            </dd>
                        </template>
                    </dl>
                    <span v-for="flag in flags" :key="flag">{{
                        formatMessage(messages[flag])
                    }}</span>
                    <span v-if="kind === 'end-city' && hasShip">{{
                        formatMessage(messages.ship)
                    }}</span>
                    <span v-if="portalEyes !== undefined">{{
                        formatMessage(messages.eyes, { count: portalEyes })
                    }}</span>
                    <span>{{
                        formatMessage(messages.pieces, { count: details.pieces.length })
                    }}</span>
                    <span>{{
                        formatMessage(messages.containers, { count: details.containers.length })
                    }}</span>
                </div>
                <div v-else-if="tab === 'layout'" class="flex flex-col gap-2">
                    <p
                        v-if="!positionedPieces.length && !templatePieces.length"
                        class="m-0 text-sm"
                    >
                        {{ formatMessage(messages.noLayout) }}
                    </p>
                    <p
                        v-if="details.limits.includes('relative_height')"
                        class="m-0 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.relativeHeight) }}
                    </p>
                    <p
                        v-else-if="details.limits.includes('height_placeholder')"
                        class="m-0 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.height) }}
                    </p>
                    <SeedMapLayoutPreview
                        v-if="previewPieces.length"
                        :pieces="previewPieces"
                        :selected="selectedPiece"
                        :label="pieceLabel"
                        :version="query.version"
                        :kind="kind"
                        :sparse-blocks="sparseBlocks"
                        :chests="knownChests"
                        :containers="details.containers"
                        :hover-loot="hoverLoot"
                        :item-images="itemImages"
                        :item-label="labels.item"
                        :unknown-chest-heights="
                            templatePieces.length
                                ? 0
                                : Math.max(details.containers.length - knownChests.length, 0)
                        "
                        @select="focus"
                        @chest-hover="previewContainerLoot"
                        @chest-click="openContainerLoot"
                        @request-blocks="loadSparseBlocks"
                    />
                    <button
                        v-for="piece in layoutList"
                        :key="piece.id"
                        :data-piece-id="piece.id"
                        :aria-pressed="selectedPiece === piece.id"
                        :class="selectedPiece === piece.id ? 'ring-2 ring-brand' : ''"
                        class="flex flex-col gap-1 rounded-lg border border-surface-4 bg-surface-3 p-2 text-left text-xs hover:bg-surface-4"
                        :disabled="!piece.positionKnown"
                        @click="focus(piece)"
                    >
                        <strong class="break-words">{{ pieceLabel(piece) }}</strong
                        ><span v-if="piece.positionKnown"
                            >{{ piece.position.x }}, {{ piece.position.y }},
                            {{ piece.position.z }} · {{ labels.rotation(piece.rotation) }}</span
                        ><span v-if="piece.portalEyes !== undefined">{{
                            formatMessage(messages.eyes, { count: piece.portalEyes })
                        }}</span>
                    </button>
                </div>
                <div
                    v-else
                    class="flex flex-col gap-3 text-sm"
                    data-onboarding-id="seed-map-container-loot"
                >
                    <p v-if="!details.containers.length" class="m-0">
                        {{ formatMessage(messages.noLoot) }}
                    </p>
                    <Combobox
                        v-else
                        :model-value="
                            containerId === allContainersValue
                                ? allContainersValue
                                : String(containerId)
                        "
                        :options="[
                            {
                                value: allContainersValue,
                                label: formatMessage(messages.allContainers),
                            },
                            ...details.containers.map((container) => ({
                                value: String(container.id),
                                label: containerLabel(container),
                            })),
                        ]"
                        @update:model-value="
                            containerId =
                                $event === allContainersValue ? allContainersValue : Number($event)
                        "
                    />
                    <p v-if="lootBusy" class="m-0">{{ formatMessage(messages.loading) }}</p>
                    <SeedMapPreviewNotice
                        v-if="lootFailed && lootPreviewLimit"
                        :reason="lootPreviewLimit"
                    />
                    <div v-else-if="lootFailed" class="flex flex-col gap-2">
                        <p class="m-0">{{ formatMessage(messages.failed) }}</p>
                        <Button type="outlined" size="2xs" @click="loadLoot">{{
                            formatMessage(messages.retry)
                        }}</Button>
                    </div>
                    <template v-if="loot">
                        <p
                            v-if="loot.estimated"
                            class="m-0 text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.estimatedLoot) }}
                        </p>
                        <p v-if="!loot.supported" class="m-0">
                            {{ formatMessage(messages.noLoot) }}
                        </p>
                        <p v-if="loot.unresolvedTables?.length" class="m-0">
                            {{ formatMessage(messages.unresolved)
                            }}<span
                                v-for="table in loot.unresolvedTables"
                                :key="table"
                                class="block text-xs"
                                >{{ labels.table(table) }}</span
                            >
                        </p>
                        <div
                            v-for="(item, index) in displayedLootItems"
                            :key="`${item.id}-${index}`"
                            class="flex gap-2 border-b border-surface-4 py-2"
                        >
                            <img
                                v-if="itemImages[item.id]"
                                :src="itemImages[item.id]"
                                alt=""
                                class="size-8 shrink-0 [image-rendering:pixelated]"
                            />
                            <div class="min-w-0">
                                <strong class="break-words"
                                    >{{ labels.item(item.id) }} ×{{ item.count }}</strong
                                ><span
                                    v-for="enchantment in item.enchantments"
                                    :key="enchantment.id"
                                    class="block text-xs text-[var(--color-text-tertiary)]"
                                    >{{
                                        formatMessage(messages.enchanted, {
                                            name: labels.enchantment(enchantment.id),
                                            level: labels.level(enchantment.level),
                                        })
                                    }}</span
                                ><span v-if="item.effect" class="block text-xs">{{
                                    effectLabel(item.effect, item.duration)
                                }}</span
                                ><span
                                    v-if="item.containerId !== undefined"
                                    class="block text-xs text-[var(--color-text-tertiary)]"
                                    >{{
                                        formatMessage(messages.lootSource, {
                                            container: lootSourceLabel(item.containerId),
                                        })
                                    }}</span
                                >
                            </div>
                        </div>
                        <Button v-if="loot.supported" type="outlined" size="2xs" @click="copyLoot"
                            ><ClipboardCopyIcon />{{
                                formatMessage(copied ? messages.copied : messages.copy)
                            }}</Button
                        >
                    </template>
                </div>
                <div
                    class="mt-4 flex flex-col gap-2 border-t border-surface-4 pt-3 text-xs text-[var(--color-text-tertiary)]"
                >
                    <p v-if="details.limits.includes('partial_layout')" class="m-0">
                        {{ formatMessage(messages.partial) }}
                    </p>
                    <p v-if="details.limits.includes('terrain_unverified')" class="m-0">
                        {{ formatMessage(messages.terrain) }}
                    </p>
                    <p v-if="details.limits.includes('piece_positions_unavailable')" class="m-0">
                        {{ formatMessage(messages.positionsUnavailable) }}
                    </p>
                    <p
                        v-if="details.limits.includes('container_positions_approximate')"
                        class="m-0"
                    >
                        {{ formatMessage(messages.estimatedContainers) }}
                    </p>
                    <p v-if="details.truncated" class="m-0">
                        {{ formatMessage(messages.truncated) }}
                    </p>
                </div>
                <details
                    :open="technicalOpen"
                    class="mt-3 text-xs text-[var(--color-text-tertiary)]"
                    @toggle="technicalOpen = ($event.target as HTMLDetailsElement).open"
                >
                    <summary class="cursor-pointer">
                        {{ formatMessage(messages.technical) }}
                    </summary>
                    <pre v-if="technicalOpen" class="m-0 mt-2 whitespace-pre-wrap break-all">{{
                        technicalIdentifiers
                    }}</pre>
                </details>
            </template>
        </div>
    </aside>
</template>
