<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, reactive, ref } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import { injectPreviewWindow } from '../../../../lib/window'
import { messages } from '../../messages'
import {
    baseName,
    canonicalize,
    createFolder,
    isDirectory,
    joinPath,
    parentPath,
    refreshDisk,
    useMacFileSystem,
} from './mac-fs'
import {
    displayName,
    expandHome,
    macAppearance,
    type MacListEntry,
    type MacOpenOptions,
    type MacPickMode,
    useMacBrowser,
} from './mac-ui'
import MacFileBrowser from './MacFileBrowser.vue'

/**
 * The macOS open panel `@tauri-apps/plugin-dialog`'s `open()` shows, as a
 * sheet on the launcher window: the request's title as the panel message, the
 * Finder browser over the preview Mac's disk, New Folder, and Cancel / Open.
 * A folder picker (`directory: true`) greys out files and bundles, and Open
 * chooses the selected folder, or the folder on screen when nothing is
 * selected. A file picker enters folders on Open and greys out files outside
 * its extension filter.
 *
 * Keyboard: Escape cancels, Return opens, arrows move the selection,
 * Command-Up/Down leave or enter folders, Command-[ / ] go back and forward,
 * Shift-Command-N makes a folder, Shift-Command-G goes to a typed path and
 * Shift-Command-Period shows hidden items.
 */
const { layer } = injectPreviewWindow()
const preview = usePreview()
const { formatMessage } = useVIntl()
const fs = useMacFileSystem()
const pickMode = reactive<MacPickMode>({ directoriesOnly: true, extensions: null })
const browser = useMacBrowser(fs, pickMode)

const open = ref(false)
const shown = ref(false)
const message = ref('')
const appearance = ref<'dark' | 'light'>('dark')
const dialog = ref<'new-folder' | 'go-to' | null>(null)
const dialogInput = ref('')
const dialogError = ref('')
const dialogField = ref<HTMLInputElement | null>(null)
const browserView = ref<InstanceType<typeof MacFileBrowser> | null>(null)
let resolver: ((value: string | null) => void) | null = null
let cancelHide: (() => void) | undefined

const currentFolderName = computed(() => displayName(browser.path.value, formatMessage))
const canOpen = computed(() => {
    if (pickMode.directoriesOnly) return true
    const entry = selectedEntry()
    return !!entry && !entry.disabled
})

/** `open({ multiple: false, ... })`: resolves with the chosen path, or null when cancelled. */
async function pick(options: MacOpenOptions = {}): Promise<string | null> {
    if (resolver) finish(null)
    await fs.load()
    refreshDisk(fs, preview.instances.instances.value)
    appearance.value = macAppearance()
    message.value = options.title ?? ''
    pickMode.directoriesOnly = options.directory !== false
    pickMode.extensions = options.extensions?.map((extension) => extension.toLowerCase()) ?? null
    dialog.value = null
    browser.reset(options.defaultPath ?? fs.lastDirectory)
    cancelHide?.()
    open.value = true
    window.addEventListener('keydown', onKeydown, true)
    await nextTick()
    requestAnimationFrame(() => {
        shown.value = true
    })
    return new Promise<string | null>((resolve) => {
        resolver = resolve
    })
}

function finish(value: string | null) {
    if (!resolver) return
    const resolve = resolver
    resolver = null
    // The panel remembers where it was for the app's next request.
    fs.lastDirectory = browser.path.value
    shown.value = false
    dialog.value = null
    window.removeEventListener('keydown', onKeydown, true)
    cancelHide = preview.clock.after(240, () => {
        if (!resolver) open.value = false
    })
    resolve(value)
}

function selectedEntry(): MacListEntry | undefined {
    return browser.entries.value.find((entry) => entry.path === browser.selected.value)
}

function choose() {
    const entry = selectedEntry()
    if (pickMode.directoriesOnly) {
        finish(entry && !entry.disabled ? entry.path : browser.path.value)
        return
    }
    if (!entry || entry.disabled) return
    if (isDirectory(entry.node) && !entry.node.package) browser.go(entry.path)
    else finish(entry.path)
}

/** Double-click on a file or bundle (folders are entered by the browser). */
function onActivate(entry: MacListEntry) {
    if (entry.disabled || pickMode.directoriesOnly) return
    finish(entry.path)
}

async function openDialog(kind: 'new-folder' | 'go-to') {
    browserView.value?.closeMenus()
    dialog.value = kind
    dialogError.value = ''
    dialogInput.value =
        kind === 'new-folder'
            ? formatMessage(messages.launcherPreviewSettingsMacUntitledFolder)
            : browser.path.value
    await nextTick()
    dialogField.value?.focus()
    dialogField.value?.select()
}

function closeDialog() {
    dialog.value = null
}

function confirmDialog() {
    const root = fs.root
    if (!root) return
    if (dialog.value === 'new-folder') {
        const name = dialogInput.value.trim()
        if (!name) return
        const created = createFolder(root, browser.path.value, name)
        if (!created) {
            dialogError.value = formatMessage(messages.launcherPreviewSettingsMacNameTaken, {
                name,
            })
            return
        }
        dialog.value = null
        browser.go(joinPath(browser.path.value, created.name))
        return
    }
    if (dialog.value === 'go-to') {
        const resolved = canonicalize(root, expandHome(dialogInput.value))
        if (!resolved || !dialogInput.value.trim()) {
            dialogError.value = formatMessage(messages.launcherPreviewSettingsMacFolderNotFound)
            return
        }
        dialog.value = null
        if (isDirectory(resolved.node) && !resolved.node.package) {
            browser.go(resolved.path)
        } else {
            browser.go(parentPath(resolved.path))
            browser.selected.value = joinPath(parentPath(resolved.path), baseName(resolved.path))
        }
    }
}

function consume(event: KeyboardEvent) {
    event.preventDefault()
    event.stopPropagation()
}

function onKeydown(event: KeyboardEvent) {
    const command = event.metaKey || event.ctrlKey
    const typing = (event.target as HTMLElement | null)?.tagName === 'INPUT'
    if (event.key === 'Escape') {
        consume(event)
        if (browserView.value?.closeMenus()) return
        if (dialog.value) closeDialog()
        else finish(null)
        return
    }
    if (dialog.value) {
        if (event.key === 'Enter') {
            consume(event)
            confirmDialog()
        }
        return
    }
    if (event.key === 'Enter') {
        consume(event)
        choose()
        return
    }
    if (command && event.shiftKey && event.code === 'KeyN') {
        consume(event)
        void openDialog('new-folder')
        return
    }
    if (command && event.shiftKey && event.code === 'KeyG') {
        consume(event)
        void openDialog('go-to')
        return
    }
    if (command && event.shiftKey && event.code === 'Period') {
        consume(event)
        fs.showHidden = !fs.showHidden
        return
    }
    if (command && event.key === 'ArrowUp') {
        consume(event)
        browser.up()
        return
    }
    if (command && event.key === 'ArrowDown') {
        consume(event)
        const entry = selectedEntry()
        if (entry && isDirectory(entry.node) && !entry.node.package) browser.go(entry.path)
        return
    }
    if (command && event.key === '[') {
        consume(event)
        browser.back()
        return
    }
    if (command && event.key === ']') {
        consume(event)
        browser.forward()
        return
    }
    if (typing) return
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        consume(event)
        browser.moveSelection(event.key === 'ArrowDown' ? 1 : -1)
    }
}

onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown, true)
    const resolve = resolver
    resolver = null
    resolve?.(null)
})

defineExpose({ pick })
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <div v-if="open" class="lp-mac-sheet-root">
            <!-- The sheet is modal to the launcher window: clicks outside it go nowhere. -->
            <div class="lp-mac-sheet-blocker" @mousedown.prevent />
            <div
                class="lp-mac-sheet lp-mac-surface"
                :class="[`lp-mac-${appearance}`, { 'is-shown': shown }]"
                role="dialog"
                aria-modal="true"
                :aria-label="message"
            >
                <div v-if="message" class="lp-mac-sheet-message">{{ message }}</div>
                <MacFileBrowser
                    ref="browserView"
                    :browser="browser"
                    variant="panel"
                    :appearance="appearance"
                    @activate="onActivate"
                />
                <div class="lp-mac-sheet-footer">
                    <button type="button" class="lp-mac-button" @click="openDialog('new-folder')">
                        {{ formatMessage(messages.launcherPreviewSettingsMacNewFolder) }}
                    </button>
                    <div class="flex-1" />
                    <button type="button" class="lp-mac-button" @click="finish(null)">
                        {{ formatMessage(messages.launcherPreviewSettingsMacCancel) }}
                    </button>
                    <button
                        type="button"
                        class="lp-mac-button is-default"
                        :disabled="!canOpen"
                        @click="choose"
                    >
                        {{ formatMessage(messages.launcherPreviewSettingsMacOpen) }}
                    </button>
                </div>

                <div v-if="dialog" class="lp-mac-dialog-backdrop">
                    <div class="lp-mac-dialog" role="dialog" aria-modal="true">
                        <div class="lp-mac-dialog-title">
                            {{
                                formatMessage(
                                    dialog === 'new-folder'
                                        ? messages.launcherPreviewSettingsMacNewFolder
                                        : messages.launcherPreviewSettingsMacGoToFolder,
                                )
                            }}
                        </div>
                        <p class="lp-mac-dialog-prompt">
                            {{
                                dialog === 'new-folder'
                                    ? formatMessage(
                                          messages.launcherPreviewSettingsMacNewFolderPrompt,
                                          {
                                              folder: currentFolderName,
                                          },
                                      )
                                    : formatMessage(
                                          messages.launcherPreviewSettingsMacGoToFolderPrompt,
                                      )
                            }}
                        </p>
                        <input
                            ref="dialogField"
                            v-model="dialogInput"
                            type="text"
                            class="lp-mac-field"
                            spellcheck="false"
                            @input="dialogError = ''"
                        />
                        <p v-if="dialogError" class="lp-mac-dialog-error">{{ dialogError }}</p>
                        <div class="lp-mac-dialog-buttons">
                            <button type="button" class="lp-mac-button" @click="closeDialog">
                                {{ formatMessage(messages.launcherPreviewSettingsMacCancel) }}
                            </button>
                            <button
                                type="button"
                                class="lp-mac-button is-default"
                                :disabled="!dialogInput.trim()"
                                @click="confirmDialog"
                            >
                                {{
                                    formatMessage(
                                        dialog === 'new-folder'
                                            ? messages.launcherPreviewSettingsMacCreate
                                            : messages.launcherPreviewSettingsMacGo,
                                    )
                                }}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped lang="scss">
.lp-mac-sheet-root {
    position: fixed;
    inset: 0;
    z-index: 400;
    pointer-events: none;
}

.lp-mac-sheet-blocker {
    position: absolute;
    inset: 0;
    pointer-events: auto;
}

.lp-mac-sheet {
    position: absolute;
    top: 34px;
    left: 50%;
    display: flex;
    width: 780px;
    height: 480px;
    flex-direction: column;
    overflow: hidden;
    border-radius: 12px;
    background: #2a2a2a;
    box-shadow:
        0 0 0 0.5px rgba(0, 0, 0, 0.85),
        inset 0 0 0 0.5px rgba(255, 255, 255, 0.16),
        0 24px 60px rgba(0, 0, 0, 0.55);
    pointer-events: auto;
    transform: translate(-50%, -112%);
    transition: transform 0.28s cubic-bezier(0.2, 0.85, 0.3, 1);

    &.is-shown {
        transform: translate(-50%, 0);
    }

    &.lp-mac-light {
        background: #f6f6f6;
        box-shadow:
            0 0 0 0.5px rgba(0, 0, 0, 0.22),
            0 24px 60px rgba(0, 0, 0, 0.3);
    }

    @media (prefers-reduced-motion) {
        transition: none;
    }
}

.lp-mac-sheet-message {
    flex-shrink: 0;
    padding: 9px 16px 8px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.5);
    text-align: center;

    .lp-mac-light & {
        border-bottom-color: rgba(0, 0, 0, 0.1);
    }
}

.lp-mac-sheet-footer {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-top: 1px solid rgba(0, 0, 0, 0.5);

    .lp-mac-light & {
        border-top-color: rgba(0, 0, 0, 0.1);
    }
}

.lp-mac-dialog-backdrop {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 60px;
    background: rgba(0, 0, 0, 0.18);
}

.lp-mac-dialog {
    display: flex;
    width: 340px;
    flex-direction: column;
    gap: 8px;
    padding: 18px 18px 14px;
    border-radius: 12px;
    background: #323232;
    box-shadow:
        0 0 0 0.5px rgba(0, 0, 0, 0.85),
        inset 0 0 0 0.5px rgba(255, 255, 255, 0.16),
        0 16px 40px rgba(0, 0, 0, 0.5);

    .lp-mac-light & {
        background: #ececec;
        box-shadow:
            0 0 0 0.5px rgba(0, 0, 0, 0.2),
            0 16px 40px rgba(0, 0, 0, 0.25);
    }
}

.lp-mac-dialog-title {
    font-weight: 700;
}

.lp-mac-dialog-prompt {
    margin: 0;
}

.lp-mac-dialog-error {
    margin: 0;
    color: #ff6b5f;
    font-size: 11px;

    .lp-mac-light & {
        color: #d70015;
    }
}

.lp-mac-dialog-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 6px;
}
</style>
