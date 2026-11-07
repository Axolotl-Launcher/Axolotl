<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { type NbtTag, NbtType } from 'deepslate/nbt'
import { computed, ref } from 'vue'

import NbtTypeIcon from './NbtTypeIcon.vue'

const props = defineProps<{
    name?: string
    tag: NbtTag
    path: (string | number)[]
    depth: number
    readOnly?: boolean
}>()

const emit = defineEmits<{
    edit: [path: (string | number)[], value: string]
    remove: [path: (string | number)[]]
    rename: [path: (string | number)[], value: string]
    add: [path: (string | number)[], value: string]
}>()

const messages = defineMessages({
    collapse: { id: 'instance.files.studio.nbt.collapse', defaultMessage: 'Collapse node' },
    expand: { id: 'instance.files.studio.nbt.expand', defaultMessage: 'Expand node' },
    add: { id: 'instance.files.studio.nbt.add', defaultMessage: 'Add child' },
    remove: { id: 'instance.files.studio.nbt.remove', defaultMessage: 'Remove node' },
    addPlaceholder: {
        id: 'instance.files.studio.nbt.add-placeholder',
        defaultMessage: 'name:value',
    },
})
const { formatMessage } = useVIntl()

const expanded = ref(props.depth < 1)
const editing = ref(false)
const renaming = ref(false)
const draft = ref('')
const renameDraft = ref(props.name ?? '')
const adding = ref(false)
const addDraft = ref('')

const expandable = computed(
    () =>
        props.tag.isCompound() ||
        props.tag.isList() ||
        props.tag.isByteArray() ||
        props.tag.isIntArray() ||
        props.tag.isLongArray(),
)
const typeName = computed(() => NbtType[props.tag.getId()])
const displayValue = computed(() => {
    if (props.tag.isCompound()) return `${props.tag.size} entries`
    if (props.tag.isList()) return `${props.tag.length} ${NbtType[props.tag.getType()]} values`
    if (props.tag.isArray()) return `${props.tag.length} values`
    return props.tag.toString()
})

function children(): Array<{ name?: string; tag: NbtTag; path: (string | number)[] }> {
    const tag = props.tag
    if (tag.isCompound()) {
        return [...tag.keys()].map((name) => ({
            name,
            tag: tag.get(name)!,
            path: [...props.path, name],
        }))
    }
    if (tag.isList()) {
        return Array.from({ length: tag.length }, (_, index) => ({
            tag: tag.get(index)!,
            path: [...props.path, index],
        }))
    }
    if (tag.isArray()) {
        return Array.from({ length: tag.length }, (_, index) => ({
            tag: tag.get(index)!,
            path: [...props.path, index],
        }))
    }
    return []
}

function beginEdit() {
    if (props.readOnly || expandable.value) return
    draft.value = props.tag.isString() ? props.tag.getAsString() : props.tag.toString()
    editing.value = true
}

function commitEdit() {
    if (draft.value.trim()) emit('edit', props.path, draft.value)
    editing.value = false
}

function commitRename() {
    if (renameDraft.value.trim() && renameDraft.value !== props.name) {
        emit('rename', props.path, renameDraft.value.trim())
    }
    renaming.value = false
}

function commitAdd() {
    if (addDraft.value.trim()) emit('add', props.path, addDraft.value)
    addDraft.value = ''
    adding.value = false
}

function forwardEdit(path: (string | number)[], value: string) {
    emit('edit', path, value)
}

function forwardRename(path: (string | number)[], value: string) {
    emit('rename', path, value)
}

function forwardAdd(path: (string | number)[], value: string) {
    emit('add', path, value)
}
</script>

<template>
    <div>
        <div
            class="group flex min-h-8 items-center gap-2 rounded px-2 text-sm hover:bg-surface-3"
            :style="{ paddingLeft: `${depth * 1.25 + 0.5}rem` }"
        >
            <button
                type="button"
                class="flex size-5 shrink-0 items-center justify-center border-0 bg-transparent p-0 text-[var(--color-text-tertiary)]"
                :class="expandable ? 'cursor-pointer' : 'cursor-default'"
                :aria-label="formatMessage(expanded ? messages.collapse : messages.expand)"
                @click="expandable && (expanded = !expanded)"
            >
                <span v-if="expandable">{{ expanded ? '▾' : '▸' }}</span>
            </button>
            <NbtTypeIcon :type="tag.getId()" />
            <template v-if="name !== undefined">
                <input
                    v-if="renaming"
                    v-model="renameDraft"
                    class="min-w-0 flex-1 rounded border border-surface-5 bg-surface-1 px-1 text-sm text-[var(--color-text-primary)]"
                    @blur="commitRename"
                    @keydown.enter.prevent="commitRename"
                />
                <button
                    v-else
                    type="button"
                    class="shrink-0 border-0 bg-transparent p-0 text-[var(--color-text-tertiary)]"
                    :class="{ 'cursor-text': !readOnly }"
                    @dblclick="!readOnly && (renaming = true)"
                >
                    {{ name }}:
                </button>
            </template>
            <span class="text-xs text-[var(--color-text-tertiary)]">{{ typeName }}</span>
            <input
                v-if="editing"
                v-model="draft"
                autofocus
                class="min-w-0 flex-1 rounded border border-brand bg-surface-1 px-2 py-0.5 font-mono text-xs text-[var(--color-text-primary)]"
                @blur="commitEdit"
                @keydown.enter.prevent="commitEdit"
                @keydown.escape="editing = false"
            />
            <button
                v-else
                type="button"
                class="min-w-0 truncate border-0 bg-transparent p-0 text-left font-mono text-xs text-[var(--color-text-default)]"
                :class="{ 'cursor-text': !expandable && !readOnly }"
                @dblclick="beginEdit"
            >
                {{ displayValue }}
            </button>
            <button
                v-if="!readOnly && expandable"
                type="button"
                class="ml-auto hidden rounded border-0 bg-transparent px-1 text-xs text-[var(--color-text-tertiary)] group-hover:inline-flex hover:text-[var(--color-text-primary)]"
                :aria-label="formatMessage(messages.add)"
                @click="adding = !adding"
            >
                +
            </button>
            <button
                v-if="!readOnly && path.length > 0"
                type="button"
                class="hidden rounded border-0 bg-transparent px-1 text-xs text-[var(--color-text-tertiary)] group-hover:inline-flex hover:text-red"
                :aria-label="formatMessage(messages.remove)"
                @click="emit('remove', path)"
            >
                ×
            </button>
        </div>
        <div
            v-if="adding"
            class="flex items-center gap-2 px-3 py-1"
            :style="{ paddingLeft: `${(depth + 1) * 1.25 + 2.25}rem` }"
        >
            <input
                v-model="addDraft"
                autofocus
                class="min-w-0 flex-1 rounded border border-surface-5 bg-surface-1 px-2 py-1 font-mono text-xs text-[var(--color-text-primary)]"
                :placeholder="formatMessage(messages.addPlaceholder)"
                @keydown.enter="commitAdd"
                @blur="commitAdd"
                @keydown.escape="adding = false"
            />
        </div>
        <div v-if="expanded && expandable">
            <NbtTreeNode
                v-for="child in children()"
                :key="child.path.join('.')"
                :name="child.name"
                :tag="child.tag"
                :path="child.path"
                :depth="depth + 1"
                :read-only="readOnly"
                @edit="forwardEdit"
                @remove="emit('remove', $event)"
                @rename="forwardRename"
                @add="forwardAdd"
            />
        </div>
    </div>
</template>
