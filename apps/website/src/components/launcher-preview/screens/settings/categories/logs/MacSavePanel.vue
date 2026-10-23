<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, reactive, ref } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'
import { injectPreviewWindow } from '../../../../lib/window'
import { messages } from '../../messages'
import { favoritePath, FINDER_FAVORITES, type FinderFavorite } from './finder-model'

/**
 * The save panel behind `save()` of @tauri-apps/plugin-dialog: on macOS an
 * NSSavePanel sheet hanging from the top of the launcher window, which blocks
 * the window until it is answered. It proposes the caller's file name, keeps
 * the extension of the ZIP filter, and opens where the last save went
 * (Downloads the first time).
 */
const props = withDefaults(defineProps<{ extension?: string }>(), { extension: 'zip' })

const { formatMessage } = useVIntl()
const { layer } = injectPreviewWindow()
const memory = usePreviewDomain('settings:save-panel', () => ({
    value: reactive({ folder: 'Downloads' as FinderFavorite }),
}))

const open = ref(false)
const fileName = ref('')
const tags = ref('')
const folder = ref<FinderFavorite>(memory.folder)
const menuOpen = ref(false)
const nameInput = ref<HTMLInputElement | null>(null)
let resolver: ((path: string | null) => void) | undefined

const folderLabels: Record<FinderFavorite, MessageDescriptor> = {
    Desktop: messages.launcherPreviewSettingsFinderDesktop,
    Documents: messages.launcherPreviewSettingsFinderDocuments,
    Downloads: messages.launcherPreviewSettingsFinderDownloads,
}

const folderIcons = {
    Desktop: MonitorIcon,
    Documents: FileTextIcon,
    Downloads: DownloadIcon,
}

const canSave = computed(() => fileName.value.trim().length > 0)

function settle(path: string | null) {
    if (!open.value) return
    open.value = false
    menuOpen.value = false
    window.removeEventListener('keydown', onKeydown, true)
    const resolve = resolver
    resolver = undefined
    resolve?.(path)
}

function save() {
    let name = fileName.value.trim()
    if (!name) return
    // The panel enforces the filter's extension.
    if (!name.toLowerCase().endsWith(`.${props.extension}`)) name = `${name}.${props.extension}`
    memory.folder = folder.value
    settle(`${favoritePath(folder.value)}/${name}`)
}

function cancel() {
    settle(null)
}

function chooseFolder(next: FinderFavorite) {
    folder.value = next
    menuOpen.value = false
}

/** A click anywhere else in the sheet dismisses the open folder menu. */
function onPanelMouseDown(event: MouseEvent) {
    const target = event.target as HTMLElement | null
    if (menuOpen.value && !target?.closest('.settings-save-panel-where')) menuOpen.value = false
}

function onKeydown(event: KeyboardEvent) {
    if (!open.value) return
    if (event.key === 'Escape') {
        // The sheet answers Escape; the launcher's own modal underneath must not.
        event.preventDefault()
        event.stopPropagation()
        if (menuOpen.value) menuOpen.value = false
        else cancel()
    } else if (event.key === 'Enter' && !menuOpen.value) {
        event.preventDefault()
        event.stopPropagation()
        save()
    }
}

/** Shows the sheet; resolves with the chosen path, or null when cancelled. */
function request(defaultName: string): Promise<string | null> {
    settle(null)
    return new Promise((resolve) => {
        resolver = resolve
        fileName.value = defaultName
        tags.value = ''
        folder.value = memory.folder
        menuOpen.value = false
        open.value = true
        window.addEventListener('keydown', onKeydown, true)
        void nextTick(() => {
            const input = nameInput.value
            if (!input) return
            input.focus()
            // Like the system panel, the name is selected without its extension.
            const dot = defaultName.lastIndexOf('.')
            input.setSelectionRange(0, dot > 0 ? dot : defaultName.length)
        })
    })
}

onBeforeUnmount(() => settle(null))

defineExpose({ request })
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <Transition name="settings-save-panel">
            <div v-if="open" class="settings-save-panel-backdrop" @mousedown.self.prevent>
                <div
                    role="dialog"
                    aria-modal="true"
                    class="settings-save-panel"
                    @mousedown="onPanelMouseDown"
                >
                    <label class="settings-save-panel-row">
                        <span>{{
                            formatMessage(messages.launcherPreviewSettingsSavePanelSaveAs)
                        }}</span>
                        <input
                            ref="nameInput"
                            v-model="fileName"
                            class="settings-save-panel-field"
                            type="text"
                            spellcheck="false"
                            autocomplete="off"
                        />
                    </label>
                    <label class="settings-save-panel-row">
                        <span>{{
                            formatMessage(messages.launcherPreviewSettingsSavePanelTags)
                        }}</span>
                        <input
                            v-model="tags"
                            class="settings-save-panel-field"
                            type="text"
                            spellcheck="false"
                            autocomplete="off"
                        />
                    </label>
                    <div class="settings-save-panel-row">
                        <span>{{
                            formatMessage(messages.launcherPreviewSettingsSavePanelWhere)
                        }}</span>
                        <div class="settings-save-panel-where">
                            <button
                                type="button"
                                class="settings-save-panel-popup"
                                aria-haspopup="listbox"
                                :aria-expanded="menuOpen"
                                @click="menuOpen = !menuOpen"
                            >
                                <component :is="folderIcons[folder]" aria-hidden="true" />
                                <span>{{ formatMessage(folderLabels[folder]) }}</span>
                                <ChevronDownIcon
                                    class="settings-save-panel-popup-chevron"
                                    aria-hidden="true"
                                />
                            </button>
                            <div v-if="menuOpen" role="listbox" class="settings-save-panel-menu">
                                <button
                                    v-for="option in FINDER_FAVORITES"
                                    :key="option"
                                    type="button"
                                    role="option"
                                    class="settings-save-panel-option"
                                    :aria-selected="option === folder"
                                    @click="chooseFolder(option)"
                                >
                                    <CheckIcon
                                        class="settings-save-panel-check"
                                        :class="{ 'is-hidden': option !== folder }"
                                        aria-hidden="true"
                                    />
                                    <component :is="folderIcons[option]" aria-hidden="true" />
                                    <span>{{ formatMessage(folderLabels[option]) }}</span>
                                </button>
                            </div>
                        </div>
                    </div>
                    <div class="settings-save-panel-buttons">
                        <button type="button" class="settings-save-panel-button" @click="cancel">
                            {{ formatMessage(messages.launcherPreviewSettingsSavePanelCancel) }}
                        </button>
                        <button
                            type="button"
                            class="settings-save-panel-button is-default"
                            :disabled="!canSave"
                            @click="save"
                        >
                            {{ formatMessage(messages.launcherPreviewSettingsSavePanelSave) }}
                        </button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
/*
 * macOS system chrome (NSSavePanel as a window sheet). The desktop app gets
 * this panel from the operating system, so it uses the system's colours rather
 * than the launcher's theme tokens.
 */
.settings-save-panel-backdrop {
    position: absolute;
    inset: 0;
    z-index: 400;
    font-family:
        -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', system-ui, sans-serif;
    -webkit-font-smoothing: antialiased;
}

.settings-save-panel {
    position: absolute;
    top: 48px;
    left: 50%;
    display: flex;
    width: 440px;
    flex-direction: column;
    gap: 10px;
    padding: 20px 20px 16px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 0 0 12px 12px;
    background: rgb(42 42 44 / 97%);
    color: rgb(255 255 255 / 92%);
    font-size: 13px;
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 60%),
        0 22px 70px 4px rgb(0 0 0 / 56%);
    transform: translateX(-50%);
}

.launcher-preview-window.light-mode .settings-save-panel {
    border-color: rgb(0 0 0 / 12%);
    background: rgb(236 236 236 / 97%);
    color: rgb(0 0 0 / 85%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 18%),
        0 22px 70px 4px rgb(0 0 0 / 28%);
}

.settings-save-panel-row {
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
}

.settings-save-panel-row > span:first-child {
    text-align: right;
}

/* The website's global `input` rules (36px minimum, brand focus shadow, a
   transparent `!important` outline) do not belong to the system panel, so the
   field draws its own focus ring with a shadow. */
.settings-save-panel-field {
    height: 22px;
    min-height: 0;
    min-width: 0;
    padding: 0 6px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 5px;
    background: rgb(255 255 255 / 8%);
    color: inherit;
    font: inherit;
    box-shadow: none;
    transition: none;
}

.launcher-preview-window.light-mode .settings-save-panel-field {
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
}

.settings-save-panel-field:focus {
    color: inherit;
    box-shadow: 0 0 0 3px rgb(10 132 255 / 55%);
}

.settings-save-panel-button:focus-visible,
.settings-save-panel-popup:focus-visible {
    outline: 3px solid rgb(10 132 255 / 55%);
    outline-offset: 0;
}

.settings-save-panel-field::selection {
    background: rgb(10 132 255 / 55%);
}

.settings-save-panel-where {
    position: relative;
    width: 230px;
}

.settings-save-panel-popup {
    display: flex;
    width: 100%;
    height: 22px;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 5px;
    background: rgb(255 255 255 / 16%);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
    box-shadow: 0 0.5px 1px rgb(0 0 0 / 20%);
}

.launcher-preview-window.light-mode .settings-save-panel-popup {
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
}

.settings-save-panel-popup svg,
.settings-save-panel-option svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: #0a84ff;
}

.settings-save-panel-popup .settings-save-panel-popup-chevron {
    margin-left: auto;
    color: currentColor;
    opacity: 0.55;
}

.settings-save-panel-menu {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    z-index: 1;
    display: flex;
    width: 100%;
    flex-direction: column;
    padding: 5px;
    border: 0.5px solid rgb(255 255 255 / 16%);
    border-radius: 8px;
    background: rgb(50 50 52 / 98%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 50%),
        0 10px 30px rgb(0 0 0 / 45%);
}

.launcher-preview-window.light-mode .settings-save-panel-menu {
    border-color: rgb(0 0 0 / 10%);
    background: rgb(246 246 246 / 98%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 12%),
        0 10px 30px rgb(0 0 0 / 18%);
}

.settings-save-panel-option {
    display: flex;
    height: 22px;
    align-items: center;
    gap: 6px;
    padding: 0 8px 0 4px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
}

.settings-save-panel-option:hover {
    background: #0a84ff;
    color: #fff;
}

.settings-save-panel-option:hover svg {
    color: #fff;
}

.settings-save-panel-option .settings-save-panel-check {
    width: 12px;
    height: 12px;
    color: currentColor;
}

.settings-save-panel-check.is-hidden {
    visibility: hidden;
}

.settings-save-panel-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
}

.settings-save-panel-button {
    min-width: 76px;
    height: 28px;
    padding: 0 12px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 6px;
    background: rgb(255 255 255 / 16%);
    color: inherit;
    font: inherit;
    font-size: 13px;
    cursor: default;
    box-shadow: 0 0.5px 1px rgb(0 0 0 / 20%);
}

.launcher-preview-window.light-mode .settings-save-panel-button {
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
}

.settings-save-panel-button:active {
    filter: brightness(0.9);
}

.settings-save-panel-button.is-default {
    border-color: transparent;
    background: #0a84ff;
    color: #fff;
}

.launcher-preview-window.light-mode .settings-save-panel-button.is-default {
    background: #007aff;
}

.settings-save-panel-button:disabled {
    opacity: 0.45;
}

.settings-save-panel-enter-active,
.settings-save-panel-leave-active {
    transition: opacity 120ms ease;
}

.settings-save-panel-enter-active .settings-save-panel,
.settings-save-panel-leave-active .settings-save-panel {
    transition: transform 200ms ease;
}

.settings-save-panel-enter-from,
.settings-save-panel-leave-to {
    opacity: 0;
}

.settings-save-panel-enter-from .settings-save-panel,
.settings-save-panel-leave-to .settings-save-panel {
    transform: translate(-50%, -100%);
}
</style>
