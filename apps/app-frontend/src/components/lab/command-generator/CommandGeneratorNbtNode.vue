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

import { levelAtPath, type NbtLookup } from '@/lab/command-generator/nbt/catalog.ts'
import { canInsert, insertKindFor } from '@/lab/command-generator/nbt/edit.ts'
import { groupSpec, shapeMatches } from '@/lab/command-generator/nbt/groups.ts'
import { NBT_TYPE_SHORT } from '@/lab/command-generator/nbt/labels.ts'
import {
    isNbtArray,
    isNbtContainer,
    type NbtTagNode,
    type NbtTagType,
} from '@/lab/command-generator/nbt/model.ts'
import { formFor, type NbtDomain } from '@/lab/command-generator/nbt/schema.ts'
import {
    type NbtValueOption,
    sameNbtValue,
    valueOptions,
} from '@/lab/command-generator/nbt/values.ts'

import CommandGeneratorNbtGroup from './CommandGeneratorNbtGroup.vue'

const props = defineProps<{
    node: NbtTagNode
    depth: number
    /** 同级节点数量，用来决定上移 / 下移按钮是否可用。 */
    siblingCount?: number
    siblingIndex?: number
    /** 结构化组用它过滤该版本不存在的附魔 / 状态效果。 */
    version?: string
    /** 目录域与目标，用来按路径查回这个节点在目录里的字段；没有目录时为 undefined。 */
    domain?: NbtDomain
    targetId?: string
    /** 物品：true 是 1.20.5+ 数据组件，false 是旧版 `tag` 的子键。 */
    components?: boolean
    /** 从根到这里的键名路径；列表元素不贡献键名。 */
    keyPath: string[]
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
    toggleEmpty: {
        id: 'app.lab.command-generator.nbt.toggle-tag.empty',
        defaultMessage: 'No child tags to expand',
    },
    tagName: { id: 'app.lab.command-generator.nbt.tag-name', defaultMessage: 'Tag name' },
    tagValue: { id: 'app.lab.command-generator.nbt.tag-value', defaultMessage: 'Tag value' },
    addTag: { id: 'app.lab.command-generator.nbt.add-tag', defaultMessage: 'Add child tag' },
    addElement: {
        id: 'app.lab.command-generator.nbt.add-element',
        defaultMessage: 'Add list entry',
    },
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
const array = computed(() => isNbtArray(props.node))
/** 空容器展开后什么也没有，箭头因此禁用；加了子项就自动恢复。 */
const hasChildren = computed(() => (props.node.children?.length ?? 0) > 0)
const toggleLabel = computed(() =>
    formatMessage(hasChildren.value ? messages.toggleTag : messages.toggleEmpty),
)

const version = computed(() => props.version ?? '1.21')

/** 没有目录域时不做任何查表：所有标签都按原始标签编辑。 */
const lookup = computed<NbtLookup | undefined>(() =>
    props.domain
        ? {
              domain: props.domain,
              version: version.value,
              targetId: props.targetId,
              components: props.components,
          }
        : undefined,
)

/**
 * 这个节点在目录里对应哪个字段、用的哪种写法。
 *
 * 路径是唯一线索：`patterns` 在旗帜方块实体里是 `{color, pattern}`，在物品
 * 组件里是另一套结构；只看键名会拿错母项的结构。粘贴进来的 SNBT 也因此能
 * 直接拿到表格编辑器，而不必先从面板添加一遍。
 */
const level = computed(() =>
    props.keyPath.length && lookup.value ? levelAtPath(lookup.value, props.keyPath) : undefined,
)
const form = computed(() => {
    const field = level.value?.spec
    if (!field) return undefined
    return formFor(field, version.value, props.node.type)
})

/** 目录里登记过结构化编辑器的标签（附魔、状态效果、物品列表）走表格，而不是标签树。 */
const spec = computed(() => groupSpec(form.value?.editor))
/**
 * 节点结构必须和组定义相符：`{levels:…}` 与裸映射是两种写法，用表格写回
 * 会把另一种写法整体改成列表，所以形状不符时退回原始标签。
 */
const asGroup = computed(
    () => Boolean(spec.value) && !props.node.raw && shapeMatches(props.node, spec.value!),
)

const insertKind = computed(() => insertKindFor(props.node))
/** 定长列表满了、标量、数组都不给「添加」按钮：点了也不会发生任何事。 */
const canAddChild = computed(() => canInsert(props.node, form.value))
const addLabel = computed(() =>
    formatMessage(insertKind.value === 'element' ? messages.addElement : messages.addTag),
)

const booleanValue = computed(() => Boolean(form.value?.valueKind ?? props.node.valueKind))

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

/** 目录给了枚举来源时用下拉框：染料颜色 16 个、旗帜图案是有限集合。 */
const options = computed<NbtValueOption[]>(() => {
    const known = valueOptions(form.value?.values, version.value)
    if (!known.length) return []
    // 手写的值不在集合里也要留在下拉框里，否则界面上会变成空白
    if (!props.node.value) return known
    if (known.some((option) => sameNbtValue(props.node.value, option.value))) return known
    return [{ value: props.node.value, label: props.node.value }, ...known]
})

const selectedValue = computed(() => {
    const match = options.value.find((option) => sameNbtValue(props.node.value, option.value))
    return match?.value ?? props.node.value
})

function onTypeChange(value: unknown) {
    emit('update', props.node.id, 'type', String(value) as NbtTagType)
}

/** byte 布尔在 SNBT 里就是 `1b` / `0b`；节点存裸值，后缀由序列化补。 */
function onBooleanChange(checked: boolean | undefined) {
    emit('update', props.node.id, 'value', checked ? '1' : '0')
}

const checked = computed(() => props.node.value.replace(/b$/i, '') === '1')

/** 列表的元素与列表共用同一条键名路径；复合标签的子项再往下走一层。 */
function childKeyPath(child: NbtTagNode): string[] {
    return props.node.type === 'list' ? props.keyPath : [...props.keyPath, child.name]
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="nbt-row flex min-w-0 flex-wrap items-center gap-2">
            <Button
                v-if="container"
                v-tooltip="toggleLabel"
                type="quiet"
                size="sm"
                icon-only
                :label="toggleLabel"
                :disabled="!hasChildren"
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
            <div v-else-if="!container && options.length" class="min-w-0 flex-1">
                <Combobox
                    :model-value="selectedValue"
                    :options="options"
                    width="full"
                    @update:model-value="emit('update', node.id, 'value', String($event ?? ''))"
                />
            </div>
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
                v-if="spec && container"
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
                v-if="canAddChild && !asGroup"
                v-tooltip="addLabel"
                type="quiet"
                size="sm"
                icon-only
                :label="addLabel"
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
            <CommandGeneratorNbtGroup
                :node="node"
                :spec="spec"
                :version="version"
                @change="emit('change')"
            />
        </div>

        <div
            v-else-if="container && expanded && hasChildren"
            class="ml-3 flex min-w-0 flex-col gap-2 border-0 border-l border-solid border-surface-4 pl-3"
        >
            <CommandGeneratorNbtNode
                v-for="(child, index) in node.children"
                :key="child.id"
                :node="child"
                :depth="depth + 1"
                :sibling-count="node.children?.length ?? 1"
                :sibling-index="index"
                :version="version"
                :domain="domain"
                :target-id="targetId"
                :components="components"
                :key-path="childKeyPath(child)"
                @update="(id, f, v) => emit('update', id, f, v)"
                @remove="(id) => emit('remove', id)"
                @add-child="(id) => emit('add-child', id)"
                @move="(id, offset) => emit('move', id, offset)"
                @change="emit('change')"
            />
        </div>
    </div>
</template>
