<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type * as Monaco from 'monaco-editor'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

declare global {
    interface Window {
        __axolotlMonacoRuntime?: Promise<typeof Monaco>
        require?: {
            config(config: Record<string, unknown>): void
            (dependencies: string[], callback: (monaco: typeof Monaco) => void): void
        }
    }
}

const props = defineProps<{
    content: string
    filePath: string
    language: string
    readOnly?: boolean
}>()

const emit = defineEmits<{
    'update:content': [content: string]
    save: []
    blur: []
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

function loadCodiconStyles() {
    if (document.querySelector('link[data-monaco-codicons]')) return
    const stylesheet = document.createElement('link')
    stylesheet.rel = 'stylesheet'
    stylesheet.href = '/launcher-preview/monaco/codicon/codicon.css'
    stylesheet.dataset.monacoCodicons = 'true'
    document.head.append(stylesheet)
}

function loadMonaco(): Promise<typeof Monaco> {
    if (window.__axolotlMonacoRuntime) return window.__axolotlMonacoRuntime

    window.__axolotlMonacoRuntime = new Promise((resolve, reject) => {
        loadCodiconStyles()
        const initialize = () => {
            const require = window.require
            if (!require) {
                reject(new Error('Monaco loader did not initialize'))
                return
            }
            require.config({ paths: { vs: '/launcher-preview/monaco/vs' } })
            require(['vs/editor/editor.main'], (loadedMonaco: typeof Monaco) =>
                resolve(loadedMonaco))
        }

        if (window.require) {
            initialize()
            return
        }

        const existingLoader = document.querySelector<HTMLScriptElement>(
            'script[data-monaco-loader]',
        )
        if (existingLoader) {
            existingLoader.addEventListener('load', initialize, { once: true })
            existingLoader.addEventListener(
                'error',
                () => reject(new Error('Failed to load Monaco editor')),
                { once: true },
            )
            return
        }

        const loader = document.createElement('script')
        loader.src = '/launcher-preview/monaco/vs/loader.js'
        loader.dataset.monacoLoader = 'true'
        loader.onload = initialize
        loader.onerror = () => reject(new Error('Failed to load Monaco editor'))
        document.head.append(loader)
    })

    return window.__axolotlMonacoRuntime
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

function registerStudioLanguages() {
    if (!monaco) return

    if (!monaco.languages.getLanguages().some(({ id }) => id === 'toml')) {
        monaco.languages.register({ id: 'toml', extensions: ['.toml'] })
        monaco.languages.setMonarchTokensProvider('toml', {
            tokenizer: {
                root: [
                    [/#.*/, 'comment'],
                    [/\[\[?.*?\]\]?/, 'type.identifier'],
                    [/^[\w.-]+(?=\s*=)/, 'key'],
                    [/"([^"\\]|\\.)*"/, 'string'],
                    [/'[^']*'/, 'string'],
                    [/\b(true|false)\b/, 'keyword'],
                    [/[-+]?\b\d+(\.\d+)?\b/, 'number'],
                ],
            },
        })
    }

    if (!monaco.languages.getLanguages().some(({ id }) => id === 'properties')) {
        monaco.languages.register({ id: 'properties', extensions: ['.properties'] })
        monaco.languages.setMonarchTokensProvider('properties', {
            tokenizer: {
                root: [
                    [/^[#!].*$/, 'comment'],
                    [/^[^\s:=]+(?=\s*[:=])/, 'key'],
                    [/[:=]/, 'delimiter'],
                    [/\\./, 'string.escape'],
                ],
            },
        })
    }

    if (!monaco.languages.getLanguages().some(({ id }) => id === 'snbt')) {
        monaco.languages.register({ id: 'snbt' })
        monaco.languages.setMonarchTokensProvider('snbt', {
            tokenizer: {
                root: [
                    [/\/\/.*$/, 'comment'],
                    [/[{}[\],:]/, 'delimiter'],
                    [/(?:true|false)\b/, 'keyword'],
                    [/-?(?:\d+\.?\d*|\.\d+)(?:[bBsSlLfFdD])?\b/, 'number'],
                    [/'(?:[^'\\]|\\.)*'/, 'string'],
                    [/"(?:[^"\\]|\\.)*"/, 'string'],
                    [/[A-Za-z0-9_.+-]+(?=\s*:)/, 'key'],
                ],
            },
        })
    }
}

function createModel() {
    if (!monaco || !editor) return
    contentSubscription?.dispose()
    model?.dispose()
    model = monaco.editor.createModel(
        props.content,
        props.language,
        monaco.Uri.parse(
            `axolotl-instance://studio/${props.filePath.split('/').map(encodeURIComponent).join('/')}`,
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
    } catch (error) {
        loading.value = false
        console.error('Failed to load Monaco editor', error)
        return
    }
    if (disposed) return
    registerStudioLanguages()
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
    })
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => emit('save'))
    editor.onDidBlurEditorWidget(() => emit('blur'))
    createModel()

    resizeObserver = new ResizeObserver(() => editor?.layout())
    resizeObserver.observe(editorElement.value)
    themeObserver = new MutationObserver(applyTheme)
    themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class'],
    })
    loading.value = false
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
    () => props.language,
    (language) => {
        if (monaco && model) monaco.editor.setModelLanguage(model, language)
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
    await editor?.getAction('editor.action.formatDocument')?.run()
}

defineExpose({ formatDocument })
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
