<script setup lang="ts">
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import HomeIcon from '@modrinth/assets/icons/home.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'

import { injectPreviewWindow } from '../../../../lib/window'
import { messages } from '../../messages'
import { useJre } from './jre'
import { baseName, type FsNode, HOME, joinPath, lookupPath, normalizePath, parentPath } from './mac'

/**
 * The open panel behind `open()` of @tauri-apps/plugin-dialog: on macOS an
 * NSOpenPanel sheet hanging from the top of the launcher window, choosing one file.
 * It browses the preview Mac's disk (`./mac.ts`) with the Finder sidebar, a list
 * view, back/forward, the path menu and the Finder shortcuts (⌘⇧G "Go to Folder",
 * ⌘⇧. hidden files, ⌘↑/⌘↓, type-to-select). It opens where the app's last pick was.
 */
const { formatMessage, locale } = useVIntl()
const { layer } = injectPreviewWindow()
const jre = useJre()

const open = ref(false)
const directory = ref(HOME)
const selectedName = ref<string | null>(null)
const backStack = ref<string[]>([])
const forwardStack = ref<string[]>([])
const pathMenuOpen = ref(false)
const goToOpen = ref(false)
const goToText = ref('')
const listEl = ref<HTMLElement | null>(null)
const goToInput = ref<HTMLInputElement | null>(null)
let resolver: ((path: string | null) => void) | undefined
let typeahead = ''
let typeaheadAt = 0

const FOLDER_LABELS: Record<string, MessageDescriptor> = {
    '/Applications': messages.launcherPreviewSettingsOpenPanelFolderApplications,
    '/Library': messages.launcherPreviewSettingsOpenPanelFolderLibrary,
    '/System': messages.launcherPreviewSettingsOpenPanelFolderSystem,
    '/Users': messages.launcherPreviewSettingsOpenPanelFolderUsers,
    '/Users/Shared': messages.launcherPreviewSettingsOpenPanelFolderShared,
    [`${HOME}/Applications`]: messages.launcherPreviewSettingsOpenPanelFolderApplications,
    [`${HOME}/Desktop`]: messages.launcherPreviewSettingsOpenPanelFolderDesktop,
    [`${HOME}/Documents`]: messages.launcherPreviewSettingsOpenPanelFolderDocuments,
    [`${HOME}/Downloads`]: messages.launcherPreviewSettingsOpenPanelFolderDownloads,
    [`${HOME}/Library`]: messages.launcherPreviewSettingsOpenPanelFolderLibrary,
    [`${HOME}/Movies`]: messages.launcherPreviewSettingsOpenPanelFolderMovies,
    [`${HOME}/Music`]: messages.launcherPreviewSettingsOpenPanelFolderMusic,
    [`${HOME}/Pictures`]: messages.launcherPreviewSettingsOpenPanelFolderPictures,
    [`${HOME}/Public`]: messages.launcherPreviewSettingsOpenPanelFolderPublic,
}

const favorites = [
    { path: '/Applications', icon: GridIcon },
    { path: `${HOME}/Desktop`, icon: MonitorIcon },
    { path: `${HOME}/Documents`, icon: FileTextIcon },
    { path: `${HOME}/Downloads`, icon: DownloadIcon },
    { path: HOME, icon: HomeIcon },
]

/** Finder's display name: localized system folders, the volume name, apps without `.app`. */
function displayName(path: string, node?: FsNode | null) {
    if (path === '/') return formatMessage(messages.launcherPreviewSettingsOpenPanelMacintoshHd)
    const label = FOLDER_LABELS[path]
    if (label) return formatMessage(label)
    const name = node?.name ?? baseName(path)
    return node?.package && name.endsWith('.app') ? name.slice(0, -4) : name
}

const root = computed(() => {
    void jre.data.diskVersion
    return jre.disk()
})

const showHidden = computed(() => jre.data.openPanel.showHidden)

const entries = computed(() => {
    const node = lookupPath(root.value, directory.value)?.node
    const collator = new Intl.Collator(locale.value, { numeric: true, sensitivity: 'base' })
    return (node?.children ?? [])
        .filter((child) => showHidden.value || (!child.hidden && !child.name.startsWith('.')))
        .map((child) => ({ node: child, path: joinPath(directory.value, child.name) }))
        .sort((a, b) => collator.compare(displayName(a.path, a.node), displayName(b.path, b.node)))
})

const selected = computed(
    () => entries.value.find((entry) => entry.node.name === selectedName.value) ?? null,
)

/** The path menu: this folder, then each enclosing folder up to the volume. */
const pathChain = computed(() => {
    const chain: string[] = []
    let current = directory.value
    while (true) {
        chain.push(current)
        if (current === '/') break
        current = parentPath(current)
    }
    return chain
})

function iconKind(node: FsNode) {
    if (node.package) return 'app'
    if (node.kind === 'dir') return 'folder'
    if (node.runtime || (!node.name.includes('.') && /\/bin$/.test(directory.value))) return 'exec'
    return 'document'
}

const dateFormat = computed(
    () => new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium', timeStyle: 'short' }),
)
const timeFormat = computed(() => new Intl.DateTimeFormat(locale.value, { timeStyle: 'short' }))

function formatModified(iso: string) {
    const date = new Date(iso)
    const today = new Date()
    const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime()
    const time = timeFormat.value.format(date)
    if (date.getTime() >= startOfToday) {
        return formatMessage(messages.launcherPreviewSettingsOpenPanelToday, { time })
    }
    if (date.getTime() >= startOfToday - 86_400_000) {
        return formatMessage(messages.launcherPreviewSettingsOpenPanelYesterday, { time })
    }
    return dateFormat.value.format(date)
}

/** Finder's decimal sizes; folders show "--". */
function formatSize(node: FsNode) {
    if (node.kind === 'dir') return '--'
    const bytes = node.size
    if (bytes < 1000) {
        return formatMessage(messages.launcherPreviewSettingsOpenPanelBytes, { count: bytes })
    }
    if (bytes < 1_000_000) return `${Math.round(bytes / 1000)} KB`
    if (bytes < 1_000_000_000) return `${(bytes / 1_000_000).toFixed(1)} MB`
    return `${(bytes / 1_000_000_000).toFixed(2)} GB`
}

function settle(path: string | null) {
    if (!open.value) return
    open.value = false
    pathMenuOpen.value = false
    goToOpen.value = false
    window.removeEventListener('keydown', onKeydown, true)
    if (path) jre.data.openPanel.directory = parentPath(path)
    const resolve = resolver
    resolver = undefined
    resolve?.(path)
}

function navigate(path: string, options: { record?: boolean; select?: string | null } = {}) {
    const target = lookupPath(root.value, path)
    if (!target || target.node.kind !== 'dir' || target.node.package) return
    if (target.path !== directory.value && options.record !== false) {
        backStack.value = [...backStack.value, directory.value]
        forwardStack.value = []
    }
    directory.value = target.path
    selectedName.value = options.select ?? null
    pathMenuOpen.value = false
    void nextTick(() => listEl.value?.scrollTo({ top: 0 }))
    if (options.select) void nextTick(scrollSelectionIntoView)
}

function goBack() {
    const previous = backStack.value.at(-1)
    if (!previous) return
    backStack.value = backStack.value.slice(0, -1)
    forwardStack.value = [directory.value, ...forwardStack.value]
    const from = directory.value
    navigate(previous, {
        record: false,
        select: parentPath(from) === previous ? baseName(from) : null,
    })
}

function goForward() {
    const next = forwardStack.value[0]
    if (!next) return
    forwardStack.value = forwardStack.value.slice(1)
    backStack.value = [...backStack.value, directory.value]
    navigate(next, { record: false })
}

function goUp() {
    if (directory.value === '/') return
    const from = directory.value
    navigate(parentPath(from), { select: baseName(from) })
}

/** Opens the selection: folders are entered, files and bundles are chosen. */
function activate(entry = selected.value) {
    if (!entry) return
    if (entry.node.kind === 'dir' && !entry.node.package) navigate(entry.path)
    else settle(entry.path)
}

function cancel() {
    settle(null)
}

function scrollSelectionIntoView() {
    listEl.value
        ?.querySelector<HTMLElement>('.settings-open-panel-row.is-selected')
        ?.scrollIntoView({ block: 'nearest' })
}

function moveSelection(step: number) {
    const list = entries.value
    if (!list.length) return
    const index = list.findIndex((entry) => entry.node.name === selectedName.value)
    const next =
        index < 0
            ? step > 0
                ? 0
                : list.length - 1
            : Math.min(list.length - 1, Math.max(0, index + step))
    selectedName.value = list[next].node.name
    void nextTick(scrollSelectionIntoView)
}

function typeToSelect(char: string) {
    const now = Date.now()
    typeahead = now - typeaheadAt > 1000 ? char : typeahead + char
    typeaheadAt = now
    const query = typeahead.toLowerCase()
    const match = entries.value.find((entry) =>
        displayName(entry.path, entry.node).toLowerCase().startsWith(query),
    )
    if (match) {
        selectedName.value = match.node.name
        void nextTick(scrollSelectionIntoView)
    }
}

function openGoTo() {
    goToOpen.value = true
    goToText.value = directory.value === '/' ? '/' : `${directory.value}/`
    pathMenuOpen.value = false
    void nextTick(() => goToInput.value?.focus())
}

/** "Go to Folder": a folder is entered, a file is revealed and selected. */
function submitGoTo() {
    let text = goToText.value.trim()
    if (!text) return
    if (text === '~' || text.startsWith('~/')) text = HOME + text.slice(1)
    const target = lookupPath(root.value, normalizePath(text))
    if (!target) return
    goToOpen.value = false
    if (target.node.kind === 'dir' && !target.node.package) navigate(target.path)
    else navigate(parentPath(target.path), { select: target.node.name })
}

function toggleHidden() {
    jre.data.openPanel.showHidden = !jre.data.openPanel.showHidden
}

function onPanelMouseDown(event: MouseEvent) {
    const target = event.target as HTMLElement | null
    if (pathMenuOpen.value && !target?.closest('.settings-open-panel-path')) {
        pathMenuOpen.value = false
    }
}

function onKeydown(event: KeyboardEvent) {
    if (!open.value) return
    // ⌘ on the Mac; the Ctrl key stands in for it on other keyboards.
    const command = event.metaKey || event.ctrlKey
    const handled = () => {
        event.preventDefault()
        event.stopPropagation()
    }
    if (goToOpen.value) {
        if (event.key === 'Escape') {
            handled()
            goToOpen.value = false
        } else if (event.key === 'Enter') {
            handled()
            submitGoTo()
        } else {
            event.stopPropagation()
        }
        return
    }
    if (event.key === 'Escape') {
        handled()
        if (pathMenuOpen.value) pathMenuOpen.value = false
        else cancel()
    } else if (event.key === 'Enter') {
        handled()
        activate()
    } else if (command && event.shiftKey && event.code === 'KeyG') {
        handled()
        openGoTo()
    } else if (command && event.shiftKey && event.code === 'Period') {
        handled()
        toggleHidden()
    } else if (command && event.shiftKey && event.code === 'KeyH') {
        handled()
        navigate(HOME)
    } else if (command && event.shiftKey && event.code === 'KeyD') {
        handled()
        navigate(`${HOME}/Desktop`)
    } else if (command && event.shiftKey && event.code === 'KeyO') {
        handled()
        navigate(`${HOME}/Documents`)
    } else if (command && event.shiftKey && event.code === 'KeyA') {
        handled()
        navigate('/Applications')
    } else if (command && event.altKey && event.code === 'KeyL') {
        handled()
        navigate(`${HOME}/Downloads`)
    } else if (command && event.key === 'ArrowUp') {
        handled()
        goUp()
    } else if (command && event.key === 'ArrowDown') {
        handled()
        activate()
    } else if (command && event.key === '[') {
        handled()
        goBack()
    } else if (command && event.key === ']') {
        handled()
        goForward()
    } else if (event.key === 'ArrowDown') {
        handled()
        moveSelection(1)
    } else if (event.key === 'ArrowUp') {
        handled()
        moveSelection(-1)
    } else if (!command && !event.altKey && event.key.length === 1) {
        handled()
        typeToSelect(event.key)
    } else {
        // The sheet is modal: the window's shortcuts do not see the keyboard.
        event.stopPropagation()
    }
}

/** Shows the sheet; resolves with the chosen file's path, or null when cancelled. */
function pick(): Promise<string | null> {
    settle(null)
    return new Promise((resolve) => {
        resolver = resolve
        const start = lookupPath(root.value, jre.data.openPanel.directory)
        directory.value = start?.node.kind === 'dir' ? start.path : HOME
        selectedName.value = null
        backStack.value = []
        forwardStack.value = []
        pathMenuOpen.value = false
        goToOpen.value = false
        open.value = true
        window.addEventListener('keydown', onKeydown, true)
    })
}

onBeforeUnmount(() => settle(null))

defineExpose({ pick })
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <Transition name="settings-open-panel">
            <div v-if="open" class="settings-open-panel-backdrop" @mousedown.self.prevent>
                <div
                    role="dialog"
                    aria-modal="true"
                    class="settings-open-panel"
                    @mousedown="onPanelMouseDown"
                >
                    <aside class="settings-open-panel-sidebar">
                        <div class="settings-open-panel-section">
                            {{ formatMessage(messages.launcherPreviewSettingsOpenPanelFavorites) }}
                        </div>
                        <button
                            v-for="favorite in favorites"
                            :key="favorite.path"
                            type="button"
                            class="settings-open-panel-place"
                            :class="{ 'is-active': directory === favorite.path }"
                            @click="navigate(favorite.path)"
                        >
                            <component :is="favorite.icon" aria-hidden="true" />
                            <span>{{ displayName(favorite.path) }}</span>
                        </button>
                        <div class="settings-open-panel-section">
                            {{ formatMessage(messages.launcherPreviewSettingsOpenPanelLocations) }}
                        </div>
                        <button
                            type="button"
                            class="settings-open-panel-place"
                            :class="{ 'is-active': directory === '/' }"
                            @click="navigate('/')"
                        >
                            <svg viewBox="0 0 16 16" aria-hidden="true">
                                <rect
                                    x="1.5"
                                    y="4.5"
                                    width="13"
                                    height="7"
                                    rx="1.6"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="1.2"
                                />
                                <circle cx="11.6" cy="8" r="0.9" fill="currentColor" />
                            </svg>
                            <span>{{ displayName('/') }}</span>
                        </button>
                    </aside>

                    <section class="settings-open-panel-main">
                        <header class="settings-open-panel-toolbar">
                            <div class="settings-open-panel-nav">
                                <button
                                    type="button"
                                    class="settings-open-panel-nav-button"
                                    :disabled="!backStack.length"
                                    :aria-label="
                                        formatMessage(messages.launcherPreviewSettingsOpenPanelBack)
                                    "
                                    @click="goBack"
                                >
                                    <ChevronLeftIcon aria-hidden="true" />
                                </button>
                                <button
                                    type="button"
                                    class="settings-open-panel-nav-button"
                                    :disabled="!forwardStack.length"
                                    :aria-label="
                                        formatMessage(
                                            messages.launcherPreviewSettingsOpenPanelForward,
                                        )
                                    "
                                    @click="goForward"
                                >
                                    <ChevronRightIcon aria-hidden="true" />
                                </button>
                            </div>
                            <div class="settings-open-panel-path">
                                <button
                                    type="button"
                                    class="settings-open-panel-path-button"
                                    aria-haspopup="menu"
                                    :aria-expanded="pathMenuOpen"
                                    @click="pathMenuOpen = !pathMenuOpen"
                                >
                                    <span class="settings-open-panel-glyph is-folder is-small" />
                                    <span class="truncate">{{ displayName(directory) }}</span>
                                    <svg
                                        viewBox="0 0 10 14"
                                        class="settings-open-panel-updown"
                                        aria-hidden="true"
                                    >
                                        <path
                                            d="M2 5l3-3 3 3M2 9l3 3 3-3"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="1.3"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                        />
                                    </svg>
                                </button>
                                <div
                                    v-if="pathMenuOpen"
                                    role="menu"
                                    class="settings-open-panel-menu"
                                >
                                    <button
                                        v-for="path in pathChain"
                                        :key="path"
                                        type="button"
                                        role="menuitem"
                                        class="settings-open-panel-menu-item"
                                        @click="navigate(path)"
                                    >
                                        <span
                                            class="settings-open-panel-glyph is-small"
                                            :class="path === '/' ? 'is-drive' : 'is-folder'"
                                        />
                                        <span class="truncate">{{ displayName(path) }}</span>
                                    </button>
                                </div>
                            </div>
                        </header>

                        <div class="settings-open-panel-columns" aria-hidden="true">
                            <span>{{
                                formatMessage(messages.launcherPreviewSettingsOpenPanelName)
                            }}</span>
                            <span>{{
                                formatMessage(messages.launcherPreviewSettingsOpenPanelDateModified)
                            }}</span>
                            <span class="is-size">{{
                                formatMessage(messages.launcherPreviewSettingsOpenPanelSize)
                            }}</span>
                        </div>
                        <div
                            ref="listEl"
                            class="settings-open-panel-list"
                            role="listbox"
                            @mousedown.self="selectedName = null"
                        >
                            <div
                                v-for="entry in entries"
                                :key="entry.path"
                                role="option"
                                class="settings-open-panel-row"
                                :class="{ 'is-selected': entry.node.name === selectedName }"
                                :aria-selected="entry.node.name === selectedName"
                                @mousedown="selectedName = entry.node.name"
                                @dblclick="activate(entry)"
                            >
                                <span class="settings-open-panel-name">
                                    <span
                                        class="settings-open-panel-glyph"
                                        :class="`is-${iconKind(entry.node)}`"
                                    />
                                    <span class="truncate">{{
                                        displayName(entry.path, entry.node)
                                    }}</span>
                                </span>
                                <span class="settings-open-panel-meta">{{
                                    formatModified(entry.node.modified)
                                }}</span>
                                <span class="settings-open-panel-meta is-size">{{
                                    formatSize(entry.node)
                                }}</span>
                            </div>
                        </div>

                        <footer class="settings-open-panel-buttons">
                            <button
                                type="button"
                                class="settings-open-panel-button"
                                @click="cancel"
                            >
                                {{ formatMessage(messages.launcherPreviewSettingsOpenPanelCancel) }}
                            </button>
                            <button
                                type="button"
                                class="settings-open-panel-button is-default"
                                :disabled="!selected"
                                @click="activate()"
                            >
                                {{ formatMessage(messages.launcherPreviewSettingsOpenPanelOpen) }}
                            </button>
                        </footer>
                    </section>

                    <div v-if="goToOpen" class="settings-open-panel-goto">
                        <span>{{
                            formatMessage(messages.launcherPreviewSettingsOpenPanelGoToFolder)
                        }}</span>
                        <input
                            ref="goToInput"
                            v-model="goToText"
                            class="settings-open-panel-field"
                            type="text"
                            spellcheck="false"
                            autocomplete="off"
                        />
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
/*
 * macOS system chrome (NSOpenPanel as a window sheet). The desktop app gets this
 * panel from the operating system, so it uses the system's colours rather than the
 * launcher's theme tokens, like the save panel of the logs page.
 */
.settings-open-panel-backdrop {
    position: absolute;
    inset: 0;
    z-index: 400;
    font-family:
        -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', system-ui, sans-serif;
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
    user-select: none;
}

.settings-open-panel {
    --panel-text: rgb(255 255 255 / 88%);
    --panel-secondary: rgb(255 255 255 / 50%);
    --panel-separator: rgb(255 255 255 / 9%);
    --panel-control: rgb(255 255 255 / 12%);
    --panel-accent: #0a84ff;
    --panel-row-alt: rgb(255 255 255 / 3.5%);
    position: absolute;
    top: 48px;
    left: 50%;
    display: grid;
    width: 700px;
    height: 430px;
    grid-template-columns: 168px minmax(0, 1fr);
    overflow: hidden;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 0 0 12px 12px;
    background: rgb(30 30 32 / 98%);
    color: var(--panel-text);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 60%),
        0 22px 70px 4px rgb(0 0 0 / 56%);
    transform: translateX(-50%);
}

.launcher-preview-window.light-mode .settings-open-panel {
    --panel-text: rgb(0 0 0 / 85%);
    --panel-secondary: rgb(0 0 0 / 50%);
    --panel-separator: rgb(0 0 0 / 10%);
    --panel-control: rgb(0 0 0 / 6%);
    --panel-accent: #007aff;
    --panel-row-alt: rgb(0 0 0 / 3%);
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 18%),
        0 22px 70px 4px rgb(0 0 0 / 28%);
}

.settings-open-panel-sidebar {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 1px;
    overflow-y: auto;
    padding: 10px 8px;
    border-right: 0.5px solid var(--panel-separator);
    background: rgb(52 52 54 / 92%);
}

.launcher-preview-window.light-mode .settings-open-panel-sidebar {
    background: rgb(232 232 232 / 92%);
}

.settings-open-panel-section {
    padding: 8px 8px 3px;
    color: var(--panel-secondary);
    font-size: 11px;
    font-weight: 600;
}

.settings-open-panel-place {
    display: flex;
    height: 24px;
    align-items: center;
    gap: 7px;
    padding: 0 8px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
}

.settings-open-panel-place svg {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    color: var(--panel-accent);
}

.settings-open-panel-place.is-active {
    background: var(--panel-control);
}

.settings-open-panel-main {
    display: flex;
    min-height: 0;
    min-width: 0;
    flex-direction: column;
}

.settings-open-panel-toolbar {
    display: flex;
    height: 44px;
    flex-shrink: 0;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border-bottom: 0.5px solid var(--panel-separator);
}

.settings-open-panel-nav {
    display: flex;
    gap: 2px;
}

.settings-open-panel-nav-button {
    display: flex;
    width: 28px;
    height: 24px;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    cursor: default;
}

.settings-open-panel-nav-button:not(:disabled):hover {
    background: var(--panel-control);
}

.settings-open-panel-nav-button:disabled {
    opacity: 0.3;
}

.settings-open-panel-nav-button svg {
    width: 16px;
    height: 16px;
}

.settings-open-panel-path {
    position: relative;
    min-width: 0;
}

.settings-open-panel-path-button {
    display: flex;
    max-width: 280px;
    height: 24px;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: default;
}

.settings-open-panel-path-button:hover {
    background: var(--panel-control);
}

.settings-open-panel-updown {
    width: 8px;
    height: 12px;
    flex-shrink: 0;
    opacity: 0.6;
}

.settings-open-panel-menu {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    z-index: 2;
    display: flex;
    min-width: 220px;
    flex-direction: column;
    padding: 5px;
    border: 0.5px solid rgb(255 255 255 / 16%);
    border-radius: 8px;
    background: rgb(50 50 52 / 98%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 50%),
        0 10px 30px rgb(0 0 0 / 45%);
}

.launcher-preview-window.light-mode .settings-open-panel-menu {
    border-color: rgb(0 0 0 / 10%);
    background: rgb(246 246 246 / 98%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 12%),
        0 10px 30px rgb(0 0 0 / 18%);
}

.settings-open-panel-menu-item {
    display: flex;
    height: 22px;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
}

.settings-open-panel-menu-item:hover {
    background: var(--panel-accent);
    color: #fff;
}

.settings-open-panel-columns,
.settings-open-panel-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 168px 76px;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
}

.settings-open-panel-columns {
    height: 26px;
    flex-shrink: 0;
    border-bottom: 0.5px solid var(--panel-separator);
    color: var(--panel-secondary);
    font-size: 11px;
    font-weight: 600;
}

.settings-open-panel-columns .is-size,
.settings-open-panel-meta.is-size {
    text-align: right;
}

.settings-open-panel-list {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    padding: 2px 6px;
}

.settings-open-panel-row {
    height: 24px;
    padding: 0 6px;
    border-radius: 5px;
    cursor: default;
}

.settings-open-panel-row:nth-child(even) {
    background: var(--panel-row-alt);
}

.settings-open-panel-row.is-selected {
    background: var(--panel-accent);
    color: #fff;
}

.settings-open-panel-row.is-selected .settings-open-panel-meta {
    color: rgb(255 255 255 / 85%);
}

.settings-open-panel-name {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
}

.settings-open-panel-meta {
    overflow: hidden;
    color: var(--panel-secondary);
    white-space: nowrap;
    text-overflow: ellipsis;
}

/* File icons in the list: a blue folder, an app, a Unix executable, a document. */
.settings-open-panel-glyph {
    position: relative;
    display: inline-block;
    width: 18px;
    height: 15px;
    flex-shrink: 0;
}

.settings-open-panel-glyph.is-small {
    width: 15px;
    height: 12px;
}

.settings-open-panel-glyph.is-folder {
    border-radius: 2px 2px 3px 3px;
    background: linear-gradient(#5ac0ff, #33a3f5);
    box-shadow: inset 0 2px 0 rgb(255 255 255 / 25%);
}

.settings-open-panel-glyph.is-folder::before {
    position: absolute;
    top: -2px;
    left: 0;
    width: 45%;
    height: 4px;
    border-radius: 2px 2px 0 0;
    background: #33a3f5;
    content: '';
}

.settings-open-panel-glyph.is-app {
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: linear-gradient(135deg, #a7a7ad, #6f6f75);
}

.settings-open-panel-glyph.is-exec {
    width: 16px;
    height: 15px;
    border-radius: 2px;
    background: #1d1d1f;
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 30%);
}

.settings-open-panel-glyph.is-exec::after {
    position: absolute;
    bottom: 2px;
    left: 2px;
    color: #30d158;
    content: 'exec';
    font-family: Menlo, monospace;
    font-size: 5px;
    line-height: 1;
}

.settings-open-panel-glyph.is-document {
    width: 13px;
    height: 16px;
    margin: 0 2px 0 3px;
    border-radius: 1px 4px 1px 1px;
    background: #f4f4f6;
    box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 25%);
}

.settings-open-panel-glyph.is-drive {
    border-radius: 3px;
    background: linear-gradient(#d4d4d8, #9d9da3);
}

.settings-open-panel-buttons {
    display: flex;
    flex-shrink: 0;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 14px 12px;
    border-top: 0.5px solid var(--panel-separator);
}

.settings-open-panel-button {
    min-width: 76px;
    height: 28px;
    padding: 0 12px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 6px;
    background: rgb(255 255 255 / 16%);
    color: inherit;
    font: inherit;
    cursor: default;
    box-shadow: 0 0.5px 1px rgb(0 0 0 / 20%);
}

.launcher-preview-window.light-mode .settings-open-panel-button {
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
}

.settings-open-panel-button:active {
    filter: brightness(0.9);
}

.settings-open-panel-button.is-default {
    border-color: transparent;
    background: var(--panel-accent);
    color: #fff;
}

.settings-open-panel-button:disabled {
    opacity: 0.45;
}

.settings-open-panel-button:focus-visible,
.settings-open-panel-place:focus-visible,
.settings-open-panel-path-button:focus-visible {
    outline: 3px solid rgb(10 132 255 / 55%);
    outline-offset: 0;
}

/* ⌘⇧G: the floating "Go to Folder" field over the panel. */
.settings-open-panel-goto {
    position: absolute;
    top: 58px;
    left: 50%;
    display: flex;
    width: 420px;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border: 0.5px solid rgb(255 255 255 / 16%);
    border-radius: 10px;
    background: rgb(50 50 52 / 98%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 50%),
        0 14px 40px rgb(0 0 0 / 45%);
    transform: translateX(-50%);
}

.launcher-preview-window.light-mode .settings-open-panel-goto {
    border-color: rgb(0 0 0 / 10%);
    background: rgb(246 246 246 / 98%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 12%),
        0 14px 40px rgb(0 0 0 / 18%);
}

.settings-open-panel-goto > span {
    color: var(--panel-secondary);
    font-size: 11px;
    font-weight: 600;
}

/* The website's global `input` rules do not belong to the system panel. */
.settings-open-panel-field {
    height: 26px;
    min-height: 0;
    min-width: 0;
    padding: 0 8px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 6px;
    background: rgb(255 255 255 / 8%);
    color: inherit;
    font: inherit;
    box-shadow: 0 0 0 3px rgb(10 132 255 / 55%);
    transition: none;
    user-select: text;
}

.launcher-preview-window.light-mode .settings-open-panel-field {
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
}

.settings-open-panel-field:focus {
    color: inherit;
}

.settings-open-panel-field::selection {
    background: rgb(10 132 255 / 55%);
}

.settings-open-panel-enter-active,
.settings-open-panel-leave-active {
    transition: opacity 120ms ease;
}

.settings-open-panel-enter-active .settings-open-panel,
.settings-open-panel-leave-active .settings-open-panel {
    transition: transform 200ms ease;
}

.settings-open-panel-enter-from,
.settings-open-panel-leave-to {
    opacity: 0;
}

.settings-open-panel-enter-from .settings-open-panel,
.settings-open-panel-leave-to .settings-open-panel {
    transform: translate(-50%, -100%);
}
</style>
