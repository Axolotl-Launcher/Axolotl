<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import LoadingIndicator from '@modrinth/ui/src/components/base/LoadingIndicator.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import { injectPreviewWindow } from '../../lib/window'
import { skinEditorMessages as messages } from './skin-editor/messages'
import { createSkinEditorTheme } from './skin-editor/skin-editor-theme'

/**
 * Port of `apps/app-frontend/src/pages/LabSkinEditor.vue`. The editor is the
 * launcher's own Blockbench skin build, served from the site instead of the
 * app's custom protocol; exporting downloads the PNG where the app opens its
 * save dialog.
 */
const { locale, formatMessage } = useVIntl()
const { root } = injectPreviewWindow()

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

const editorUrl = computed(
    () =>
        `/launcher-preview/blockbench-skin/index.html?embed=skin&lang=${encodeURIComponent(blockbenchLocale.value)}`,
)

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

function reloadEditor() {
    beginEditorLoad()
    frameKey.value += 1
}

const frame = ref<HTMLIFrameElement>()

function sendThemeToEditor() {
    if (!root.value) return
    frame.value?.contentWindow?.postMessage(
        { type: 'axolotl-skin-theme', theme: createSkinEditorTheme(root.value) },
        '*',
    )
}

function handleFrameLoad() {
    sendThemeToEditor()
}

function handleEditorMessage(event: MessageEvent<unknown>) {
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
    const link = document.createElement('a')
    link.href = message.dataUrl
    link.download = message.name
    link.click()
}

let themeObserver: MutationObserver | undefined

watch(
    editorUrl,
    (url) => {
        if (url) beginEditorLoad()
    },
    { immediate: true },
)

onMounted(() => {
    window.addEventListener('message', handleEditorMessage)
    themeObserver = new MutationObserver(sendThemeToEditor)
    if (root.value) {
        themeObserver.observe(root.value, { attributes: true, attributeFilter: ['class', 'style'] })
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
.launcher-preview-window .app-viewport:has(.skin-editor-page),
.launcher-preview-window .app-viewport:has(.skin-editor-page) .page-transition-grid,
.launcher-preview-window .app-viewport:has(.skin-editor-page) .page-transition-layer {
    height: 100%;
    min-height: 0;
    overflow: hidden;
}
</style>
