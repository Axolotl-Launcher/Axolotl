<script setup lang="ts">
import type * as Monaco from 'monaco-editor/editor/editor.api.js'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'

const props = defineProps<{
    content: string
    filePath?: string
    language?: string
    readOnly?: boolean
    options?: Monaco.editor.IStandaloneEditorConstructionOptions
}>()

const emit = defineEmits<{
    'update:content': [content: string]
    save: []
    blur: []
    ready: [editor: Monaco.editor.IStandaloneCodeEditor, runtime: typeof Monaco]
    error: [error: Error]
}>()

const messages = defineMessages({
    loading: {
        id: 'instance.files.studio.editor-loading',
        defaultMessage: 'Loading editor...',
    },
})

const { formatMessage } = useVIntl()
const editorElement = ref<HTMLElement | null>(null)
const loading = ref(true)

let monaco: typeof Monaco | null = null
let editor: Monaco.editor.IStandaloneCodeEditor | null = null
let model: Monaco.editor.ITextModel | null = null
let contentSubscription: Monaco.IDisposable | null = null
let resizeObserver: ResizeObserver | null = null
let themeObserver: MutationObserver | null = null
let applyingExternalContent = false
let disposed = false
const modelScope = crypto.randomUUID()

async function loadMonaco() {
    return (await import('./monaco-runtime')).monaco
}

function cssVariable(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

function applyTheme() {
    if (!monaco) return
    const isLight = document.documentElement.classList.contains('light-mode')
    monaco.editor.defineTheme('axolotl-studio', {
        base: isLight ? 'vs' : 'vs-dark',
        inherit: true,
        rules: [],
        colors: {
            'editor.background': cssVariable('--surface-2'),
            'editor.foreground': cssVariable('--color-text-default'),
            'editorGutter.background': cssVariable('--surface-2'),
            'editorLineNumber.foreground': cssVariable('--color-text-tertiary'),
            'editor.lineHighlightBackground': cssVariable('--surface-3'),
            'editorCursor.foreground': cssVariable('--color-brand'),
        },
    })
    monaco.editor.setTheme('axolotl-studio')
}

function createModel() {
    if (!monaco || !editor) return
    contentSubscription?.dispose()
    editor.setModel(null)
    model?.dispose()
    model = monaco.editor.createModel(
        props.content,
        props.language ?? 'plaintext',
        monaco.Uri.parse(
            `inmemory://axolotl/${modelScope}/${(props.filePath ?? 'document').split('/').map(encodeURIComponent).join('/')}`,
        ),
    )
    editor.setModel(model)
    contentSubscription = model.onDidChangeContent(() => {
        if (!applyingExternalContent) emit('update:content', model?.getValue() ?? '')
    })
}

onMounted(async () => {
    try {
        monaco = await loadMonaco()
        if (disposed) return
        applyTheme()

        if (!editorElement.value) return
        editor = monaco.editor.create(editorElement.value, {
            automaticLayout: false,
            fontSize: 14,
            fontLigatures: false,
            minimap: { enabled: true },
            padding: { top: 12 },
            readOnly: props.readOnly,
            renderWhitespace: 'selection',
            scrollBeyondLastLine: false,
            theme: 'axolotl-studio',
            ...props.options,
            model: null,
        })
        editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => emit('save'))
        editor.onDidBlurEditorWidget(() => emit('blur'))
        createModel()
        emit('ready', editor, monaco)

        resizeObserver = new ResizeObserver(() => editor?.layout())
        resizeObserver.observe(editorElement.value)
        themeObserver = new MutationObserver(applyTheme)
        themeObserver.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ['class'],
        })
        loading.value = false
    } catch (error) {
        loading.value = false
        emit('error', error instanceof Error ? error : new Error(String(error)))
    }
})
watch(
    () => props.filePath,
    () => createModel(),
)

watch(
    () => props.content,
    (content) => {
        if (!model || model.getValue() === content) return
        applyingExternalContent = true
        model.setValue(content)
        applyingExternalContent = false
    },
)

watch(
    () => props.language ?? 'plaintext',
    (language) => {
        if (monaco && model) monaco.editor.setModelLanguage(model, language ?? 'plaintext')
    },
)

watch(
    () => props.readOnly,
    (readOnly) => editor?.updateOptions({ readOnly }),
)

onBeforeUnmount(() => {
    disposed = true
    contentSubscription?.dispose()
    resizeObserver?.disconnect()
    themeObserver?.disconnect()
    editor?.dispose()
    model?.dispose()
})

async function formatDocument() {
    const currentEditor = editor
    const currentModel = model
    if (!currentEditor || !currentModel || props.readOnly) return
    const language = currentModel.getLanguageId()
    const { prepareFormatting } = await import('./monaco-runtime')
    await prepareFormatting(currentModel)
    if (
        disposed ||
        model !== currentModel ||
        currentModel.getLanguageId() !== language ||
        props.readOnly
    )
        return
    await currentEditor.getAction('editor.action.formatDocument')?.run()
}

watch(
    () => props.options,
    (options) => {
        if (options) editor?.updateOptions(options)
    },
    { deep: true },
)

function focus() {
    editor?.focus()
}
function getEditor() {
    return editor
}

defineExpose({ formatDocument, focus, getEditor })
</script>

<template>
    <div class="relative size-full min-h-0 min-w-0 bg-surface-2">
        <div
            v-if="loading"
            class="absolute inset-0 z-[1] flex items-center justify-center text-sm text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.loading) }}
        </div>
        <div ref="editorElement" class="size-full min-h-0 min-w-0" />
    </div>
</template>
