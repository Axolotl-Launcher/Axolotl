<template>
    <div class="relative w-full">
        <SelectRoot :id="id" :name="name" :model-value="modelValue" @update:model-value="onUpdate">
            <SelectTrigger
                :class="[headlessTokenClasses.selectTrigger, 'w-full']"
                :aria-label="name || placeholder"
            >
                <SelectValue :placeholder="placeholder">
                    {{ selectedLabel }}
                </SelectValue>
                <ChevronDownIcon class="size-4 shrink-0 text-[var(--color-text-tertiary)]" />
            </SelectTrigger>
            <SelectPortal>
                <SelectContent
                    :class="headlessTokenClasses.selectContent"
                    position="popper"
                    :side-offset="4"
                >
                    <SelectViewport>
                        <SelectItem
                            v-for="option in options"
                            :key="String(option.value)"
                            :value="option.value"
                            :class="headlessTokenClasses.selectItem"
                        >
                            <SelectItemText>{{ option.label }}</SelectItemText>
                            <SelectItemIndicator class="absolute right-2 flex items-center">
                                <CheckIcon class="size-3.5 text-brand" />
                            </SelectItemIndicator>
                        </SelectItem>
                    </SelectViewport>
                </SelectContent>
            </SelectPortal>
        </SelectRoot>
    </div>
</template>

<script setup lang="ts" generic="T extends string | number">
import { CheckIcon, ChevronDownIcon } from '@modrinth/assets'
import {
    SelectContent,
    SelectItem,
    SelectItemIndicator,
    SelectItemText,
    SelectPortal,
    SelectRoot,
    SelectTrigger,
    SelectValue,
    SelectViewport,
} from 'reka-ui'
import { computed } from 'vue'

import { headlessTokenClasses } from './token-classes'

export interface HeadlessSelectOption<T> {
    value: T
    label: string
}

const props = defineProps<{
    modelValue?: T
    options: HeadlessSelectOption<T>[]
    placeholder?: string
    id?: string
    name?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: T]
}>()

const selectedLabel = computed(() => {
    if (props.modelValue === undefined) return undefined
    return props.options.find((option) => option.value === props.modelValue)?.label
})

function onUpdate(value: string | number | undefined) {
    if (value === undefined) return
    emit('update:modelValue', value as T)
}
</script>
