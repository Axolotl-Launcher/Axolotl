<template>
    <PopoutMenu
        ref="dropdown"
        v-bind="$attrs"
        v-model:open="openState"
        data-button-group-item
        :disabled="disabled"
        :dropdown-id="dropdownId"
        :tooltip="tooltip"
        :placement="placement"
        menu
    >
        <slot></slot>
        <template v-if="$slots.trigger" #trigger><slot name="trigger" /></template>
        <template #menu>
            <slot name="menu-header" />
            <MenuItems :options="options">
                <template v-for="(_, name) in $slots" #[name]><slot :name="name" /></template>
            </MenuItems>
        </template>
    </PopoutMenu>
</template>

<script setup lang="ts">
import { ref } from 'vue'

import type { MenuOption } from './menu-options'
import MenuItems from './MenuItems.vue'
import PopoutMenu from './PopoutMenu.vue'

export type Option = MenuOption
withDefaults(
    defineProps<{
        options: MenuOption[]
        disabled?: boolean
        dropdownId?: string
        tooltip?: string
        placement?: string
    }>(),
    { options: () => [], disabled: false, placement: 'bottom-end' },
)
defineOptions({ inheritAttrs: false })
const openState = defineModel<boolean>('open', { default: false })
const dropdown = ref<InstanceType<typeof PopoutMenu>>()
const close = () => dropdown.value?.hide()
const open = () => dropdown.value?.show()
defineExpose({ open, close })
</script>
