<!-- 由 S4 集成到 LabRecipeGenerator.vue -->
<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import type { SlotDisplay } from '../display.ts'
import type { TextureAtlas } from '../resources.ts'
import type { RecipeSlot, SlotValue } from '../types.ts'
import { useResultCountWheel } from '../useResultCountWheel.ts'
import RecipeItemIcon from './RecipeItemIcon.vue'

const RECIPE_SLOT_MIME_TYPE = 'application/x-axolotl-recipe-slot'

const props = withDefaults(
    defineProps<{
        recipeSlot: RecipeSlot
        value: SlotValue | undefined
        display: SlotDisplay | null
        atlas: TextureAtlas
        count?: number
        countEditable?: boolean
        result?: boolean
    }>(),
    {
        count: 1,
        countEditable: false,
        result: false,
    },
)

const emit = defineEmits<{
    clear: []
    dropValue: [value: SlotValue]
    updateCount: [count: number]
}>()

const { formatMessage } = useVIntl()
const dragDepth = ref(0)

const messages = defineMessages({
    emptySlot: { id: 'app.lab.recipe-generator.slots.empty', defaultMessage: 'Empty slot' },
})

const dragActive = computed(() => dragDepth.value > 0)
const slotLabel = computed(() => `${formatMessage(messages.emptySlot)} ${props.recipeSlot}`)
const { hint: wheelHint, onWheel: onResultWheel } = useResultCountWheel({
    getSlot: () => (props.countEditable ? props.recipeSlot : null),
    getValue: () => props.value,
    getCount: () => props.count ?? 1,
    setCount: (count) => emit('updateCount', count),
})

function hasRecipePayload(event: DragEvent) {
    const types = event.dataTransfer?.types
    if (!types) return false
    const typeList = Array.from(types)
    return (
        !typeList.includes('Files') &&
        (typeList.includes(RECIPE_SLOT_MIME_TYPE) || typeList.includes('text/plain'))
    )
}

function isSlotValue(value: unknown): value is SlotValue {
    if (typeof value !== 'object' || value === null) return false
    const candidate = value as { kind?: unknown; id?: unknown; uid?: unknown }
    switch (candidate.kind) {
        case 'item':
        case 'vanilla_tag':
            return typeof candidate.id === 'string'
        case 'custom_item':
        case 'custom_tag':
            return typeof candidate.uid === 'string'
        default:
            return false
    }
}

function parseSlotValue(raw: string): SlotValue | null {
    if (!raw) return null
    try {
        const parsed: unknown = JSON.parse(raw)
        return isSlotValue(parsed) ? parsed : null
    } catch {
        return null
    }
}

function onSlotDropEvent(event: Event) {
    const detail = (event as CustomEvent<{ value?: unknown }>).detail
    if (!detail || !isSlotValue(detail.value)) return
    emit('dropValue', detail.value)
}

function onDragEnter(event: DragEvent) {
    if (!hasRecipePayload(event)) return
    dragDepth.value += 1
}

function onDragOver(event: DragEvent) {
    const dataTransfer = event.dataTransfer
    if (!dataTransfer) return
    if (!hasRecipePayload(event)) {
        dataTransfer.dropEffect = 'none'
        return
    }
    event.preventDefault()
    dataTransfer.dropEffect = 'copy'
}

function onDragLeave(event: DragEvent) {
    if (!hasRecipePayload(event)) return
    dragDepth.value = Math.max(0, dragDepth.value - 1)
}

function onDrop(event: DragEvent) {
    dragDepth.value = 0
    const dataTransfer = event.dataTransfer
    if (!dataTransfer || !hasRecipePayload(event)) return
    const raw = dataTransfer.getData(RECIPE_SLOT_MIME_TYPE) || dataTransfer.getData('text/plain')
    const value = parseSlotValue(raw)
    if (!value) return
    event.preventDefault()
    emit('dropValue', value)
}
</script>

<template>
    <div
        class="flex min-w-0 flex-col items-center gap-[0.35rem]"
        :class="{ 'is-drag-target': dragActive }"
        :data-recipe-slot="recipeSlot"
        @axolotl-recipe-slot-drop="onSlotDropEvent"
        @dragenter="onDragEnter"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
    >
        <button
            v-tooltip="wheelHint"
            type="button"
            class="recipe-slot-button"
            :class="{ 'recipe-result-button': result }"
            :title="wheelHint ?? slotLabel"
            :aria-label="wheelHint ?? slotLabel"
            @click="emit('clear')"
            @wheel="onResultWheel(recipeSlot, $event)"
        >
            <RecipeItemIcon :display="display" :atlas="atlas" :size="48" />
        </button>
    </div>
</template>

<style scoped>
.recipe-slot-button {
    display: flex;
    width: 3.5rem;
    height: 3.5rem;
    align-items: center;
    justify-content: center;
    border: 2px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-2);
    padding: 0;
    box-shadow:
        inset 1px 1px 0 rgb(0 0 0 / 20%),
        inset -1px -1px 0 rgb(255 255 255 / 10%);
    cursor: pointer;
    transition:
        border-color 0.15s ease,
        background-color 0.15s ease;
}

.recipe-slot-button:hover {
    border-color: var(--color-brand);
    background: var(--color-surface-3);
}

.recipe-slot-button:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 1px;
}

.is-drag-target .recipe-slot-button {
    border-color: var(--color-brand);
    background: var(--color-brand-highlight);
    cursor: copy;
}

.recipe-result-button {
    border-color: color-mix(in srgb, var(--color-brand) 55%, var(--color-surface-5));
}

@media (max-width: 32rem) {
    .recipe-slot-button {
        width: 3.5rem;
        height: 3.5rem;
    }
}
</style>
