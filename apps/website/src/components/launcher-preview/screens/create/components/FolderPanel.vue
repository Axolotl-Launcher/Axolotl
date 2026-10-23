<script setup lang="ts">
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FileIcon from '@modrinth/assets/icons/file.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import HomeIcon from '@modrinth/assets/icons/home.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { type Component, computed, nextTick, onBeforeUnmount, ref, shallowRef } from 'vue'

import { injectPreviewWindow } from '../../../lib/window'
import { APP_SUPPORT, findNode, type FsNode, HOME, loadMacFileSystem } from '../mac-fs'
import { folderPanelMessages as messages } from '../messages'

/**
 * The native macOS open panel the app's `pickFolder` (`@tauri-apps/plugin-dialog`
 * `open({ directory: true })`) shows, simulated over the preview Mac's disk. It
 * opens where launchers keep their data (`~/Library/Application Support`); folders
 * can be chosen, files are listed dimmed. `browse()` shows a folder the way Finder
 * does for the app's "open folder" buttons.
 */
const { formatMessage, locale } = useVIntl()
const { layer } = injectPreviewWindow()

const open = ref(false)
const mode = ref<'pick' | 'browse'>('pick')
const root = shallowRef<FsNode | null>(null)
const currentPath = ref(APP_SUPPORT)
const backStack = ref<string[]>([])
const forwardStack = ref<string[]>([])
const selectedName = ref<string | null>(null)
const listRef = ref<HTMLElement | null>(null)
let resolvePick: ((path: string | null) => void) | null = null

const favorites = computed<{ path: string; label: string; icon: Component }[]>(() => [
    { path: HOME, label: HOME.split('/').pop() ?? HOME, icon: HomeIcon },
    { path: `${HOME}/Desktop`, label: 'Desktop', icon: MonitorIcon },
    { path: `${HOME}/Documents`, label: 'Documents', icon: FileTextIcon },
    { path: `${HOME}/Downloads`, label: 'Downloads', icon: DownloadIcon },
])

const currentNode = computed(() => (root.value ? findNode(root.value, currentPath.value) : null))
const currentName = computed(
    () => currentPath.value.split('/').filter(Boolean).pop() ?? 'Macintosh HD',
)

const entries = computed(() =>
    [...(currentNode.value?.children ?? [])]
        .filter((child) => !child.name.startsWith('.'))
        .sort((a, b) =>
            a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true }),
        ),
)

const crumbs = computed(() => {
    const parts = currentPath.value.split('/').filter(Boolean)
    return [
        { label: 'Macintosh HD', path: '/' },
        ...parts.map((part, index) => ({
            label: part,
            path: `/${parts.slice(0, index + 1).join('/')}`,
        })),
    ]
})

const dateFormatter = computed(
    () => new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium', timeStyle: 'short' }),
)

function formatDate(iso: string) {
    if (!iso) return ''
    return dateFormatter.value.format(new Date(iso))
}

function isSelectable(node: FsNode) {
    return mode.value === 'browse' || node.kind === 'dir'
}

function navigate(path: string, record = true) {
    if (path === currentPath.value) return
    if (record) {
        backStack.value = [...backStack.value, currentPath.value]
        forwardStack.value = []
    }
    currentPath.value = path
    selectedName.value = null
    void nextTick(() => listRef.value?.scrollTo({ top: 0 }))
}

function goBack() {
    const previous = backStack.value.at(-1)
    if (!previous) return
    backStack.value = backStack.value.slice(0, -1)
    forwardStack.value = [currentPath.value, ...forwardStack.value]
    navigate(previous, false)
}

function goForward() {
    const next = forwardStack.value[0]
    if (!next) return
    forwardStack.value = forwardStack.value.slice(1)
    backStack.value = [...backStack.value, currentPath.value]
    navigate(next, false)
}

function childPath(node: FsNode) {
    return currentPath.value === '/' ? `/${node.name}` : `${currentPath.value}/${node.name}`
}

function select(node: FsNode) {
    if (!isSelectable(node)) return
    selectedName.value = node.name
}

function enter(node: FsNode) {
    if (node.kind === 'dir') navigate(childPath(node))
}

function finish(path: string | null) {
    open.value = false
    const resolve = resolvePick
    resolvePick = null
    resolve?.(path)
}

function confirm() {
    if (mode.value === 'browse') {
        finish(null)
        return
    }
    const selected = entries.value.find((entry) => entry.name === selectedName.value)
    finish(selected && selected.kind === 'dir' ? childPath(selected) : currentPath.value)
}

function moveSelection(delta: number) {
    const selectable = entries.value.filter(isSelectable)
    if (selectable.length === 0) return
    const index = selectable.findIndex((entry) => entry.name === selectedName.value)
    const next =
        selectable[Math.min(selectable.length - 1, Math.max(0, index < 0 ? 0 : index + delta))]
    selectedName.value = next.name
}

function onKeydown(event: KeyboardEvent) {
    if (!open.value) return
    if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        finish(null)
    } else if (event.key === 'Enter') {
        event.preventDefault()
        confirm()
    } else if (event.key === 'ArrowDown' && (event.metaKey || event.ctrlKey)) {
        event.preventDefault()
        const selected = entries.value.find((entry) => entry.name === selectedName.value)
        if (selected) enter(selected)
    } else if (event.key === 'ArrowUp' && (event.metaKey || event.ctrlKey)) {
        event.preventDefault()
        const parent = crumbs.value.at(-2)
        if (parent) navigate(parent.path)
    } else if (event.key === 'ArrowDown') {
        event.preventDefault()
        moveSelection(1)
    } else if (event.key === 'ArrowUp') {
        event.preventDefault()
        moveSelection(-1)
    }
}

async function show(nextMode: 'pick' | 'browse', path: string) {
    root.value = await loadMacFileSystem()
    mode.value = nextMode
    backStack.value = []
    forwardStack.value = []
    currentPath.value = findNode(root.value, path) ? path : HOME
    selectedName.value = null
    open.value = true
    window.addEventListener('keydown', onKeydown, true)
}

/** `pickFolder()`: resolves with the chosen folder, or `null` when cancelled. */
function pick(): Promise<string | null> {
    resolvePick?.(null)
    return new Promise((resolve) => {
        resolvePick = resolve
        void show('pick', APP_SUPPORT)
    })
}

/** `openPath()`: shows the folder in a Finder-style window. */
function browse(path: string) {
    resolvePick?.(null)
    resolvePick = null
    void show('browse', path)
}

function onAfterLeave() {
    window.removeEventListener('keydown', onKeydown, true)
}

onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown, true)
    resolvePick?.(null)
    resolvePick = null
})

defineExpose({ pick, browse })
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <Transition
            enter-active-class="transition duration-150 ease-out"
            enter-from-class="opacity-0 scale-[0.98]"
            leave-active-class="transition duration-100 ease-in"
            leave-to-class="opacity-0 scale-[0.98]"
            @after-leave="onAfterLeave"
        >
            <div
                v-if="open"
                class="folder-panel-backdrop fixed inset-0 flex items-center justify-center"
            >
                <div
                    role="dialog"
                    aria-modal="true"
                    :aria-label="currentName"
                    class="flex h-[460px] w-[760px] flex-col overflow-hidden rounded-xl border border-solid border-surface-5 bg-surface-3 shadow-2xl"
                >
                    <div
                        class="flex h-12 shrink-0 items-center gap-2 border-0 border-b border-solid border-surface-5 px-3"
                    >
                        <button
                            v-if="mode === 'browse'"
                            class="mr-2 size-3 rounded-full bg-red"
                            :aria-label="formatMessage(messages.cancel)"
                            @click="finish(null)"
                        />
                        <Button
                            v-tooltip="formatMessage(messages.back)"
                            type="quiet"
                            size="sm"
                            icon-only
                            :aria-label="formatMessage(messages.back)"
                            :disabled="backStack.length === 0"
                            @click="goBack"
                            ><ChevronLeftIcon
                        /></Button>
                        <Button
                            v-tooltip="formatMessage(messages.forward)"
                            type="quiet"
                            size="sm"
                            icon-only
                            :aria-label="formatMessage(messages.forward)"
                            :disabled="forwardStack.length === 0"
                            @click="goForward"
                            ><ChevronRightIcon
                        /></Button>
                        <span
                            class="ml-1 flex min-w-0 items-center gap-2 font-semibold text-[var(--color-text-primary)]"
                        >
                            <FolderIcon class="size-4 shrink-0 text-[var(--color-text-tertiary)]" />
                            <span class="truncate">{{ currentName }}</span>
                        </span>
                    </div>

                    <div class="flex min-h-0 flex-1">
                        <nav
                            class="flex w-44 shrink-0 flex-col gap-0.5 border-0 border-r border-solid border-surface-5 bg-surface-2 p-2"
                        >
                            <span
                                class="px-2 pb-1 pt-1 text-xs font-semibold text-[var(--color-text-tertiary)]"
                                >{{ formatMessage(messages.favorites) }}</span
                            >
                            <button
                                v-for="favorite in favorites"
                                :key="favorite.path"
                                class="flex items-center gap-2 rounded-md px-2 py-1 text-left text-sm"
                                :class="
                                    currentPath === favorite.path
                                        ? 'bg-surface-4 text-[var(--color-text-primary)]'
                                        : 'text-[var(--color-text-default)] hover:bg-surface-4'
                                "
                                @click="navigate(favorite.path)"
                            >
                                <component :is="favorite.icon" class="size-4 shrink-0 text-brand" />
                                <span class="truncate">{{ favorite.label }}</span>
                            </button>
                        </nav>

                        <div class="flex min-w-0 flex-1 flex-col">
                            <div
                                class="grid shrink-0 grid-cols-[minmax(0,1fr)_12rem] gap-4 border-0 border-b border-solid border-surface-5 px-4 py-1.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                            >
                                <span>{{ formatMessage(messages.name) }}</span>
                                <span>{{ formatMessage(messages.dateModified) }}</span>
                            </div>
                            <div ref="listRef" class="min-h-0 flex-1 overflow-y-auto px-2 py-1">
                                <div
                                    v-if="entries.length === 0"
                                    class="flex h-full items-center justify-center text-sm text-[var(--color-text-tertiary)]"
                                >
                                    {{ formatMessage(messages.emptyFolder) }}
                                </div>
                                <button
                                    v-for="entry in entries"
                                    :key="entry.name"
                                    class="grid w-full grid-cols-[minmax(0,1fr)_12rem] items-center gap-4 rounded-md px-2 py-1 text-left text-sm"
                                    :class="[
                                        selectedName === entry.name
                                            ? 'bg-brand text-brand-inverted'
                                            : isSelectable(entry)
                                              ? 'text-[var(--color-text-primary)] hover:bg-surface-4'
                                              : 'cursor-default text-[var(--color-text-tertiary)] opacity-60',
                                    ]"
                                    @click="select(entry)"
                                    @dblclick="enter(entry)"
                                >
                                    <span class="flex min-w-0 items-center gap-2">
                                        <FolderIcon
                                            v-if="entry.kind === 'dir'"
                                            class="size-4 shrink-0"
                                            :class="selectedName === entry.name ? '' : 'text-brand'"
                                        />
                                        <FileIcon v-else class="size-4 shrink-0" />
                                        <span class="truncate">{{ entry.name }}</span>
                                    </span>
                                    <span
                                        class="truncate text-xs"
                                        :class="
                                            selectedName === entry.name
                                                ? ''
                                                : 'text-[var(--color-text-tertiary)]'
                                        "
                                        >{{ formatDate(entry.modified) }}</span
                                    >
                                </button>
                            </div>
                            <div
                                class="flex shrink-0 items-center gap-1 overflow-hidden border-0 border-t border-solid border-surface-5 px-3 py-1.5 text-xs text-[var(--color-text-tertiary)]"
                            >
                                <template v-for="(crumb, index) in crumbs" :key="crumb.path">
                                    <ChevronRightIcon v-if="index > 0" class="size-3 shrink-0" />
                                    <button
                                        class="flex shrink-0 items-center gap-1 rounded px-1 hover:text-[var(--color-text-primary)]"
                                        @click="navigate(crumb.path)"
                                    >
                                        <FolderIcon v-if="index > 0" class="size-3" />
                                        {{ crumb.label }}
                                    </button>
                                </template>
                            </div>
                        </div>
                    </div>

                    <div
                        v-if="mode === 'pick'"
                        class="flex shrink-0 items-center justify-end gap-2 border-0 border-t border-solid border-surface-5 px-4 py-3"
                    >
                        <Button type="outlined" @click="finish(null)">{{
                            formatMessage(messages.cancel)
                        }}</Button>
                        <Button type="colored" color="brand" @click="confirm">{{
                            formatMessage(messages.open)
                        }}</Button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.folder-panel-backdrop {
    z-index: 400;
}
</style>
