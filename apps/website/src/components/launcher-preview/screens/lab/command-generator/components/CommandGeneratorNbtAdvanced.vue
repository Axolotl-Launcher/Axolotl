<script setup lang="ts">
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, watch } from 'vue'

import {
    createItemNbtTree,
    createNbtTag,
    type NbtTagNode,
    parseNbt,
    serializeNbtDocument,
} from '../nbt-tree.ts'
import CommandGeneratorNbtNode from './CommandGeneratorNbtNode.vue'

const props = defineProps<{
    modelValue?: string
    kind?: 'item' | 'entity' | 'generic'
    version?: string
    forceDataComponents?: boolean
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
        defaultMessage: 'Add root tag',
    },
    snbtPreview: {
        id: 'app.lab.command-generator.nbt.advanced.snbt-preview',
        defaultMessage: 'SNBT preview',
    },
    presetEnchanted: {
        id: 'app.lab.command-generator.nbt.advanced.preset-enchanted',
        defaultMessage: 'Enchanted item',
    },
    presetCustomName: {
        id: 'app.lab.command-generator.nbt.advanced.preset-custom-name',
        defaultMessage: 'Custom name',
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
const presets = [
    { label: messages.presetEnchanted, factory: () => createItemNbtTree(itemFormat.value) },
    {
        label: messages.presetCustomName,
        factory: () =>
            itemFormat.value !== 'legacy'
                ? createItemNbtTree(itemFormat.value).slice(0, 1)
                : [
                      createNbtTag('display', 'compound', '', [
                          createNbtTag('Name', 'string', '{"text":"我的物品"}'),
                      ]),
                  ],
    },
]

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
            parseError.value = error instanceof Error ? error.message : String(error)
        }
    },
    { immediate: true },
)

function publish() {
    if (parseError.value) return
    lastOutput = snbtPreview.value
    emit('update:modelValue', lastOutput)
}

function applyPreset(preset: (typeof presets)[number]) {
    components.value = itemFormat.value !== 'legacy'
    parseError.value = ''
    nodes.value = preset.factory()
    publish()
}

function onUpdate(id: string, field: string, value: string) {
    const node = findNodeById(nodes.value, id)
    if (node) {
        if (field === 'name') node.name = value
        else if (field === 'value') node.value = value
        else if (field === 'type') node.type = value as NbtTagNode['type']
        publish()
    }
}

function onRemove(id: string) {
    removeNodeById(nodes.value, id)
    publish()
}

function onAddChild(parentId: string) {
    const parent = findNodeById(nodes.value, parentId)
    if (parent) {
        if (!parent.children) parent.children = []
        parent.children.push(createNbtTag('new_tag', 'string', ''))
        publish()
    }
}

function addRootNode() {
    nodes.value.push(createNbtTag('new_tag', 'string', ''))
    publish()
}

function findNodeById(list: NbtTagNode[], id: string): NbtTagNode | undefined {
    for (const node of list) {
        if (node.id === id) return node
        if (node.children) {
            const found = findNodeById(node.children, id)
            if (found) return found
        }
    }
    return undefined
}

function removeNodeById(list: NbtTagNode[], id: string): boolean {
    for (let i = 0; i < list.length; i++) {
        if (list[i].id === id) {
            list.splice(i, 1)
            return true
        }
        if (list[i].children) {
            if (removeNodeById(list[i].children!, id)) return true
        }
    }
    return false
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <header class="flex flex-wrap items-center justify-between gap-2">
            <span
                class="flex items-center gap-2 text-sm font-semibold text-[var(--color-text-primary)]"
            >
                <CodeIcon class="h-4 w-4" />
                {{ formatMessage(messages.title) }}
            </span>
            <div class="flex flex-wrap gap-2">
                <Button
                    v-for="preset in presets"
                    :key="preset.label.id"
                    type="outlined"
                    size="2xs"
                    @click="applyPreset(preset)"
                >
                    {{ formatMessage(preset.label) }}
                </Button>
            </div>
        </header>

        <p v-if="parseError" class="text-sm text-red">{{ parseError }}</p>
        <div v-else class="flex min-w-0 flex-col gap-2">
            <CommandGeneratorNbtNode
                v-for="node in nodes"
                :key="node.id"
                :node="node"
                :depth="0"
                @update="onUpdate"
                @remove="onRemove"
                @add-child="onAddChild"
            />
            <button
                class="mt-1 flex items-center gap-2 self-start rounded-lg bg-surface-3 px-3 py-2 text-sm font-medium hover:bg-surface-4 focus-visible:outline-brand"
                type="button"
                @click="addRootNode"
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
    </div>
</template>
