<script setup lang="ts">
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ChevronUpIcon from '@modrinth/assets/icons/chevron-up.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import { commandGeneratorMessages as commonMessages } from '../i18n/messages.ts'
import type { NbtTagNode, NbtTagType } from '../nbt-tree.ts'

const props = defineProps<{
    node: NbtTagNode
    depth: number
}>()

const emit = defineEmits<{
    update: [id: string, field: string, value: string]
    remove: [id: string]
    'add-child': [id: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    typeString: {
        id: 'app.lab.command-generator.nbt.type.string',
        defaultMessage: 'String',
    },
    typeInt: {
        id: 'app.lab.command-generator.nbt.type.int',
        defaultMessage: 'Int',
    },
    typeLong: { id: 'app.lab.command-generator.nbt.type.long', defaultMessage: 'Long' },
    typeRaw: { id: 'app.lab.command-generator.nbt.type.raw', defaultMessage: 'SNBT array' },
    typeShort: {
        id: 'app.lab.command-generator.nbt.type.short',
        defaultMessage: 'Short',
    },
    typeByte: {
        id: 'app.lab.command-generator.nbt.type.byte',
        defaultMessage: 'Byte',
    },
    typeFloat: {
        id: 'app.lab.command-generator.nbt.type.float',
        defaultMessage: 'Float',
    },
    typeDouble: {
        id: 'app.lab.command-generator.nbt.type.double',
        defaultMessage: 'Double',
    },
    typeCompound: {
        id: 'app.lab.command-generator.nbt.type.compound',
        defaultMessage: 'Compound',
    },
    typeList: {
        id: 'app.lab.command-generator.nbt.type.list',
        defaultMessage: 'List',
    },
    childCount: {
        id: 'app.lab.command-generator.nbt.child-count',
        defaultMessage: '{count} children',
    },
})

const expanded = ref(true)

const isContainer = computed(() => props.node.type === 'compound' || props.node.type === 'list')

const typeOptions = computed<ComboboxOption[]>(() => [
    { value: 'string', label: formatMessage(messages.typeString) },
    { value: 'int', label: formatMessage(messages.typeInt) },
    { value: 'long', label: formatMessage(messages.typeLong) },
    { value: 'raw', label: formatMessage(messages.typeRaw) },
    { value: 'short', label: formatMessage(messages.typeShort) },
    { value: 'byte', label: formatMessage(messages.typeByte) },
    { value: 'float', label: formatMessage(messages.typeFloat) },
    { value: 'double', label: formatMessage(messages.typeDouble) },
    { value: 'compound', label: formatMessage(messages.typeCompound) },
    { value: 'list', label: formatMessage(messages.typeList) },
])

const childCountLabel = computed(() =>
    formatMessage(messages.childCount, { count: props.node.children?.length ?? 0 }),
)

function onTypeChange(value: unknown) {
    emit('update', props.node.id, 'type', String(value) as NbtTagType)
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="nbt-row flex min-w-0 flex-wrap items-center gap-2">
            <Button
                v-if="isContainer"
                v-tooltip="formatMessage(commonMessages.toggleTag)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(commonMessages.toggleTag)"
                :aria-expanded="expanded"
                @click="expanded = !expanded"
            >
                <ChevronUpIcon v-if="expanded" />
                <ChevronDownIcon v-else />
            </Button>
            <span v-else class="w-7 shrink-0" />

            <input
                class="nbt-name min-w-0 flex-1 rounded-lg border-0 bg-surface-4 px-2 py-2 text-sm outline-none focus:ring-4 focus:ring-brand-shadow"
                :aria-label="formatMessage(commonMessages.tagName)"
                :value="node.name"
                @input="emit('update', node.id, 'name', ($event.target as HTMLInputElement).value)"
            />

            <div class="min-w-0">
                <Combobox
                    :model-value="node.type"
                    :options="typeOptions"
                    @update:model-value="onTypeChange"
                />
            </div>

            <input
                v-if="!isContainer"
                class="nbt-value min-w-0 flex-1 rounded-lg border-0 bg-surface-4 px-2 py-2 text-sm outline-none focus:ring-4 focus:ring-brand-shadow"
                :aria-label="formatMessage(commonMessages.tagValue)"
                :value="String(node.value)"
                @input="emit('update', node.id, 'value', ($event.target as HTMLInputElement).value)"
            />
            <span v-else class="text-xs text-[var(--color-text-tertiary)]">{{
                childCountLabel
            }}</span>

            <Button
                v-if="isContainer"
                v-tooltip="formatMessage(commonMessages.addTag)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(commonMessages.addTag)"
                @click="emit('add-child', node.id)"
            >
                <PlusIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(commonMessages.removeTag)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(commonMessages.removeTag)"
                @click="emit('remove', node.id)"
            >
                <XIcon />
            </Button>
        </div>

        <div
            v-if="isContainer && expanded"
            class="ml-3 flex min-w-0 flex-col gap-2 border-0 border-l border-solid border-surface-4 pl-3"
        >
            <CommandGeneratorNbtNode
                v-for="child in node.children"
                :key="child.id"
                :node="child"
                :depth="depth + 1"
                @update="(id, f, v) => emit('update', id, f, v)"
                @remove="(id) => emit('remove', id)"
                @add-child="(id) => emit('add-child', id)"
            />
        </div>
    </div>
</template>
