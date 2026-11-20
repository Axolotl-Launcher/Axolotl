<script setup lang="ts">
import { ChevronDownIcon } from '@modrinth/assets'
import { Button, Combobox, defineMessages, StyledInput, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { NbtGroupColumn } from '@/lab/command-generator/nbt/groups.ts'

const props = defineProps<{
    column: NbtGroupColumn
    modelValue: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
    /** 请求从弹窗里挑一个值（附魔、状态效果）。 */
    'request-pick': []
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
    pickEnchantment: {
        id: 'app.lab.command-generator.nbt.group.pick-enchantment',
        defaultMessage: 'Pick an enchantment',
    },
    pickEffect: {
        id: 'app.lab.command-generator.nbt.group.pick-effect',
        defaultMessage: 'Pick a status effect',
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

/** 附魔与状态效果的 ID 是有限集合，给一个弹窗；其余 ID 只能手写。 */
const pickLabel = computed(() => {
    if (props.column.kind === 'enchantment') return formatMessage(messages.pickEnchantment)
    if (props.column.kind === 'effect') return formatMessage(messages.pickEffect)
    return undefined
})

function commit(event: Event) {
    emit('update:modelValue', (event.target as HTMLInputElement).value)
}
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
        <StyledInput
            v-else-if="numeric"
            class="w-full"
            size="small"
            type="number"
            :aria-label="formatMessage(column.label)"
            :min="column.min"
            :max="column.max"
            :model-value="modelValue"
            @change="commit"
        />
        <div v-else class="flex min-w-0 items-center gap-1">
            <StyledInput
                class="min-w-0 flex-1"
                size="small"
                :aria-label="formatMessage(column.label)"
                :placeholder="placeholder"
                :model-value="modelValue"
                @change="commit"
            />
            <Button
                v-if="pickLabel"
                v-tooltip="pickLabel"
                type="outlined"
                size="2xs"
                icon-only
                :label="pickLabel"
                @click="emit('request-pick')"
            >
                <ChevronDownIcon class="h-3 w-3" />
            </Button>
        </div>
    </div>
</template>
