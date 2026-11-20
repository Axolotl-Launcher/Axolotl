<script setup lang="ts">
import { ChevronDownIcon, ChevronUpIcon, ListOrderedIcon, PlusIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    Checkbox,
    Combobox,
    type ComboboxOption,
    defineMessages,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { groupSpec } from '@/lab/command-generator/nbt/groups.ts'
import { NBT_TYPE_SHORT } from '@/lab/command-generator/nbt/labels.ts'
import {
    isNbtArray,
    isNbtContainer,
    type NbtTagNode,
    type NbtTagType,
} from '@/lab/command-generator/nbt/model.ts'

import CommandGeneratorNbtGroup from './CommandGeneratorNbtGroup.vue'

const props = defineProps<{
    node: NbtTagNode
    depth: number
    /** 同级节点数量，用来决定上移 / 下移按钮是否可用。 */
    siblingCount?: number
    siblingIndex?: number
}>()

const emit = defineEmits<{
    update: [id: string, field: string, value: string]
    remove: [id: string]
    'add-child': [id: string]
    move: [id: string, offset: number]
    change: []
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
    typeByteArray: {
        id: 'app.lab.command-generator.nbt.type.byte-array',
        defaultMessage: 'Byte array',
    },
    typeIntArray: {
        id: 'app.lab.command-generator.nbt.type.int-array',
        defaultMessage: 'Int array',
    },
    typeLongArray: {
        id: 'app.lab.command-generator.nbt.type.long-array',
        defaultMessage: 'Long array',
    },
    childCount: {
        id: 'app.lab.command-generator.nbt.child-count',
        defaultMessage: '{count} children',
    },
    elementType: {
        id: 'app.lab.command-generator.nbt.element-type',
        defaultMessage: 'Holds {type}',
    },
    arrayHint: {
        id: 'app.lab.command-generator.nbt.array-hint',
        defaultMessage: 'Comma separated, no suffixes',
    },
    toggleTag: {
        id: 'app.lab.command-generator.nbt.toggle-tag',
        defaultMessage: 'Expand or collapse tag',
    },
    tagName: { id: 'app.lab.command-generator.nbt.tag-name', defaultMessage: 'Tag name' },
    tagValue: { id: 'app.lab.command-generator.nbt.tag-value', defaultMessage: 'Tag value' },
    addTag: { id: 'app.lab.command-generator.nbt.add-tag', defaultMessage: 'Add child tag' },
    removeTag: {
        id: 'app.lab.command-generator.nbt.remove-tag',
        defaultMessage: 'Remove tag',
    },
    moveUp: {
        id: 'app.lab.command-generator.nbt.move-up',
        defaultMessage: 'Move up',
    },
    moveDown: {
        id: 'app.lab.command-generator.nbt.move-down',
        defaultMessage: 'Move down',
    },
    editAsGroup: {
        id: 'app.lab.command-generator.nbt.edit-as-group',
        defaultMessage: 'Edit as a list',
    },
    editAsRaw: {
        id: 'app.lab.command-generator.nbt.edit-as-raw',
        defaultMessage: 'Edit as raw tags',
    },
})

const expanded = ref(true)

const container = computed(() => isNbtContainer(props.node))

/** 目录里登记过结构化编辑器的标签（附魔、状态效果、描述）走表格，而不是标签树。 */
const spec = computed(() => groupSpec(props.node.group))
const asGroup = computed(() => Boolean(spec.value) && !props.node.raw)
const array = computed(() => isNbtArray(props.node))
const booleanValue = computed(() => props.node.valueKind === 'boolean')

const typeOptions = computed<ComboboxOption<string>[]>(() => [
    { value: 'string', label: formatMessage(messages.typeString) },
    { value: 'byte', label: formatMessage(messages.typeByte) },
    { value: 'short', label: formatMessage(messages.typeShort) },
    { value: 'int', label: formatMessage(messages.typeInt) },
    { value: 'long', label: formatMessage(messages.typeLong) },
    { value: 'float', label: formatMessage(messages.typeFloat) },
    { value: 'double', label: formatMessage(messages.typeDouble) },
    { value: 'compound', label: formatMessage(messages.typeCompound) },
    { value: 'list', label: formatMessage(messages.typeList) },
    { value: 'byte_array', label: formatMessage(messages.typeByteArray) },
    { value: 'int_array', label: formatMessage(messages.typeIntArray) },
    { value: 'long_array', label: formatMessage(messages.typeLongArray) },
])

const childCountLabel = computed(() =>
    formatMessage(messages.childCount, { count: props.node.children?.length ?? 0 }),
)

const elementTypeLabel = computed(() =>
    props.node.elementType
        ? formatMessage(messages.elementType, {
              type: NBT_TYPE_SHORT[props.node.elementType],
          })
        : '',
)

const canMoveUp = computed(() => (props.siblingIndex ?? 0) > 0)
const canMoveDown = computed(() => {
    const index = props.siblingIndex ?? 0
    const count = props.siblingCount ?? 1
    return index < count - 1
})

function onTypeChange(value: unknown) {
    emit('update', props.node.id, 'type', String(value) as NbtTagType)
}

/** byte 布尔在 SNBT 里就是 `1b` / `0b`，开关直接写这两个值。 */
function onBooleanChange(checked: boolean | undefined) {
    emit('update', props.node.id, 'value', checked ? '1b' : '0b')
}

const checked = computed(() => props.node.value.replace(/b$/i, '') === '1')
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="nbt-row flex min-w-0 flex-wrap items-center gap-2">
            <Button
                v-if="container"
                v-tooltip="formatMessage(messages.toggleTag)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(messages.toggleTag)"
                :aria-expanded="expanded"
                @click="expanded = !expanded"
            >
                <ChevronUpIcon v-if="expanded" />
                <ChevronDownIcon v-else />
            </Button>
            <span v-else class="w-7 shrink-0" />

            <input
                class="nbt-name min-w-0 flex-1 rounded-lg border-0 bg-surface-4 px-2 py-2 font-mono text-sm outline-none focus:ring-4 focus:ring-brand-shadow"
                :aria-label="formatMessage(messages.tagName)"
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

            <Checkbox
                v-if="!container && booleanValue"
                :model-value="checked"
                :description="formatMessage(messages.tagValue)"
                @update:model-value="onBooleanChange"
            />
            <input
                v-else-if="!container"
                class="nbt-value min-w-0 flex-1 rounded-lg border-0 bg-surface-4 px-2 py-2 font-mono text-sm outline-none focus:ring-4 focus:ring-brand-shadow"
                :aria-label="formatMessage(messages.tagValue)"
                :placeholder="array ? formatMessage(messages.arrayHint) : undefined"
                :value="String(node.value)"
                @input="emit('update', node.id, 'value', ($event.target as HTMLInputElement).value)"
            />
            <span v-else class="flex items-center gap-2 text-xs text-[var(--color-text-tertiary)]">
                <span>{{ childCountLabel }}</span>
                <span v-if="elementTypeLabel" class="font-mono">{{ elementTypeLabel }}</span>
            </span>

            <Button
                v-if="spec"
                v-tooltip="formatMessage(asGroup ? messages.editAsRaw : messages.editAsGroup)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(asGroup ? messages.editAsRaw : messages.editAsGroup)"
                :aria-pressed="asGroup"
                @click="emit('update', node.id, 'raw', node.raw ? '' : '1')"
            >
                <ListOrderedIcon />
            </Button>
            <Button
                v-if="container && !asGroup"
                v-tooltip="formatMessage(messages.addTag)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(messages.addTag)"
                @click="emit('add-child', node.id)"
            >
                <PlusIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.moveUp)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(messages.moveUp)"
                :disabled="!canMoveUp"
                @click="emit('move', node.id, -1)"
            >
                <ChevronUpIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.moveDown)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(messages.moveDown)"
                :disabled="!canMoveDown"
                @click="emit('move', node.id, 1)"
            >
                <ChevronDownIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.removeTag)"
                type="quiet"
                size="sm"
                icon-only
                :label="formatMessage(messages.removeTag)"
                @click="emit('remove', node.id)"
            >
                <XIcon />
            </Button>
        </div>

        <div
            v-if="container && expanded && asGroup && spec"
            class="ml-3 flex min-w-0 flex-col gap-2 border-0 border-l border-solid border-surface-4 pl-3"
        >
            <CommandGeneratorNbtGroup :node="node" :spec="spec" @change="emit('change')" />
        </div>

        <div
            v-else-if="container && expanded"
            class="ml-3 flex min-w-0 flex-col gap-2 border-0 border-l border-solid border-surface-4 pl-3"
        >
            <CommandGeneratorNbtNode
                v-for="(child, index) in node.children"
                :key="child.id"
                :node="child"
                :depth="depth + 1"
                :sibling-count="node.children?.length ?? 1"
                :sibling-index="index"
                @update="(id, f, v) => emit('update', id, f, v)"
                @remove="(id) => emit('remove', id)"
                @add-child="(id) => emit('add-child', id)"
                @move="(id, offset) => emit('move', id, offset)"
                @change="emit('change')"
            />
        </div>
    </div>
</template>
