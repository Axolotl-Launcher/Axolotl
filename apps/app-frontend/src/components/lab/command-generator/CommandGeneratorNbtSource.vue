<script setup lang="ts">
import { ListOrderedIcon } from '@modrinth/assets'
import {
    attachSnbtCompletion,
    Button,
    CodeEditor,
    defineMessages,
    type Monaco,
    type MonacoEditor,
    type MonacoEditorOptions,
    registerSnbt,
    type SnbtCompletionHandle,
    useVIntl,
} from '@modrinth/ui'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { errorText } from '@/helpers/error-text'
import type { NbtLookup } from '@/lab/command-generator/nbt/catalog.ts'
import { createNbtCompletionSource } from '@/lab/command-generator/nbt/completion-source.ts'
import { compareVersion, type NbtDomain } from '@/lab/command-generator/nbt/schema.ts'
import { parseNbt, serializeNbtDocument } from '@/lab/command-generator/nbt/snbt.ts'

const props = defineProps<{
    modelValue?: string
    kind?: 'item' | 'entity' | 'generic'
    version?: string
    forceDataComponents?: boolean
    domain?: NbtDomain
    targetId?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    hint: {
        id: 'app.lab.command-generator.nbt.source.hint',
        defaultMessage: 'Tab completes a tag; Ctrl+S applies the command.',
    },
    compact: {
        id: 'app.lab.command-generator.nbt.source.compact',
        defaultMessage: 'Collapse to one line',
    },
    invalid: {
        id: 'app.lab.command-generator.nbt.source.invalid',
        defaultMessage: 'Syntax error: {message}',
    },
})

const options: MonacoEditorOptions = {
    minimap: { enabled: false },
    lineNumbers: 'on',
    fontSize: 13,
    tabSize: 4,
    insertSpaces: true,
    wordWrap: 'on',
    scrollBeyondLastLine: false,
    renderLineHighlight: 'none',
    overviewRulerLanes: 0,
    padding: { top: 8, bottom: 8 },
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
    suggestOnTriggerCharacters: true,
    quickSuggestions: { other: true, comments: false, strings: true },
    // Enter 换行、Tab 接受候选：手写时更接近普通编辑器。
    acceptSuggestionOnEnter: 'off',
    tabCompletion: 'on',
    fixedOverflowWidgets: true,
}

const version = computed(() => props.version ?? '1.21')
const text = ref(props.modelValue ?? '')
const error = ref('')

let completion: SnbtCompletionHandle | null = null
let lastEmitted = props.modelValue ?? ''
let timer: ReturnType<typeof setTimeout> | undefined

/** 与高级面板一致：物品的根形态由页面开关决定，文本以实际写法为准。 */
const useComponents = computed(() => {
    if (props.kind !== 'item') return false
    if (props.forceDataComponents !== undefined) return props.forceDataComponents
    return compareVersion(version.value, '1.20.5') >= 0
})

const components = computed(() => {
    if (props.kind !== 'item') return false
    const first = text.value.trimStart()[0]
    if (first === '[') return true
    if (first === '{') return false
    return useComponents.value
})

const domain = computed<NbtDomain | undefined>(() => {
    if (props.domain) return props.domain
    if (props.kind === 'item') return 'item'
    if (props.kind === 'entity') return 'entity'
    return undefined
})

const lookup = computed<NbtLookup | null>(() =>
    domain.value
        ? {
              domain: domain.value,
              version: version.value,
              targetId: props.targetId,
              components: components.value,
          }
        : null,
)

const source = createNbtCompletionSource(() => ({
    lookup: lookup.value,
    version: version.value,
    locale: locale.value,
    formatMessage: (descriptor) => formatMessage(descriptor),
}))

function onReady(editor: MonacoEditor, monaco: Monaco) {
    registerSnbt(monaco)
    const model = editor.getModel()
    if (model) completion = attachSnbtCompletion(editor, model, source)
}

/** 解析并压成一行；语法不完整时抛错，由调用方显示。 */
function compact(): string {
    const raw = text.value.trim()
    if (!raw) return ''
    const parsed = parseNbt(raw)
    return serializeNbtDocument(parsed.nodes, parsed.components)
}

function publish(next: string) {
    error.value = ''
    if (next === lastEmitted) return
    lastEmitted = next
    emit('update:modelValue', next)
}

function commit() {
    try {
        publish(compact())
    } catch (cause) {
        error.value = errorText(cause)
    }
}

/** 按钮：把编辑器里的排版也换掉，方便确认最终会发出去什么。 */
function collapse() {
    try {
        const next = compact()
        publish(next)
        text.value = next
    } catch (cause) {
        error.value = errorText(cause)
    }
}

watch(text, () => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(commit, 400)
})

watch(
    () => props.modelValue,
    (value) => {
        const next = value ?? ''
        if (next === lastEmitted) return
        lastEmitted = next
        text.value = next
    },
)

onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
    completion?.dispose()
})
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.hint) }}
            </span>
            <Button type="outlined" size="sm" @click="collapse">
                <ListOrderedIcon class="h-4 w-4" />
                {{ formatMessage(messages.compact) }}
            </Button>
        </div>
        <div class="h-72 overflow-hidden rounded-xl border border-solid border-surface-4">
            <CodeEditor
                :content="text"
                language="snbt"
                file-path="nbt.snbt"
                :options="options"
                @update:content="text = $event"
                @blur="commit"
                @save="commit"
                @ready="onReady"
            />
        </div>
        <p v-if="error" class="m-0 text-sm text-red">
            {{ formatMessage(messages.invalid, { message: error }) }}
        </p>
    </div>
</template>
