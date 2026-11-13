<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { NbtString, NbtTag } from 'deepslate/nbt'
import { computed, ref, watch } from 'vue'

import { errorText } from '@/helpers/error-text'

import NbtTreeNode from './NbtTreeNode.vue'
import StudioEditor from './StudioEditor.vue'

const props = defineProps<{
    content: string
    filePath: string
    readOnly?: boolean
}>()

const emit = defineEmits<{
    'update:content': [content: string]
    'update:mode': [mode: 'tree' | 'snbt']
    format: []
    save: []
}>()

const messages = defineMessages({
    tree: { id: 'instance.files.studio.nbt.tree', defaultMessage: 'Tree' },
    snbt: { id: 'instance.files.studio.nbt.snbt', defaultMessage: 'SNBT' },
})
const { formatMessage } = useVIntl()

const mode = ref<'tree' | 'snbt'>('tree')
const root = ref<NbtTag | null>(null)
const parseError = ref('')
const draft = ref(props.content)
const snbtEditor = ref<InstanceType<typeof StudioEditor> | null>(null)
const history = ref([props.content])
let historyIndex = 0
let lastEmittedContent = props.content

function parseContent(content: string) {
    draft.value = content
    try {
        const parsed = NbtTag.fromString(content)
        if (!parsed.isCompound()) throw new Error('NBT root must be a compound')
        root.value = parsed
        parseError.value = ''
    } catch (error) {
        parseError.value = errorText(error)
    }
}

function updateSnbt(content: string) {
    draft.value = content
    parseContent(content)
    lastEmittedContent = content
    emit('update:content', content)
}

watch(
    () => props.content,
    (content) => {
        parseContent(content)
        if (content !== lastEmittedContent) {
            history.value = [content]
            historyIndex = 0
            lastEmittedContent = content
        }
    },
    { immediate: true },
)

const rootCompound = computed(() => (root.value?.isCompound() ? root.value : null))

function setMode(nextMode: 'tree' | 'snbt') {
    if (nextMode === 'tree' && parseError.value) return
    if (nextMode === 'tree') {
        history.value = [draft.value]
        historyIndex = 0
    }
    mode.value = nextMode
    emit('update:mode', nextMode)
    emit('save')
}

function handleFocusout(event: FocusEvent) {
    const currentTarget = event.currentTarget
    const nextTarget = event.relatedTarget
    if (
        currentTarget instanceof HTMLElement &&
        nextTarget instanceof Node &&
        currentTarget.contains(nextTarget)
    )
        return
    emit('save')
}

function resolve(path: (string | number)[]) {
    let current: NbtTag | undefined = root.value ?? undefined
    for (const segment of path) {
        if (!current) return undefined
        if (typeof segment === 'string' && current.isCompound()) current = current.get(segment)
        else if (typeof segment === 'number' && (current.isList() || current.isArray())) {
            current = current.get(segment)
        } else return undefined
    }
    return current
}

function parentOf(path: (string | number)[]) {
    return resolve(path.slice(0, -1))
}

function updateContent() {
    if (!root.value) return
    const content = root.value.toPrettyString()
    draft.value = content
    if (history.value[historyIndex] !== content) {
        history.value = history.value.slice(0, historyIndex + 1)
        history.value.push(content)
        historyIndex += 1
    }
    lastEmittedContent = content
    emit('update:content', content)
}

function undo() {
    if (mode.value !== 'tree' || historyIndex === 0) return
    historyIndex -= 1
    const content = history.value[historyIndex]
    parseContent(content)
    lastEmittedContent = content
    emit('update:content', content)
}

function redo() {
    if (mode.value !== 'tree' || historyIndex >= history.value.length - 1) return
    historyIndex += 1
    const content = history.value[historyIndex]
    parseContent(content)
    lastEmittedContent = content
    emit('update:content', content)
}

function handleKeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey)) return
    if (event.key.toLowerCase() === 'z') {
        event.preventDefault()
        if (event.shiftKey) redo()
        else undo()
    } else if (event.key.toLowerCase() === 'y') {
        event.preventDefault()
        redo()
    }
}

function editValue(path: (string | number)[], value: string) {
    const target = resolve(path)
    if (!target) return
    try {
        const parsed = target.isString() ? new NbtString(value) : NbtTag.fromString(value)
        if (parsed.getId() !== target.getId()) throw new Error('Value type cannot be changed')
        const parent = parentOf(path)
        const last = path.at(-1)
        if (parent?.isCompound() && typeof last === 'string') parent.set(last, parsed)
        else if (parent?.isList() && typeof last === 'number') parent.set(last, parsed)
        else if (parent?.isByteArray() && typeof last === 'number' && parsed.isByte())
            parent.set(last, parsed)
        else if (parent?.isIntArray() && typeof last === 'number' && parsed.isInt())
            parent.set(last, parsed)
        else if (parent?.isLongArray() && typeof last === 'number' && parsed.isLong())
            parent.set(last, parsed)
        else throw new Error('Value cannot be changed')
        parseError.value = ''
        updateContent()
    } catch (error) {
        parseError.value = errorText(error)
    }
}

function removeValue(path: (string | number)[]) {
    const parent = parentOf(path)
    const last = path.at(-1)
    if (parent?.isCompound() && typeof last === 'string') parent.delete(last)
    else if (parent?.isListOrArray() && typeof last === 'number') parent.delete(last)
    parseError.value = ''
    updateContent()
}

function renameValue(path: (string | number)[], name: string) {
    const parent = parentOf(path)
    const oldName = path.at(-1)
    if (!parent?.isCompound() || typeof oldName !== 'string' || parent.has(name)) return
    const value = parent.get(oldName)
    if (!value) return
    parent.delete(oldName)
    parent.set(name, value)
    parseError.value = ''
    updateContent()
}

function addValue(path: (string | number)[], input: string) {
    const parent = resolve(path)
    if (!parent) return
    try {
        const separator = parent.isCompound() ? input.indexOf(':') : -1
        const name = separator === -1 ? undefined : input.slice(0, separator).trim()
        const valueText = separator === -1 ? input.trim() : input.slice(separator + 1).trim()
        const value = NbtTag.fromString(valueText)
        if (parent.isCompound() && name && !parent.has(name)) parent.set(name, value)
        else if (parent.isList() && value.getId() === parent.getType()) parent.add(value)
        else if (parent.isByteArray() && value.isByte()) parent.add(value)
        else if (parent.isIntArray() && value.isInt()) parent.add(value)
        else if (parent.isLongArray() && value.isLong()) parent.add(value)
        else throw new Error('Value type does not match the container')
        parseError.value = ''
        updateContent()
    } catch (error) {
        parseError.value = errorText(error)
    }
}

async function formatDocument() {
    await snbtEditor.value?.formatDocument()
}

defineExpose({ formatDocument })
</script>

<template>
    <div class="flex size-full min-h-0 min-w-0 flex-col bg-surface-2" @focusout="handleFocusout">
        <div
            class="flex h-10 shrink-0 items-center gap-1 border-0 border-b border-solid border-surface-4 px-3"
        >
            <button
                v-for="candidate in ['tree', 'snbt'] as const"
                :key="candidate"
                type="button"
                class="rounded border-0 px-3 py-1 text-xs font-semibold capitalize"
                :class="
                    mode === candidate
                        ? 'bg-brand text-[var(--color-text-primary)]'
                        : 'bg-transparent text-[var(--color-text-tertiary)] hover:bg-surface-3'
                "
                @click="setMode(candidate)"
            >
                {{ formatMessage(messages[candidate]) }}
            </button>
            <span v-if="parseError" class="ml-2 truncate text-xs text-red">{{ parseError }}</span>
        </div>
        <div
            v-if="mode === 'tree'"
            class="min-h-0 flex-1 overflow-auto p-2"
            tabindex="0"
            @keydown="handleKeydown"
        >
            <NbtTreeNode
                v-if="rootCompound"
                :tag="rootCompound"
                :path="[]"
                :depth="0"
                :read-only="readOnly"
                @edit="editValue"
                @remove="removeValue"
                @rename="renameValue"
                @add="addValue"
            />
        </div>
        <StudioEditor
            v-else
            ref="snbtEditor"
            :file-path="filePath"
            :content="draft"
            :read-only="readOnly"
            :language="'snbt'"
            @update:content="updateSnbt"
        />
    </div>
</template>
