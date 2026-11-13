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
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'

import { createSkinEditorTheme } from '@/components/lab/skin-editor/skin-editor-theme'
import {
    createSkinEditorRequestId,
    loadSkinEditorPreferences,
    normalizeExportFilename,
    parseSkinEditorMessage,
    saveSkinEditorPreferences,
    shouldPromptBeforeLeave,
    type SkinEditorDialogLabels,
    type SkinEditorEditorMessage,
    type SkinEditorParentMessage,
    type SkinEditorPreferences,
} from '@/lab/skin-editor/session'

const { locale, formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const platformName = ref<string>()
const preferences = ref<SkinEditorPreferences>(loadSkinEditorPreferences())
const dirty = ref(false)
const saveInProgress = ref(false)
const recoveryUpdatedAt = ref<number>()

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
    settings: {
        id: 'app.lab.skin-editor.settings.tooltip',
        defaultMessage: 'Skin editor settings',
    },
    discardDraftFailed: {
        id: 'app.lab.skin-editor.discard-draft-failed',
        defaultMessage:
            'The recovery draft could not be discarded. Stay on this page and try again.',
    },
    settingsTitle: { id: 'app.lab.skin-editor.settings.title', defaultMessage: 'Editor settings' },
    autoSave: {
        id: 'app.lab.skin-editor.settings.auto-save',
        defaultMessage: 'Automatically save a recovery draft',
    },
    autoSaveDescription: {
        id: 'app.lab.skin-editor.settings.auto-save-description',
        defaultMessage:
            'Recovery drafts stay on this device and are separate from exported PNG files.',
    },
    interval: {
        id: 'app.lab.skin-editor.settings.interval',
        defaultMessage: 'Save interval (seconds)',
    },
    intervalDescription: {
        id: 'app.lab.skin-editor.settings.interval-description',
        defaultMessage: 'Use a value between 5 and 600 seconds.',
    },
    warnBeforeLeave: {
        id: 'app.lab.skin-editor.settings.warn-before-leave',
        defaultMessage: 'Warn before leaving with unsaved changes',
    },
    warnBeforeLeaveDescription: {
        id: 'app.lab.skin-editor.settings.warn-before-leave-description',
        defaultMessage:
            'When auto-save is off, this warning is always enabled to protect your work.',
    },
    recoveryTitle: {
        id: 'app.lab.skin-editor.recovery.title',
        defaultMessage: 'Continue last skin?',
    },
    recoveryBody: {
        id: 'app.lab.skin-editor.recovery.body',
        defaultMessage: 'A recovery draft was saved on {date}.',
    },
    recoveryStartNew: {
        id: 'app.lab.skin-editor.recovery.start-new',
        defaultMessage: 'Start new',
    },
    recoveryContinue: {
        id: 'app.lab.skin-editor.recovery.continue',
        defaultMessage: 'Continue',
    },
    unsavedTitle: { id: 'app.lab.skin-editor.unsaved.title', defaultMessage: 'Unsaved skin' },
    unsavedBody: {
        id: 'app.lab.skin-editor.unsaved.body',
        defaultMessage: 'Save your skin before leaving, or discard the current changes.',
    },
    unsavedStay: { id: 'app.lab.skin-editor.unsaved.stay', defaultMessage: 'Stay' },
    unsavedDiscard: { id: 'app.lab.skin-editor.unsaved.discard', defaultMessage: 'Discard' },
    unsavedSave: { id: 'app.lab.skin-editor.unsaved.save', defaultMessage: 'Save PNG' },
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
    dirty.value = false
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

function postToEditor(message: SkinEditorParentMessage) {
    frame.value?.contentWindow?.postMessage(message, '*')
}

function sendThemeToEditor() {
    postToEditor({ type: 'axolotl-skin-theme', theme: createSkinEditorTheme() })
}

function sendConfigToEditor() {
    postToEditor({
        type: 'axolotl-skin-config',
        autoSaveEnabled: preferences.value.autoSaveEnabled,
        autoSaveIntervalSeconds: preferences.value.autoSaveIntervalSeconds,
        warnBeforeLeave: preferences.value.warnBeforeLeave,
    })
}

function formatRecoveryDate(updatedAt: number) {
    return new Intl.DateTimeFormat(locale.value, {
        dateStyle: 'medium',
        timeStyle: 'short',
    }).format(new Date(updatedAt))
}

function buildDialogLabels(): SkinEditorDialogLabels {
    return {
        settingsLabel: formatMessage(messages.settings),
        settingsTitle: formatMessage(messages.settingsTitle),
        autoSave: formatMessage(messages.autoSave),
        autoSaveDescription: formatMessage(messages.autoSaveDescription),
        interval: formatMessage(messages.interval),
        intervalDescription: formatMessage(messages.intervalDescription),
        warnBeforeLeave: formatMessage(messages.warnBeforeLeave),
        warnBeforeLeaveDescription: formatMessage(messages.warnBeforeLeaveDescription),
        recoveryTitle: formatMessage(messages.recoveryTitle),
        recoveryBody: formatMessage(messages.recoveryBody, {
            date:
                recoveryUpdatedAt.value !== undefined
                    ? formatRecoveryDate(recoveryUpdatedAt.value)
                    : '',
        }),
        recoveryStartNew: formatMessage(messages.recoveryStartNew),
        recoveryContinue: formatMessage(messages.recoveryContinue),
        unsavedTitle: formatMessage(messages.unsavedTitle),
        unsavedBody: formatMessage(messages.unsavedBody),
        unsavedStay: formatMessage(messages.unsavedStay),
        unsavedDiscard: formatMessage(messages.unsavedDiscard),
        unsavedSave: formatMessage(messages.unsavedSave),
    }
}

function sendLabelsToEditor() {
    postToEditor({ type: 'axolotl-skin-labels', ...buildDialogLabels() })
}

function handleFrameLoad() {
    sendThemeToEditor()
}

const commandResolvers = new Map<string, { resolve: (ok: boolean) => void; timeout?: number }>()
let activeSkinAction: 'route' | 'new-skin' | null = null
let activeSkinActionPromise: Promise<boolean> | null = null
let activeSkinActionResolve: ((allowed: boolean) => void) | null = null

function sendCommand(
    command: 'flush-draft' | 'discard-draft' | 'restore-draft' | 'start-new' | 'request-export',
    reason?: 'manual' | 'leave',
): Promise<boolean> {
    const requestId = createSkinEditorRequestId()
    return new Promise((resolve) => {
        const timeout = window.setTimeout(() => {
            commandResolvers.delete(requestId)
            resolve(false)
        }, 15_000)
        commandResolvers.set(requestId, {
            resolve: (ok) => {
                window.clearTimeout(timeout)
                commandResolvers.delete(requestId)
                resolve(ok)
            },
            timeout,
        })
        postToEditor({ type: 'axolotl-skin-command', requestId, command, reason })
    })
}

function acknowledgeExportRequest(requestId: string) {
    const resolver = commandResolvers.get(requestId)
    if (!resolver || resolver.timeout === undefined) return
    window.clearTimeout(resolver.timeout)
    resolver.timeout = undefined
}

async function writeExportedSkin(name: string, dataUrl: string): Promise<boolean> {
    const path = await save({
        defaultPath: normalizeExportFilename(name),
        filters: [{ name: formatMessage(messages.exportSkin), extensions: ['png'] }],
    })
    if (!path) return false

    const response = await fetch(dataUrl)
    if (!response.ok) throw new Error(`Failed to read exported skin: ${response.status}`)
    await writeFile(path, new Uint8Array(await response.arrayBuffer()))
    return true
}

async function handleExportRequest(
    message: Extract<SkinEditorEditorMessage, { type: 'axolotl-skin-export-request' }>,
) {
    if (saveInProgress.value) {
        postToEditor({
            type: 'axolotl-skin-export-accepted',
            requestId: message.requestId,
            accepted: false,
        })
        postToEditor({
            type: 'axolotl-skin-export-result',
            requestId: message.requestId,
            saved: false,
        })
        return
    }

    saveInProgress.value = true
    let saved = false
    try {
        acknowledgeExportRequest(message.requestId)
        postToEditor({
            type: 'axolotl-skin-export-accepted',
            requestId: message.requestId,
            accepted: true,
        })
        saved = await writeExportedSkin(message.name, message.dataUrl)
    } catch (error) {
        handleError(error)
    } finally {
        saveInProgress.value = false
        postToEditor({
            type: 'axolotl-skin-export-result',
            requestId: message.requestId,
            saved,
        })
    }
}

const UNSAVED_PROMPT_TIMEOUT_MS = 5 * 60 * 1000
const unsavedPromptResolvers = new Map<string, (choice: 'stay' | 'discard' | 'save') => void>()

function requestUnsavedChoice(): Promise<'stay' | 'discard' | 'save'> {
    if (editorState.value !== 'ready') return Promise.resolve('stay')
    const requestId = createSkinEditorRequestId()
    return new Promise((resolve) => {
        const finish = (choice: 'stay' | 'discard' | 'save') => {
            unsavedPromptResolvers.delete(requestId)
            resolve(choice)
        }
        const timeout = window.setTimeout(() => finish('stay'), UNSAVED_PROMPT_TIMEOUT_MS)
        unsavedPromptResolvers.set(requestId, (choice) => {
            window.clearTimeout(timeout)
            finish(choice)
        })
        postToEditor({ type: 'axolotl-skin-prompt-unsaved', requestId })
    })
}

async function confirmUnsavedChanges(): Promise<boolean> {
    if (activeSkinActionPromise) return await activeSkinActionPromise
    try {
        return await promptForUnsavedChanges()
    } catch (error) {
        handleError(error)
        return false
    }
}

async function promptForUnsavedChanges(): Promise<boolean> {
    const result = await requestUnsavedChoice()
    if (result === 'stay') return false
    if (result === 'discard') {
        const discarded = await sendCommand('discard-draft')
        if (!discarded) handleError(new Error(formatMessage(messages.discardDraftFailed)))
        return discarded
    }
    return await sendCommand('request-export', 'leave')
}

async function handleNewSkinRequest() {
    if (activeSkinAction) return
    activeSkinAction = 'new-skin'
    try {
        activeSkinActionPromise = new Promise((resolve) => {
            activeSkinActionResolve = resolve
        })
        const allowed = await promptForUnsavedChanges()
        activeSkinActionResolve?.(allowed)
        if (allowed) {
            const started = await sendCommand('start-new')
            if (started) dirty.value = false
        }
    } finally {
        activeSkinActionResolve?.(false)
        activeSkinActionPromise = null
        activeSkinActionResolve = null
        activeSkinAction = null
    }
}

function handleCommandResult(requestId: string, ok: boolean) {
    commandResolvers.get(requestId)?.resolve(ok)
}

async function handleEditorMessage(event: MessageEvent<unknown>) {
    if (event.source !== frame.value?.contentWindow) return
    const message = parseSkinEditorMessage(event.data)
    if (!message) {
        const raw = event.data as { type?: unknown; requestId?: unknown }
        if (raw?.type === 'axolotl-skin-export-request' && typeof raw.requestId === 'string') {
            postToEditor({
                type: 'axolotl-skin-export-accepted',
                requestId: raw.requestId,
                accepted: false,
            })
            postToEditor({
                type: 'axolotl-skin-export-result',
                requestId: raw.requestId,
                saved: false,
            })
        }
        return
    }

    if (message.type === 'axolotl-skin-theme-ready') {
        recoveryUpdatedAt.value = message.recovery?.updatedAt
        sendThemeToEditor()
        sendConfigToEditor()
        sendLabelsToEditor()
        markEditorReady()
        return
    }

    if (message.type === 'axolotl-skin-state') {
        dirty.value = message.dirty
        return
    }

    if (message.type === 'axolotl-skin-export-request') {
        await handleExportRequest(message)
        return
    }

    if (message.type === 'axolotl-skin-command-result') {
        handleCommandResult(message.requestId, message.ok)
        return
    }

    if (message.type === 'axolotl-skin-new-skin-request') {
        await handleNewSkinRequest()
        return
    }

    if (message.type === 'axolotl-skin-settings-change') {
        try {
            preferences.value = saveSkinEditorPreferences({
                autoSaveEnabled: message.autoSaveEnabled,
                autoSaveIntervalSeconds: message.autoSaveIntervalSeconds,
                warnBeforeLeave: message.warnBeforeLeave,
            })
        } catch (error) {
            handleError(error)
        }
        sendConfigToEditor()
        return
    }

    if (message.type === 'axolotl-skin-prompt-result') {
        unsavedPromptResolvers.get(message.requestId)?.(message.choice)
    }
}

function handleBeforeUnload(event: BeforeUnloadEvent) {
    if (!dirty.value) return
    event.preventDefault()
    event.returnValue = ''
}

onBeforeRouteLeave(async () => {
    if (!dirty.value) return true
    if (activeSkinAction === 'new-skin' && activeSkinActionPromise) {
        const allowed = await activeSkinActionPromise
        if (!allowed) return false
        if (!dirty.value) return true
    }
    if (activeSkinAction) return false
    activeSkinAction = 'route'
    try {
        if (!shouldPromptBeforeLeave(preferences.value, dirty.value)) {
            if (await sendCommand('flush-draft')) return true
        }
        return await confirmUnsavedChanges()
    } finally {
        activeSkinAction = null
    }
})

const frame = ref<HTMLIFrameElement>()
let themeObserver: MutationObserver | undefined

watch(
    editorUrl,
    (url) => {
        if (url) beginEditorLoad()
    },
    { immediate: true },
)

watch(locale, sendLabelsToEditor)

onMounted(async () => {
    window.addEventListener('message', handleEditorMessage)
    window.addEventListener('beforeunload', handleBeforeUnload)
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

onBeforeUnmount(() => {
    clearLoadTimeout()
    window.removeEventListener('message', handleEditorMessage)
    window.removeEventListener('beforeunload', handleBeforeUnload)
    themeObserver?.disconnect()
    for (const { resolve, timeout } of commandResolvers.values()) {
        if (timeout !== undefined) window.clearTimeout(timeout)
        resolve(false)
    }
    commandResolvers.clear()
    for (const resolve of [...unsavedPromptResolvers.values()]) resolve('stay')
    unsavedPromptResolvers.clear()
})
</script>

<template>
    <main class="skin-editor-page relative flex h-full min-h-0 w-full flex-1 flex-col bg-surface-1">
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
            class="min-h-0 w-full flex-1 border-0 transition-opacity duration-150"
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
