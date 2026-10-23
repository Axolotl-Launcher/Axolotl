<!-- 由 S4 集成到 LabRecipeGenerator.vue -->
<script setup lang="ts">
import { computed } from 'vue'

import { getSlotDisplay } from '../display.ts'
import type { TextureAtlas } from '../resources.ts'
import type { RecipeSlot, RecipeSlotContext, SlotValue } from '../types.ts'
import RecipeSlotCell from './RecipeSlotCell.vue'

const props = withDefaults(
    defineProps<{
        slots: readonly RecipeSlot[]
        values: Partial<Record<RecipeSlot, SlotValue>>
        ctx: RecipeSlotContext | null
        atlas: TextureAtlas
        variant?: 'crafting' | 'row'
        twoByTwo?: boolean
    }>(),
    {
        variant: 'row',
        twoByTwo: false,
    },
)

const emit = defineEmits<{
    updateSlot: [slot: RecipeSlot, value: SlotValue | undefined]
    updateCount: [slot: RecipeSlot, count: number]
}>()

const TWO_BY_TWO_DISABLED_SLOTS = new Set<RecipeSlot>([
    'crafting.3',
    'crafting.6',
    'crafting.7',
    'crafting.8',
    'crafting.9',
])

const gridSlots = computed(() => {
    const slots =
        props.variant === 'crafting'
            ? props.slots.filter((slot) => slot !== 'crafting.result')
            : props.slots
    if (props.variant === 'crafting' && props.twoByTwo) {
        return slots.filter((slot) => !TWO_BY_TWO_DISABLED_SLOTS.has(slot))
    }
    return slots
})

function slotDisplay(slot: RecipeSlot) {
    return props.ctx ? getSlotDisplay(props.values[slot], props.ctx) : null
}

function countFor(slot: RecipeSlot) {
    const value = props.values[slot]
    return value && (value.kind === 'item' || value.kind === 'custom_item') && value.count
        ? value.count
        : 1
}

function canEditCount(slot: RecipeSlot) {
    return slot === 'crafting.result' || slot === 'stonecutter.result' || slot === 'brewing.result'
}

function updateSlot(slot: RecipeSlot, value: SlotValue | undefined) {
    emit('updateSlot', slot, value)
}

function updateCount(slot: RecipeSlot, count: number) {
    emit('updateCount', slot, count)
}
</script>

<template>
    <div class="recipe-slot-grid" :class="`is-${variant}`">
        <div v-if="variant === 'crafting'" class="recipe-crafting-editor">
            <div
                class="recipe-crafting-grid grid-auto-rows-[3.75rem] grid grid-cols-[repeat(3,3.75rem)] gap-[0.45rem] rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 p-[0.6rem]"
                :class="{ 'is-two-by-two': twoByTwo }"
            >
                <RecipeSlotCell
                    v-for="slot in gridSlots"
                    :key="slot"
                    :recipe-slot="slot"
                    :value="values[slot]"
                    :display="slotDisplay(slot)"
                    :atlas="atlas"
                    :count="countFor(slot)"
                    :count-editable="canEditCount(slot)"
                    @clear="updateSlot(slot, undefined)"
                    @drop-value="updateSlot(slot, $event)"
                    @update-count="updateCount(slot, $event)"
                />
            </div>
            <div v-if="slots.includes('crafting.result')" class="recipe-result-column">
                <RecipeSlotCell
                    :recipe-slot="'crafting.result'"
                    :value="values['crafting.result']"
                    :display="slotDisplay('crafting.result')"
                    :atlas="atlas"
                    :count="countFor('crafting.result')"
                    count-editable
                    result
                    @clear="updateSlot('crafting.result', undefined)"
                    @drop-value="updateSlot('crafting.result', $event)"
                    @update-count="updateCount('crafting.result', $event)"
                />
            </div>
        </div>
        <div v-else class="recipe-slot-row">
            <RecipeSlotCell
                v-for="slot in slots"
                :key="slot"
                :recipe-slot="slot"
                :value="values[slot]"
                :display="slotDisplay(slot)"
                :atlas="atlas"
                :count="countFor(slot)"
                :count-editable="canEditCount(slot)"
                :result="slot === 'crafting.result' || slot === 'stonecutter.result'"
                @clear="updateSlot(slot, undefined)"
                @drop-value="updateSlot(slot, $event)"
                @update-count="updateCount(slot, $event)"
            />
        </div>
    </div>
</template>

<style scoped>
.recipe-slot-grid {
    width: 100%;
    min-width: 0;
}

.recipe-crafting-editor {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1.5rem;
    padding-top: 0.25rem;
}

.recipe-crafting-grid.is-two-by-two {
    grid-template-columns: repeat(2, 3.75rem);
    grid-auto-rows: 3.75rem;
}

.recipe-result-column {
    display: flex;
    align-items: center;
    padding-left: 1.5rem;
    border-left: 1px solid var(--color-surface-5);
}

.recipe-slot-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding-top: 0.25rem;
}

@media (max-width: 32rem) {
    .recipe-crafting-editor {
        flex-direction: column;
    }

    .recipe-result-column {
        padding-left: 0;
        border-left: 0;
    }

    .recipe-slot-row {
        gap: 0.5rem;
    }
}
</style>
