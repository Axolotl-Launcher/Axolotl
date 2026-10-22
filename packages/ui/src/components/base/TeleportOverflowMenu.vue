<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import IconButton from './buttons/IconButton.vue'
import type { MenuColor, MenuOption } from './menu-options'
import OverflowMenu from './OverflowMenu.vue'

export type Option = {
    id: string
    label?: string
    icon?: import('vue').Component
    action?: (() => void) | string
    shown?: boolean
    color?: MenuColor
    disabled?: boolean
    tooltip?: string
}
const props = withDefaults(
    defineProps<{
        options: (Option | { divider?: boolean; shown?: boolean })[]
        label?: string
        disabled?: boolean
        hoverable?: boolean
        btnClass?: string | string[] | Record<string, boolean>
    }>(),
    { label: 'More actions', disabled: false, hoverable: false },
)
const emit = defineEmits<{ select: [option: Option]; open: [] }>()
const openState = ref(false)
const options = computed<MenuOption[]>(() =>
    props.options.map((option) => {
        if (!('id' in option)) return { divider: true, shown: option.shown }
        return {
            ...option,
            link: typeof option.action === 'string' ? option.action : undefined,
            action: () => {
                emit('select', option)
                if (typeof option.action === 'function') option.action()
            },
        }
    }),
)
watch(openState, (open) => {
    if (open) emit('open')
})
</script>

<template>
    <OverflowMenu
        v-model:open="openState"
        :options="options"
        :hoverable="hoverable"
        :content-attrs="{ 'data-pyro-telepopover-root': '' }"
        :disabled="disabled"
    >
        <template #trigger>
            <IconButton :label="label" type="quiet" :class="btnClass" :disabled="disabled"
                ><slot
            /></IconButton>
        </template>
        <template v-for="(_, name) in $slots" #[name]><slot :name="name" /></template>
    </OverflowMenu>
</template>
