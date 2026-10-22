<script setup lang="ts">
import { type MenuAction, type MenuColor, type MenuOption, PointMenu } from '@modrinth/ui'
import { type Component, computed, ref, shallowRef } from 'vue'

type ContextOption = {
    section?: MenuAction['section']
    name?: string
    id?: string
    label?: string
    type?: string
    color?: string
    tone?: string
    shown?: boolean
    disabled?: boolean
    action?: () => void
    icon?: Component
}
const emit = defineEmits(['menu-closed', 'option-clicked'])
const menu = ref<InstanceType<typeof PointMenu>>()
const item = shallowRef<unknown>()
const options = shallowRef<ContextOption[]>([])
const entries = computed<MenuOption[]>(() =>
    options.value.flatMap((option): MenuOption[] => {
        if (option.type === 'divider') return [{ divider: true }]
        const id = option.name ?? option.id ?? ''
        return [
            {
                ...option,
                id,
                color: (option.color ??
                    (option.tone === 'red' ? 'danger' : undefined)) as MenuColor,
                action: () => {
                    if (option.action) option.action()
                    else emit('option-clicked', { item: item.value, option: id })
                },
            } as MenuOption,
        ]
    }),
)
function showMenu(event: MouseEvent, target: unknown, actions: ContextOption[]) {
    item.value = target
    options.value = actions
    menu.value?.show(event)
}
const close = () => menu.value?.close()
defineExpose({
    showMenu,
    open: (event: MouseEvent, actions: ContextOption[]) => showMenu(event, null, actions),
    close,
})
</script>

<template>
    <PointMenu ref="menu" :options="entries" v-bind="$attrs" @close="emit('menu-closed')">
        <template v-for="(_, name) in $slots" #[name]><slot :name="name" /></template>
    </PointMenu>
</template>
