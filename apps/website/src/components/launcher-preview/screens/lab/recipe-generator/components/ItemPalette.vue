<script setup lang="ts">
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useVirtualScroll } from '@modrinth/ui/src/composables/virtual-scroll.ts'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import Fuse from '../../../../lib/fuzzy-search.ts'
import type { SlotDisplay } from '../display.ts'
import type { TextureAtlas } from '../resources.ts'
import type { SlotValue } from '../types.ts'
import RecipeItemIcon from './RecipeItemIcon.vue'
import RecipeSlotDragLayer from './RecipeSlotDragLayer.vue'

export type PaletteEntry = {
    key: string
    name: string
    id: string
    display: SlotDisplay
    value: SlotValue
}

const props = defineProps<{
    entries: PaletteEntry[]
    atlas: TextureAtlas
    loading: boolean
}>()

const emit = defineEmits<{
    pick: [value: SlotValue]
}>()

const { formatMessage } = useVIntl()
const search = ref('')
const debouncedSearch = ref('')
let searchTimer: ReturnType<typeof window.setTimeout> | undefined
let lastPickKey = ''
let lastPickAt = 0
const draggingKey = ref('')
let suppressClicksUntil = 0

const RECIPE_SLOT_MIME_TYPE = 'application/x-axolotl-recipe-slot'
const isTauriRuntime = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
const PALETTE_MIN_COLUMN_WIDTH = 72
const PALETTE_ROW_GAP = 6.4
const PALETTE_ROW_HEIGHT = 64 + PALETTE_ROW_GAP

type StartDrag = (
    event: PointerEvent,
    value: SlotValue,
    display: SlotDisplay,
    atlas: TextureAtlas,
    onFinish?: (moved: boolean) => void,
) => void

const messages = defineMessages({
    searchPlaceholder: {
        id: 'app.lab.recipe-generator.items.search-placeholder',
        defaultMessage: 'Search items',
    },
    loading: { id: 'app.lab.recipe-generator.items.loading', defaultMessage: 'Loading items' },
    empty: {
        id: 'app.lab.recipe-generator.items.empty',
        defaultMessage: 'No items match your search.',
    },
    addItem: { id: 'app.lab.recipe-generator.items.add', defaultMessage: 'Add to recipe' },
})

watch(search, (value) => {
    if (searchTimer) window.clearTimeout(searchTimer)
    searchTimer = window.setTimeout(() => {
        debouncedSearch.value = value
    }, 120)
})

const fuse = computed(
    () =>
        new Fuse(props.entries, {
            keys: ['name', 'id'],
            threshold: 0.35,
            ignoreLocation: true,
        }),
)

const visibleEntries = computed(() => {
    const query = debouncedSearch.value.trim()
    if (!query) return props.entries
    return fuse.value.search(query).map((result) => result.item)
})

const gridScroller = ref<HTMLElement | null>(null)
const columns = ref(1)
let gridObserver: ResizeObserver | null = null

function updateColumns() {
    const element = gridScroller.value
    if (!element) return
    const availableWidth = Math.max(0, element.clientWidth - 6)
    columns.value = Math.max(
        1,
        Math.floor(
            (availableWidth + PALETTE_ROW_GAP) / (PALETTE_MIN_COLUMN_WIDTH + PALETTE_ROW_GAP),
        ),
    )
}

watch(
    gridScroller,
    (element) => {
        gridObserver?.disconnect()
        gridObserver = null
        if (!element) return
        updateColumns()
        if (typeof ResizeObserver === 'undefined') return
        gridObserver = new ResizeObserver(updateColumns)
        gridObserver.observe(element)
    },
    { flush: 'post' },
)

onBeforeUnmount(() => {
    gridObserver?.disconnect()
    gridObserver = null
})

const paletteRows = computed<PaletteEntry[][]>(() => {
    const count = Math.max(1, columns.value)
    const rows: PaletteEntry[][] = []
    for (let index = 0; index < visibleEntries.value.length; index += count) {
        rows.push(visibleEntries.value.slice(index, index + count))
    }
    return rows
})

const {
    listContainer,
    totalHeight,
    visibleTop,
    visibleItems: visibleRows,
} = useVirtualScroll(paletteRows, {
    itemHeight: PALETTE_ROW_HEIGHT,
    bufferSize: 4,
})

function rowKey(row: PaletteEntry[]) {
    return row[0]?.key ?? row.length
}

function pick(value: SlotValue) {
    const key = JSON.stringify(value)
    const now = Date.now()
    if (key === lastPickKey && now - lastPickAt < 300) return
    lastPickKey = key
    lastPickAt = now
    emit('pick', value)
}

function pickFromClick(event: MouseEvent, value: SlotValue) {
    if (event.detail > 1 || Date.now() < suppressClicksUntil) return
    pick(value)
}

function onDragStart(event: DragEvent, entry: PaletteEntry) {
    if (isTauriRuntime) {
        event.preventDefault()
        return
    }
    const dataTransfer = event.dataTransfer
    if (!dataTransfer) return
    const payload = JSON.stringify(entry.value)
    draggingKey.value = entry.key
    dataTransfer.effectAllowed = 'copy'
    dataTransfer.setData(RECIPE_SLOT_MIME_TYPE, payload)
    dataTransfer.setData('text/plain', payload)
}

function onDragEnd() {
    draggingKey.value = ''
    suppressClicksUntil = Date.now() + 350
}

function startPointerDrag(event: PointerEvent, entry: PaletteEntry, startDrag: StartDrag) {
    if (!isTauriRuntime || event.button !== 0) return
    draggingKey.value = entry.key
    startDrag(event, entry.value, entry.display, props.atlas, (moved) => {
        draggingKey.value = ''
        if (moved) suppressClicksUntil = Date.now() + 350
    })
}
</script>

<template>
    <RecipeSlotDragLayer v-slot="{ startDrag }">
        <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-2 p-3">
            <StyledInput
                v-model="search"
                :placeholder="formatMessage(messages.searchPlaceholder)"
                clearable
                class="w-full shrink-0"
            />
            <div
                v-if="loading"
                class="flex min-h-24 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.loading) }}
            </div>
            <div
                v-else-if="!visibleEntries.length"
                class="flex min-h-24 items-center justify-center px-4 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.empty) }}
            </div>
            <div v-else ref="gridScroller" class="recipe-palette-grid">
                <div
                    ref="listContainer"
                    class="recipe-palette-virtual"
                    :style="{ height: `${totalHeight}px`, overflowAnchor: 'none' }"
                >
                    <div
                        class="grid-auto-rows-[4rem] absolute inset-x-0 grid gap-[0.4rem] p-[0.1rem_0.25rem_0.25rem_0.1rem]"
                        :style="{
                            top: `${visibleTop}px`,
                            gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                        }"
                    >
                        <template v-for="row in visibleRows" :key="rowKey(row)">
                            <button
                                v-for="entry in row"
                                :key="entry.key"
                                type="button"
                                :draggable="!isTauriRuntime"
                                class="recipe-palette-item"
                                :class="{ 'is-dragging': draggingKey === entry.key }"
                                :style="{ touchAction: isTauriRuntime ? 'none' : undefined }"
                                :title="`${formatMessage(messages.addItem)}: ${entry.name}`"
                                :aria-label="`${formatMessage(messages.addItem)}: ${entry.name}`"
                                @click="pickFromClick($event, entry.value)"
                                @pointerdown="startPointerDrag($event, entry, startDrag)"
                                @dragstart="onDragStart($event, entry)"
                                @dragend="onDragEnd"
                            >
                                <RecipeItemIcon
                                    :display="entry.display"
                                    :atlas="atlas"
                                    :size="34"
                                    :show-count="false"
                                />
                                <span class="recipe-palette-name">{{ entry.name }}</span>
                            </button>
                        </template>
                    </div>
                </div>
            </div>
        </div>
    </RecipeSlotDragLayer>
</template>

<style scoped>
.recipe-palette-grid {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
}

.recipe-palette-virtual {
    position: relative;
    width: 100%;
    min-height: 0;
}

.recipe-palette-item {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 0.2rem;
    overflow: hidden;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-4);
    padding: 0.2rem 0.1rem;
    color: var(--color-text-primary);
    cursor: grab;
    transition:
        background-color 0.15s ease,
        border-color 0.15s ease,
        transform 0.1s ease;
}

.recipe-palette-item:hover,
.recipe-palette-item:focus-visible {
    border-color: var(--color-brand);
    background: var(--color-surface-3);
    outline: none;
}

.recipe-palette-item:active {
    cursor: grabbing;
    transform: translateY(1px);
}

.recipe-palette-item.is-dragging {
    opacity: 0.6;
    border-color: var(--color-brand);
}

.recipe-palette-name {
    width: 100%;
    overflow: hidden;
    color: var(--color-text-tertiary);
    font-size: 0.55rem;
    line-height: 1.15;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
}
</style>
