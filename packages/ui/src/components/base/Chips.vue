<template>
    <div class="chips flex gap-2 flex-wrap" role="radiogroup" :aria-label="ariaLabel">
        <Button
            v-for="item in items"
            :key="formatLabel(item)"
            v-tooltip="isDisabled(item) ? getDisabledTooltip(item) : undefined"
            role="radio"
            :aria-checked="selected === item"
            :data-state="selected === item ? 'checked' : 'unchecked'"
            :type="selected === item ? 'chip' : 'base'"
            v-bind="selected === item ? { color: 'brand' as const } : {}"
            :size="size === 'small' ? 'sm' : 'md'"
            :disabled="isDisabled(item)"
            :class="{ capitalize }"
            @click="toggleItem(item)"
        >
            <CheckIcon v-if="selected === item && !hideCheckmarkIcon" />
            <span>{{ formatLabel(item) }}</span>
        </Button>
    </div>
</template>

<script setup lang="ts" generic="T">
import { CheckIcon } from '@modrinth/assets'

import Button from './buttons/Button.vue'

const props = withDefaults(
    defineProps<{
        items: T[]
        formatLabel?: (item: T) => string
        neverEmpty?: boolean
        capitalize?: boolean
        size?: 'standard' | 'small'
        ariaLabel?: string
        disabledItems?: T[]
        disabledTooltip?: string | ((item: T) => string | undefined)
        hideCheckmarkIcon?: boolean
    }>(),
    {
        neverEmpty: true,
        // Intentional any type, as this default should only be used for primitives (string or number)
        formatLabel: (item: T) => String(item),
        capitalize: true,
        size: 'standard',
    },
)

const selected = defineModel<T | null>()

// If one always has to be selected, default to the first one
if (props.items.length > 0 && props.neverEmpty && !selected.value) {
    selected.value = props.items[0]
}

function isDisabled(item: T): boolean {
    return props.disabledItems?.includes(item) ?? false
}

function getDisabledTooltip(item: T): string | undefined {
    return typeof props.disabledTooltip === 'function'
        ? props.disabledTooltip(item)
        : props.disabledTooltip
}

function toggleItem(item: T) {
    if (isDisabled(item)) return
    if (selected.value === item && !props.neverEmpty) {
        selected.value = null
    } else {
        selected.value = item
    }
}
</script>

<style scoped>
.capitalize {
    text-transform: capitalize;
}
</style>
