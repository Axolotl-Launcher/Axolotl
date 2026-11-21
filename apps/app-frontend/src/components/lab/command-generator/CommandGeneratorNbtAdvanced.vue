<script setup lang="ts">
import { CodeIcon, PlusIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { errorText } from '@/helpers/error-text'
import {
    levelAtPath,
    type NbtLevel,
    type NbtLookup,
    rootFieldsFor,
} from '@/lab/command-generator/nbt/catalog.ts'
import {
    appendElement,
    appendField,
    renumberListElements,
} from '@/lab/command-generator/nbt/edit.ts'
import { nodeFromField, nodeFromForm } from '@/lab/command-generator/nbt/from-spec.ts'
import {
    findNbtNode,
    moveNbtNode,
    nbtKeyPathFor,
    removeNbtNode,
} from '@/lab/command-generator/nbt/model.ts'
import {
    compareVersion,
    keyForVersion,
    type NbtDomain,
    type NbtFieldSpec,
    type NbtValueForm,
} from '@/lab/command-generator/nbt/schema.ts'
import {
    createNbtTag,
    type NbtTagNode,
    parseNbt,
    serializeNbtDocument,
} from '@/lab/command-generator/nbt-tree.ts'

import CommandGeneratorNbtNode from './CommandGeneratorNbtNode.vue'
import CommandGeneratorNbtPalette from './CommandGeneratorNbtPalette.vue'

const props = defineProps<{
    modelValue?: string
    kind?: 'item' | 'entity' | 'generic'
    version?: string
    forceDataComponents?: boolean
    /** 目录域；不传时按 `kind` 推断（物品 → 组件，其余 → 实体）。 */
    domain?: NbtDomain
    /** 已知实体 / 物品 ID 时，搜索面板只列适用于它的字段。 */
    targetId?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.lab.command-generator.nbt.advanced.title',
        defaultMessage: 'Advanced NBT tree',
    },
    addRoot: {
        id: 'app.lab.command-generator.nbt.advanced.add-root',
        defaultMessage: 'Add tag',
    },
    snbtPreview: {
        id: 'app.lab.command-generator.nbt.advanced.snbt-preview',
        defaultMessage: 'SNBT preview',
    },
    mismatch: {
        id: 'app.lab.command-generator.nbt.advanced.mismatch',
        defaultMessage:
            'These tags belong to the other NBT form: {keys}. Switch the form above, or rename them.',
    },
})

const version = computed(() => props.version ?? '1.21')

/**
 * 物品的 NBT 有两种组织方式，1.20.5 是分水岭：
 * - 旧版：`{id, Count, tag:{display:…, Enchantments:…}}`，编辑的是 `tag` 里的复合标签；
 * - 1.20.5+：`[custom_name=…, enchantments={…}]`，编辑的是组件列表。
 *
 * 页面上的「使用 1.20.5+ 数据组件」开关只在 1.20–1.20.4 出现；其余版本由版本号定。
 */
const useComponents = computed(() => {
    if (props.kind !== 'item') return false
    if (props.forceDataComponents !== undefined) return props.forceDataComponents
    return compareVersion(version.value, '1.20.5') >= 0
})

const nodes = ref<NbtTagNode[]>([])
/** 当前文档实际是哪种形态；以字符串为准，空文档才跟随开关。 */
const components = ref(false)
const parseError = ref('')
let lastOutput = props.modelValue ?? ''

const snbtPreview = computed(() => serializeNbtDocument(nodes.value, components.value))

/**
 * 目录域。`/data` 的 `value` 是 `generic`：`targetType=storage` 时没有任何
 * 字段目录，这时面板不列候选字段，只让用户手写标签，而不是把实体标签摆出来。
 */
const domain = computed<NbtDomain | undefined>(() => {
    if (props.domain) return props.domain
    if (props.kind === 'item') return 'item'
    if (props.kind === 'entity') return 'entity'
    return undefined
})

const lookup = computed<NbtLookup>(() => ({
    domain: domain.value ?? 'entity',
    version: version.value,
    targetId: props.targetId,
    components: components.value,
}))

/** 这一种形态下根层能填什么：组件，或旧版 `tag` 的子键。 */
const rootFields = computed(() => (domain.value ? rootFieldsFor(lookup.value) : []))

/**
 * 根层出现**另一种形态**才有的键，说明开关与文档对不上（例如在组件列表里
 * 留着 `display`）。只比较两种形态的根键集合：用户自己写的 `new_tag` 两边
 * 都没有，不该被当成错误提示。
 */
const foreignKeys = computed(() => {
    if (props.kind !== 'item') return []
    const known = new Set(rootFields.value.map((field) => keyForVersion(field, version.value)))
    const other = new Set(
        rootFieldsFor({ ...lookup.value, components: !components.value }).map((field) =>
            keyForVersion(field, version.value),
        ),
    )
    return nodes.value
        .map((node) => node.name)
        .filter((name) => other.has(name) && !known.has(name))
})

watch(
    () => props.modelValue,
    (val) => {
        if (val === lastOutput && nodes.value.length) return
        lastOutput = val ?? ''
        try {
            const parsed = parseNbt(lastOutput)
            nodes.value = parsed.nodes
            components.value = parsed.components || (!val && useComponents.value)
            parseError.value = ''
        } catch (error) {
            parseError.value = errorText(error)
        }
    },
    { immediate: true },
)

/** 开关拨动后，文档形态跟着变，序列化也要换一种写法。空文档没有内容可换，不发事件。 */
watch(useComponents, (next) => {
    if (next === components.value) return
    components.value = next
    if (nodes.value.length) publish()
})

function publish() {
    if (parseError.value) return
    lastOutput = snbtPreview.value
    emit('update:modelValue', lastOutput)
}

function onUpdate(id: string, field: string, value: string) {
    const node = findNbtNode(nodes.value, id)
    if (node) {
        if (field === 'name') node.name = value
        else if (field === 'value') node.value = value
        else if (field === 'type') node.type = value as NbtTagNode['type']
        else if (field === 'raw') node.raw = value === '1'
        publish()
    }
}

function onRemove(id: string) {
    removeNodeById(nodes.value, id)
    publish()
}

/** NBT 本身无序，但输出文本的顺序影响可读性，所以允许手动上移 / 下移。 */
function onMove(id: string, offset: number) {
    if (moveNbtNode(nodes.value, id, offset)) publish()
}

const palette = useTemplateRef<InstanceType<typeof CommandGeneratorNbtPalette>>('palette')
/** 搜索面板添加到的容器；`null` 表示根层。 */
const paletteParentId = ref<string | null>(null)

const paletteParent = computed(() =>
    paletteParentId.value === null ? undefined : findNbtNode(nodes.value, paletteParentId.value),
)

/**
 * 面板里该列哪些字段：根层是这一形态的根字段，复合标签内部是**这个母项**的子字段。
 *
 * 不能退回根字段：`Items` 里面能填的是物品栈的 `id`/`count`/`Slot`，给它列实体字段
 * 只会让人把 `Pos` 写进物品里。
 */
const paletteLevel = computed<NbtLevel>(() => {
    if (!domain.value) return { fields: [] }
    if (paletteParentId.value === null) return { fields: rootFields.value }
    const path = nbtKeyPathFor(nodes.value, paletteParentId.value)
    if (!path) return { fields: [] }
    return levelAtPath(lookup.value, path) ?? { fields: [] }
})

const paletteFields = computed(() => paletteLevel.value.fields)

/** 列表的元素没有键名、也不能混型，面板因此只提供「添加元素」。 */
const paletteElement = computed(() =>
    paletteParent.value?.type === 'list' ? paletteLevel.value.element : undefined,
)

/** 面板标题里带上母项，明确「这些是它的子项」。列表元素用列表名，不用显示序号。 */
const paletteParentKey = computed(() => {
    if (paletteParentId.value === null) return undefined
    const path = nbtKeyPathFor(nodes.value, paletteParentId.value)
    return path?.[path.length - 1]
})

const paletteExistingKeys = computed(() => {
    if (paletteParentId.value === null) return nodes.value.map((node) => node.name)
    const parent = paletteParent.value
    if (!parent || parent.type === 'list') return []
    return (parent.children ?? []).map((node) => node.name)
})

function openPalette(parentId: string | null = null) {
    paletteParentId.value = parentId
    palette.value?.show()
}

/** 把新节点放到当前目标容器里：根层、某个复合标签的子节点，或某个列表的元素。 */
function onPalettePick(spec: NbtFieldSpec, form?: NbtValueForm) {
    const name = keyForVersion(spec, version.value)
    const siblings =
        paletteParentId.value === null ? nodes.value : (paletteParent.value?.children ?? [])
    // 同名的键已经在了就别再加：复合标签里两条同名键只有后一条生效
    if (siblings.some((node) => node.name === name)) return
    const node = form ? nodeFromForm(name, form, version.value) : nodeFromField(spec, version.value)
    if (paletteParentId.value === null) {
        nodes.value.push(node)
        publish()
        return
    }
    const parent = paletteParent.value
    if (!parent) return
    appendField(parent, node)
    publish()
}

/** 列表：插入元素，元素类型与定长都跟着列表的写法走。 */
function onPaletteElement() {
    const parent = paletteParent.value
    if (!parent) return
    if (!appendElement(parent, paletteLevel.value.form, version.value)) return
    renumberListElements(parent)
    publish()
}

function onAddChild(parentId: string) {
    openPalette(parentId)
}

/** 手动添加：面板底部的「自己写一个」走这里，落在当前选中的容器里。 */
function addRootNode() {
    const node = createNbtTag('new_tag', 'string', '')
    if (paletteParentId.value === null) {
        nodes.value.push(node)
    } else {
        const parent = paletteParent.value
        if (!parent) return
        appendField(parent, node)
    }
    publish()
}

function removeNodeById(list: NbtTagNode[], id: string): boolean {
    return removeNbtNode(list, id)
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <header
            class="flex items-center gap-2 text-sm font-semibold text-[var(--color-text-primary)]"
        >
            <CodeIcon class="h-4 w-4" />
            {{ formatMessage(messages.title) }}
        </header>

        <p v-if="parseError" class="text-sm text-red">{{ parseError }}</p>
        <div v-else class="flex min-w-0 flex-col gap-2">
            <p
                v-if="foreignKeys.length"
                class="m-0 rounded-lg bg-surface-3 px-3 py-2 text-xs leading-4 text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.mismatch, { keys: foreignKeys.join(', ') }) }}
            </p>
            <CommandGeneratorNbtNode
                v-for="(node, index) in nodes"
                :key="node.id"
                :node="node"
                :depth="0"
                :sibling-count="nodes.length"
                :sibling-index="index"
                :version="version"
                :domain="domain"
                :target-id="targetId"
                :components="components"
                :key-path="[node.name]"
                @update="onUpdate"
                @remove="onRemove"
                @add-child="onAddChild"
                @move="onMove"
                @change="publish"
            />
            <button
                class="nbt-add-root mt-1 flex items-center gap-2 self-start rounded-lg bg-surface-3 px-3 py-2 text-sm font-medium hover:bg-surface-4 focus-visible:outline-brand"
                type="button"
                @click="openPalette(null)"
            >
                <PlusIcon class="h-4 w-4" />
                {{ formatMessage(messages.addRoot) }}
            </button>
        </div>

        <div v-if="nodes.length" class="min-w-0 rounded-lg bg-surface-3 p-3">
            <span class="mb-2 block text-xs text-[var(--color-text-tertiary)]">{{
                formatMessage(messages.snbtPreview)
            }}</span>
            <code
                class="block max-h-48 overflow-auto break-all text-xs leading-5 text-[var(--color-text-default)]"
                >{{ snbtPreview }}</code
            >
        </div>

        <CommandGeneratorNbtPalette
            ref="palette"
            :domain="domain"
            :version="version"
            :target-id="targetId"
            :existing-keys="paletteExistingKeys"
            :fields="paletteFields"
            :element="paletteElement"
            :parent-key="paletteParentKey"
            @pick="onPalettePick"
            @element="onPaletteElement"
            @manual="addRootNode"
        />
    </div>
</template>
