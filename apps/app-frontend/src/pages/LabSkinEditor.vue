<script setup lang="ts">
import { SaveIcon, SettingsIcon } from '@modrinth/assets'
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
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'

import { createSkinEditorTheme } from '@/components/lab/skin-editor/skin-editor-theme'
import SkinEditorRecoveryModal from '@/components/lab/skin-editor/SkinEditorRecoveryModal.vue'
import SkinEditorSettingsModal from '@/components/lab/skin-editor/SkinEditorSettingsModal.vue'
import SkinEditorUnsavedChangesModal from '@/components/lab/skin-editor/SkinEditorUnsavedChangesModal.vue'
import {
    createSkinEditorRequestId,
    loadSkinEditorPreferences,
    normalizeExportFilename,
    parseSkinEditorMessage,
    saveSkinEditorPreferences,
    shouldPromptBeforeLeave,
    type SkinEditorAutoSaveStatus,
    type SkinEditorEditorMessage,
    type SkinEditorParentMessage,
    type SkinEditorPreferences,
} from '@/lab/skin-editor/session'

const { locale, formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const platformName = ref<string>()
const preferences = ref<SkinEditorPreferences>(loadSkinEditorPreferences())
const settingsModal = shallowRef<InstanceType<typeof SkinEditorSettingsModal>>()
const recoveryModal = shallowRef<InstanceType<typeof SkinEditorRecoveryModal>>()
const unsavedModal = shallowRef<InstanceType<typeof SkinEditorUnsavedChangesModal>>()
const dirty = ref(false)
const autoSaveStatus = ref<SkinEditorAutoSaveStatus>('idle')
const lastSavedAt = ref<number>()
const saveInProgress = ref(false)

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
    save: { id: 'app.lab.skin-editor.save', defaultMessage: 'Save PNG' },
    autoSaveOff: {
        id: 'app.lab.skin-editor.auto-save.off',
        defaultMessage: 'Auto-save off',
    },
    autoSaveSaving: {
        id: 'app.lab.skin-editor.auto-save.saving',
        defaultMessage: 'Saving draft…',
    },
    autoSaveSaved: {
        id: 'app.lab.skin-editor.auto-save.saved',
        defaultMessage: 'Draft saved {time}',
    },
    autoSaveError: {
        id: 'app.lab.skin-editor.auto-save.error',
        defaultMessage: 'Auto-save failed',
    },
    unsaved: {
        id: 'app.lab.skin-editor.auto-save.unsaved',
        defaultMessage: 'Unsaved changes',
    },
    upToDate: {
        id: 'app.lab.skin-editor.auto-save.up-to-date',
        defaultMessage: 'Up to date',
    },
    discardDraftFailed: {
        id: 'app.lab.skin-editor.discard-draft-failed',
        defaultMessage:
            'The recovery draft could not be discarded. Stay on this page and try again.',
    },
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

const autoSaveLabel = computed(() => {
    if (!preferences.value.autoSaveEnabled) return formatMessage(messages.autoSaveOff)
    if (autoSaveStatus.value === 'saving') return formatMessage(messages.autoSaveSaving)
    if (autoSaveStatus.value === 'error') return formatMessage(messages.autoSaveError)
    if (autoSaveStatus.value === 'saved' && lastSavedAt.value) {
        return formatMessage(messages.autoSaveSaved, {
            time: new Intl.DateTimeFormat(locale.value, {
                hour: '2-digit',
                minute: '2-digit',
            }).format(new Date(lastSavedAt.value)),
        })
    }
    if (dirty.value) return formatMessage(messages.unsaved)
    if (lastSavedAt.value) {
        return formatMessage(messages.autoSaveSaved, {
            time: new Intl.DateTimeFormat(locale.value, {
                hour: '2-digit',
                minute: '2-digit',
            }).format(new Date(lastSavedAt.value)),
        })
    }
    return formatMessage(messages.upToDate)
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
    autoSaveStatus.value = 'idle'
    lastSavedAt.value = undefined
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
    })
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
    const result = await unsavedModal.value?.prompt()
    if (result === 'stay' || !result) return false
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
        sendThemeToEditor()
        sendConfigToEditor()
        markEditorReady()
        if (message.recovery) recoveryModal.value?.show(message.recovery)
        return
    }

    if (message.type === 'axolotl-skin-state') {
        dirty.value = message.dirty
        autoSaveStatus.value = message.autoSave
        lastSavedAt.value = message.lastSavedAt
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
    }
}

function openSettings() {
    settingsModal.value?.show()
}

function saveSettings(next: SkinEditorPreferences) {
    preferences.value = saveSkinEditorPreferences(next)
    sendConfigToEditor()
}

async function continueRecovery() {
    await sendCommand('restore-draft')
}

async function startNewFromRecovery() {
    await sendCommand('start-new')
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
        <div
            v-if="editorState === 'ready'"
            class="flex h-10 shrink-0 items-center justify-end gap-2 border-0 border-b border-solid border-surface-4 bg-surface-2 px-3"
            data-testid="skin-editor-controls"
        >
            <span
                class="whitespace-nowrap text-xs font-medium text-[var(--color-text-tertiary)]"
                :role="autoSaveStatus === 'error' ? 'alert' : 'status'"
                :aria-live="autoSaveStatus === 'error' ? 'assertive' : 'polite'"
            >
                {{ autoSaveLabel }}
            </span>
            <Button
                v-tooltip="formatMessage(messages.save)"
                type="quiet"
                size="sm"
                icon-only
                circular
                :label="formatMessage(messages.save)"
                :disabled="saveInProgress"
                @click="sendCommand('request-export', 'manual')"
            >
                <SaveIcon aria-hidden="true" />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.settings)"
                type="quiet"
                size="sm"
                icon-only
                circular
                :label="formatMessage(messages.settings)"
                @click="openSettings"
            >
                <SettingsIcon aria-hidden="true" />
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

        <SkinEditorSettingsModal
            ref="settingsModal"
            :preferences="preferences"
            @save="saveSettings"
        />
        <SkinEditorRecoveryModal
            ref="recoveryModal"
            @continue="continueRecovery"
            @start-new="startNewFromRecovery"
        />
        <SkinEditorUnsavedChangesModal ref="unsavedModal" />
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
