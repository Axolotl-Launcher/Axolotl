<script setup lang="ts">
import { Combobox, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { NbtGroupColumn } from '@/lab/command-generator/nbt/groups.ts'

const props = defineProps<{
    column: NbtGroupColumn
    modelValue: string
    /** 输入框挂的候选列表 id（附魔、状态效果）。 */
    datalist?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    idPlaceholder: {
        id: 'app.lab.command-generator.nbt.group.id-placeholder',
        defaultMessage: 'minecraft:…',
    },
    colorPlaceholder: {
        id: 'app.lab.command-generator.nbt.group.color-placeholder',
        defaultMessage: '255,0,0',
    },
    yes: {
        id: 'app.lab.command-generator.nbt.group.yes',
        defaultMessage: 'Yes',
    },
    no: {
        id: 'app.lab.command-generator.nbt.group.no',
        defaultMessage: 'No',
    },
})

const options = computed<Array<{ value: string; label: string }>>(() => {
    if (props.column.kind === 'boolean') {
        return [
            { value: '1', label: formatMessage(messages.yes) },
            { value: '0', label: formatMessage(messages.no) },
        ]
    }
    return (props.column.enumValues ?? []).map((value) => ({ value, label: value }))
})

const placeholder = computed(() => {
    if (props.column.kind === 'id') return formatMessage(messages.idPlaceholder)
    if (props.column.kind === 'int-list') return formatMessage(messages.colorPlaceholder)
    return undefined
})

const numeric = computed(() => props.column.kind === 'number')
</script>

<template>
    <div class="flex min-w-0 flex-col gap-0.5" :class="numeric ? 'w-24' : 'min-w-32 flex-1'">
        <span class="text-[0.625rem] text-[var(--color-text-tertiary)]">
            {{ formatMessage(column.label) }}
        </span>

        <Combobox
            v-if="column.kind === 'boolean' || column.kind === 'enum'"
            :model-value="modelValue"
            :options="options"
            width="full"
            @update:model-value="emit('update:modelValue', String($event ?? ''))"
        />
        <input
            v-else-if="numeric"
            class="w-full rounded-md border-0 bg-surface-3 px-2 py-1 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
            :aria-label="formatMessage(column.label)"
            :min="column.min"
            :max="column.max"
            type="number"
            :value="modelValue"
            @change="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
        />
        <input
            v-else
            class="w-full rounded-md border-0 bg-surface-3 px-2 py-1 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
            :aria-label="formatMessage(column.label)"
            :list="datalist"
            :placeholder="placeholder"
            :value="modelValue"
            @change="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
        />
    </div>
</template>
