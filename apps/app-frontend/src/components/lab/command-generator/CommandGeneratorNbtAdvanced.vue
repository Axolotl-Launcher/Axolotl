<script setup lang="ts">
import { CodeIcon, PlusIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { errorText } from '@/helpers/error-text'
import { childrenAtPath } from '@/lab/command-generator/nbt/catalog.ts'
import { nodeFromField } from '@/lab/command-generator/nbt/from-spec.ts'
import {
    findNbtNode,
    moveNbtNode,
    nbtKeyPathFor,
    removeNbtNode,
} from '@/lab/command-generator/nbt/model.ts'
import type { NbtDomain, NbtFieldSpec } from '@/lab/command-generator/nbt/schema.ts'
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
})

const nodes = ref<NbtTagNode[]>([])
const components = ref(false)
const parseError = ref('')
let lastOutput = props.modelValue ?? ''

const itemFormat = computed(() => {
    if (props.kind !== 'item') return 'legacy'
    if ((props.version ?? '1.21').localeCompare('1.21.5', undefined, { numeric: true }) >= 0)
        return 'inline'
    if (
        props.forceDataComponents ??
        (props.version ?? '1.21').localeCompare('1.20', undefined, { numeric: true }) >= 0
    )
        return 'components'
    return 'legacy'
})

const snbtPreview = computed(() => serializeNbtDocument(nodes.value, components.value))

watch(
    () => props.modelValue,
    (val) => {
        if (val === lastOutput && nodes.value.length) return
        lastOutput = val ?? ''
        try {
            const parsed = parseNbt(lastOutput)
            nodes.value = parsed.nodes
            components.value = parsed.components || (!val && itemFormat.value !== 'legacy')
            parseError.value = ''
        } catch (error) {
            parseError.value = errorText(error)
        }
    },
    { immediate: true },
)

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

const domain = computed<NbtDomain>(
    () => props.domain ?? (props.kind === 'item' ? 'item' : 'entity'),
)

/**
 * 面板里该列哪些字段：根层是这一域的全部字段，复合标签内部是**这个母项**的子字段。
 *
 * 不能退回根字段：`Items` 里面能填的是物品栈的 `id`/`count`/`Slot`，给它列实体字段
 * 只会让人把 `Pos` 写进物品里。
 */
const paletteFields = computed<NbtFieldSpec[] | undefined>(() => {
    if (paletteParentId.value === null) return undefined
    const path = nbtKeyPathFor(nodes.value, paletteParentId.value)
    if (!path) return []
    return childrenAtPath(domain.value, path, props.targetId) ?? []
})

/** 面板标题里带上母项，明确「这些是它的子项」。 */
const paletteParentKey = computed(() => {
    if (paletteParentId.value === null) return undefined
    return findNbtNode(nodes.value, paletteParentId.value)?.name
})

const paletteExistingKeys = computed(() => {
    const target =
        paletteParentId.value === null
            ? nodes.value
            : (findNbtNode(nodes.value, paletteParentId.value)?.children ?? [])
    return target.map((node) => node.name)
})

function openPalette(parentId: string | null = null) {
    paletteParentId.value = parentId
    palette.value?.show()
}

/** 把新节点放到当前目标容器里：根层或某个复合标签的子节点。 */
function appendNode(node: NbtTagNode) {
    if (paletteParentId.value === null) {
        nodes.value.push(node)
        publish()
        return
    }
    const parent = findNbtNode(nodes.value, paletteParentId.value)
    if (!parent) return
    if (!parent.children) parent.children = []
    parent.children.push(node)
    publish()
}

function onPalettePick(spec: NbtFieldSpec) {
    appendNode(nodeFromField(spec))
}

function onAddChild(parentId: string) {
    openPalette(parentId)
}

/** 手动添加：面板底部的「自己写一个」走这里，落在当前选中的容器里。 */
function addRootNode() {
    appendNode(createNbtTag('new_tag', 'string', ''))
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
            <CommandGeneratorNbtNode
                v-for="(node, index) in nodes"
                :key="node.id"
                :node="node"
                :depth="0"
                :sibling-count="nodes.length"
                :sibling-index="index"
                :version="version ?? '1.21'"
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
            :version="version ?? '1.21'"
            :target-id="targetId"
            :existing-keys="paletteExistingKeys"
            :fields="paletteFields"
            :parent-key="paletteParentKey"
            @pick="onPalettePick"
            @manual="addRootNode"
        />
    </div>
</template>
