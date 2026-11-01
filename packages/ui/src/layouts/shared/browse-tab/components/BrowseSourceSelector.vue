<script setup lang="ts">
import type { Component } from 'vue'

import Button from '#ui/components/base/buttons/Button.vue'
import PopoutMenu from '#ui/components/base/PopoutMenu.vue'

defineProps<{
    selected: string
    label: string
    icon: Component
    options: { id: string; label: string; icon: Component }[]
    disabled?: boolean
}>()
const emit = defineEmits<{ select: [source: string] }>()

function selectSource(source: string, hide: () => void) {
    emit('select', source)
    hide()
}
</script>

<template>
    <PopoutMenu placement="bottom-end">
        <template #trigger>
            <Button size="md" class="flex items-center gap-2">
                <component :is="icon" class="size-5" />
                <span>{{ label }}</span>
            </Button>
        </template>
        <template #menu="{ hide }">
            <div class="flex w-min flex-col gap-1 p-1">
                <Button
                    v-for="option in options"
                    :key="option.id"
                    :type="selected === option.id ? 'colored' : 'quiet'"
                    class="flex w-full items-center gap-2 !justify-start text-left"
                    :disabled="disabled"
                    :aria-pressed="selected === option.id"
                    @click="selectSource(option.id, hide)"
                >
                    <component :is="option.icon" class="size-4" />
                    {{ option.label }}
                </Button>
            </div>
        </template>
    </PopoutMenu>
</template>
