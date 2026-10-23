<script setup lang="ts">
import {
    Button,
    defineMessages,
    injectNotificationManager,
    LoadingIndicator,
    useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { writeFile } from '@tauri-apps/plugin-fs'
import { platform } from '@tauri-apps/plugin-os'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import { createSkinEditorTheme } from '@/components/lab/skin-editor/skin-editor-theme'

const { locale, formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const platformName = ref<string>()
const messages = defineMessages({
    title: { id: 'app.lab.skin-editor.title', defaultMessage: 'Skin editor' },
    loading: { id: 'app.lab.skin-editor.loading', defaultMessage: 'Loading skin editor' },
    loadErrorTitle: {
        id: 'app.lab.skin-editor.load-error-title',
        defaultMessage: 'Skin editor could not be loaded',
    },
    loadErrorDescription: {
        id: 'app.lab.skin-editor.load-error-description',
        defaultMessage: 'The embedded editor did not finish loading. Try again.',
    },
    retry: { id: 'app.lab.skin-editor.retry', defaultMessage: 'Try again' },
    exportSkin: { id: 'app.lab.skin-editor.export-skin', defaultMessage: 'Minecraft skin PNG' },
})

const blockbenchLocale = computed(() => {
    const normalized = locale.value.toLowerCase().replace('_', '-')
    if (normalized === 'zh-tw' || normalized === 'zh-hk') return 'zh_tw'
    if (normalized.startsWith('zh')) return 'zh'
    if (normalized === 'pt-br') return 'pt_br'
    return normalized.split('-')[0]
})

const editorState = ref<'loading' | 'ready' | 'error'>('loading')
const frameKey = ref(0)
let loadTimeout: number | undefined

const editorUrl = computed(() => {
    if (import.meta.env.DEV) {
        return `/__blockbench_skin__/index.html?embed=skin&lang=${encodeURIComponent(blockbenchLocale.value)}`
    }
    if (!platformName.value) return ''
    const baseUrl =
        platformName.value === 'windows'
            ? 'http://axolotl-skin.localhost'
            : 'axolotl-skin://localhost'
    return `${baseUrl}/index.html?embed=skin&lang=${encodeURIComponent(blockbenchLocale.value)}`
})

function clearLoadTimeout() {
    if (loadTimeout !== undefined) window.clearTimeout(loadTimeout)
    loadTimeout = undefined
}

function beginEditorLoad() {
    clearLoadTimeout()
    editorState.value = 'loading'
    loadTimeout = window.setTimeout(() => {
        editorState.value = 'error'
    }, 15_000)
}

function markEditorReady() {
    clearLoadTimeout()
    editorState.value = 'ready'
}

function markEditorError() {
    clearLoadTimeout()
    editorState.value = 'error'
}

async function reloadEditor() {
    beginEditorLoad()
    if (!editorUrl.value) {
        try {
            platformName.value = await platform()
        } catch (error) {
            markEditorError()
            handleError(error)
            return
        }
    }
    frameKey.value += 1
}

function sendThemeToEditor() {
    frame.value?.contentWindow?.postMessage(
        { type: 'axolotl-skin-theme', theme: createSkinEditorTheme() },
        '*',
    )
}

function handleFrameLoad() {
    sendThemeToEditor()
}

async function handleEditorMessage(event: MessageEvent<unknown>) {
    if (event.source !== frame.value?.contentWindow) return
    if (!event.data || typeof event.data !== 'object') return
    const message = event.data as { type?: unknown; name?: unknown; dataUrl?: unknown }
    if (message.type === 'axolotl-skin-theme-ready') {
        sendThemeToEditor()
        markEditorReady()
        return
    }
    if (
        message.type !== 'axolotl-skin-export' ||
        typeof message.name !== 'string' ||
        typeof message.dataUrl !== 'string'
    )
        return
    try {
        const path = await save({
            defaultPath: message.name,
            filters: [{ name: formatMessage(messages.exportSkin), extensions: ['png'] }],
        })
        if (!path) return
        const response = await fetch(message.dataUrl)
        if (!response.ok) throw new Error(`Failed to read exported skin: ${response.status}`)
        await writeFile(path, new Uint8Array(await response.arrayBuffer()))
    } catch (error) {
        handleError(error)
    }
}

const frame = ref<HTMLIFrameElement>()
let themeObserver: MutationObserver | undefined

watch(
    editorUrl,
    (url) => {
        if (url) beginEditorLoad()
    },
    { immediate: true },
)

onMounted(async () => {
    window.addEventListener('message', handleEditorMessage)
    themeObserver = new MutationObserver(sendThemeToEditor)
    themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class', 'style'],
    })
    if (!import.meta.env.DEV) {
        try {
            platformName.value = await platform()
        } catch (error) {
            markEditorError()
            handleError(error)
        }
    }
})
onUnmounted(() => {
    clearLoadTimeout()
    window.removeEventListener('message', handleEditorMessage)
    themeObserver?.disconnect()
})
</script>

<template>
    <main class="skin-editor-page relative flex h-full min-h-0 w-full flex-1 bg-surface-1">
        <h1 class="sr-only">{{ formatMessage(messages.title) }}</h1>
        <div
            v-if="editorState === 'loading'"
            class="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-surface-1 text-[var(--color-text-tertiary)]"
            role="status"
            aria-live="polite"
        >
            <LoadingIndicator />
            <p class="m-0">{{ formatMessage(messages.loading) }}</p>
        </div>
        <div
            v-else-if="editorState === 'error'"
            class="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-surface-1 p-6 text-center"
            role="alert"
        >
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.loadErrorTitle) }}
            </h2>
            <p class="m-0 max-w-md text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.loadErrorDescription) }}
            </p>
            <Button type="colored" color="brand" @click="reloadEditor">
                {{ formatMessage(messages.retry) }}
            </Button>
        </div>
        <iframe
            v-if="editorUrl"
            :key="`${frameKey}:${editorUrl}`"
            ref="frame"
            :title="formatMessage(messages.title)"
            :src="editorUrl"
            allow="clipboard-read; clipboard-write"
            class="h-full min-h-0 w-full flex-1 border-0 transition-opacity duration-150"
            :class="editorState === 'ready' ? 'opacity-100' : 'pointer-events-none opacity-0'"
            :aria-label="formatMessage(messages.title)"
            :aria-hidden="editorState !== 'ready'"
            @load="handleFrameLoad"
            @error="markEditorError"
        />
    </main>
</template>

<style>
.app-viewport:has(.skin-editor-page),
.app-viewport:has(.skin-editor-page) .page-transition-grid,
.app-viewport:has(.skin-editor-page) .page-transition-layer {
    height: 100%;
    min-height: 0;
    overflow: hidden;
}
</style>
