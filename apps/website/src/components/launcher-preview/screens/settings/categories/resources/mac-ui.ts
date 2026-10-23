import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import { readSiteTheme } from '../../../../lib/state/settings'
import { messages } from '../../messages'
import {
    canonicalize,
    type FsNode,
    HOME,
    isDirectory,
    joinPath,
    type MacFileSystem,
    parentPath,
    sortedChildren,
} from './mac-fs'

export type MacIconName =
    | 'folder'
    | 'document'
    | 'image'
    | 'disk-image'
    | 'app'
    | 'drive'
    | 'applications'
    | 'desktop'
    | 'documents'
    | 'downloads'
    | 'home'
    | 'chevron-left'
    | 'chevron-right'
    | 'chevron-up-down'
    | 'chevron-down'
    | 'search'

/* The preview Mac's own interface: Finder and the open panel follow the system
   appearance (the website's theme stands in for it), not the launcher theme. */
export function macAppearance(): 'dark' | 'light' {
    return readSiteTheme() === 'light' ? 'light' : 'dark'
}

type Format = (descriptor: MessageDescriptor, values?: Record<string, unknown>) => string

/** Finder's localized names of the system folders. */
const SPECIAL_FOLDERS: Record<string, MessageDescriptor> = {
    '/': messages.launcherPreviewSettingsMacMacintoshHd,
    '/Applications': messages.launcherPreviewSettingsMacApplications,
    '/Library': messages.launcherPreviewSettingsMacLibrary,
    '/System': messages.launcherPreviewSettingsMacSystem,
    '/Users': messages.launcherPreviewSettingsMacUsers,
    '/Users/Shared': messages.launcherPreviewSettingsMacShared,
    [`${HOME}/Applications`]: messages.launcherPreviewSettingsMacApplications,
    [`${HOME}/Desktop`]: messages.launcherPreviewSettingsMacDesktop,
    [`${HOME}/Documents`]: messages.launcherPreviewSettingsMacDocuments,
    [`${HOME}/Downloads`]: messages.launcherPreviewSettingsMacDownloads,
    [`${HOME}/Library`]: messages.launcherPreviewSettingsMacLibrary,
    [`${HOME}/Movies`]: messages.launcherPreviewSettingsMacMovies,
    [`${HOME}/Music`]: messages.launcherPreviewSettingsMacMusic,
    [`${HOME}/Pictures`]: messages.launcherPreviewSettingsMacPictures,
    [`${HOME}/Public`]: messages.launcherPreviewSettingsMacPublic,
}

/** The name Finder shows for an item: localized system folders, apps without `.app`. */
export function displayName(path: string, format: Format): string {
    const special = SPECIAL_FOLDERS[path]
    if (special) return format(special)
    const name = path.split('/').filter(Boolean).pop() ?? ''
    return name.endsWith('.app') ? name.slice(0, -4) : name
}

function sameDay(a: Date, b: Date) {
    return (
        a.getFullYear() === b.getFullYear() &&
        a.getMonth() === b.getMonth() &&
        a.getDate() === b.getDate()
    )
}

/** Finder's "Date Modified" column. */
export function formatFinderDate(iso: string, locale: string, format: Format): string {
    const date = new Date(iso)
    const now = new Date()
    const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1)
    const time = new Intl.DateTimeFormat(locale, { hour: 'numeric', minute: '2-digit' }).format(
        date,
    )
    if (sameDay(date, now)) return format(messages.launcherPreviewSettingsMacToday, { time })
    if (sameDay(date, yesterday))
        return format(messages.launcherPreviewSettingsMacYesterday, { time })
    const day = new Intl.DateTimeFormat(locale, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
    }).format(date)
    return format(messages.launcherPreviewSettingsMacDateTime, { date: day, time })
}

/** Finder's "Size" column (decimal units, `--` for folders). */
export function formatFinderSize(node: FsNode, locale: string, format: Format): string {
    if (node.kind === 'dir' && !node.package) return '--'
    const bytes = node.size
    if (bytes === 0) return format(messages.launcherPreviewSettingsMacZeroBytes)
    if (bytes < 1000) return format(messages.launcherPreviewSettingsMacBytes, { count: bytes })
    const units: [number, string, number][] = [
        [1e9, 'GB', 2],
        [1e6, 'MB', 1],
        [1e3, 'KB', 0],
    ]
    const [divisor, unit, digits] = units.find(([size]) => bytes >= size) ?? units[2]
    const value = new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(
        bytes / divisor,
    )
    return `${value} ${unit}`
}

export type SortKey = 'name' | 'modified' | 'size'

export interface MacListEntry {
    node: FsNode
    path: string
    /** Not choosable in this panel (files in a folder picker, filtered-out files). */
    disabled: boolean
}

/** The options of `MacOpenPanel.pick`, after `open()` of `@tauri-apps/plugin-dialog`. */
export interface MacOpenOptions {
    /** The dialog title, shown as the panel's message. */
    title?: string
    /** Choose a folder (the default) rather than a file. */
    directory?: boolean
    /** Where the panel opens; it remembers its last folder otherwise. */
    defaultPath?: string
    /** A file picker's filter: extensions without the dot. */
    extensions?: string[]
}

/** What a browser lets the user choose; read reactively, so a panel can switch it per request. */
export interface MacPickMode {
    /** A folder picker: files and bundles are greyed out. */
    directoriesOnly: boolean
    /** A file picker's extension filter (lower case, without the dot), or null for any file. */
    extensions: string[] | null
}

function isChoosable(node: FsNode, mode: MacPickMode): boolean {
    const folder = node.kind === 'dir' && !node.package
    if (mode.directoriesOnly) return folder
    if (folder || !mode.extensions) return true
    const extension = node.name.includes('.') ? node.name.split('.').pop()!.toLowerCase() : ''
    return mode.extensions.includes(extension)
}

/**
 * Navigation of a Finder-style browser: the current folder with back and
 * forward history, the selection, the search filter and the sort column.
 */
export function useMacBrowser(fs: MacFileSystem, mode: MacPickMode) {
    const path = ref(HOME)
    const backStack = ref<string[]>([])
    const forwardStack = ref<string[]>([])
    const selected = ref<string | null>(null)
    const search = ref('')
    const sortKey = ref<SortKey>('name')
    const sortAscending = ref(true)

    const current = computed(() =>
        fs.root ? (canonicalize(fs.root, path.value)?.node ?? null) : null,
    )

    const entries = computed<MacListEntry[]>(() => {
        const node = current.value
        if (!isDirectory(node)) return []
        const query = search.value.trim().toLowerCase()
        const list = sortedChildren(node)
            .filter((child) => fs.showHidden || !child.hidden)
            .filter((child) => !query || child.name.toLowerCase().includes(query))
            .map((child) => ({
                node: child,
                path: joinPath(path.value, child.name),
                disabled: !isChoosable(child, mode),
            }))
        if (sortKey.value === 'modified') {
            list.sort((a, b) => Date.parse(a.node.modified) - Date.parse(b.node.modified))
        } else if (sortKey.value === 'size') {
            list.sort((a, b) => a.node.size - b.node.size)
        }
        if (!sortAscending.value) list.reverse()
        return list
    })

    function go(target: string, record = true) {
        if (!fs.root) return
        const resolved = canonicalize(fs.root, target)
        if (!resolved || !isDirectory(resolved.node) || resolved.node.package) return
        if (resolved.path === path.value) return
        if (record) {
            backStack.value = [...backStack.value, path.value]
            forwardStack.value = []
        }
        path.value = resolved.path
        selected.value = null
        search.value = ''
    }

    function back() {
        const previous = backStack.value.at(-1)
        if (!previous) return
        backStack.value = backStack.value.slice(0, -1)
        forwardStack.value = [...forwardStack.value, path.value]
        const from = path.value
        go(previous, false)
        selected.value = from
    }

    function forward() {
        const next = forwardStack.value.at(-1)
        if (!next) return
        forwardStack.value = forwardStack.value.slice(0, -1)
        backStack.value = [...backStack.value, path.value]
        go(next, false)
    }

    function up() {
        if (path.value === '/') return
        const from = path.value
        go(parentPath(path.value))
        selected.value = from
    }

    /** Arrow-key selection over the choosable entries. */
    function moveSelection(delta: number) {
        const list = entries.value.filter((entry) => !entry.disabled)
        if (list.length === 0) return
        const index = list.findIndex((entry) => entry.path === selected.value)
        const next = index < 0 ? (delta > 0 ? 0 : list.length - 1) : index + delta
        selected.value = list[Math.max(0, Math.min(list.length - 1, next))].path
    }

    function toggleSort(key: SortKey) {
        if (sortKey.value === key) sortAscending.value = !sortAscending.value
        else {
            sortKey.value = key
            sortAscending.value = key === 'name'
        }
    }

    /** Starts over at `start`; a file (or bundle) opens its folder with the item selected. */
    function reset(start: string) {
        backStack.value = []
        forwardStack.value = []
        selected.value = null
        search.value = ''
        path.value = HOME
        const resolved = fs.root ? canonicalize(fs.root, start) : null
        if (!resolved) return
        if (isDirectory(resolved.node) && !resolved.node.package) {
            go(resolved.path, false)
            return
        }
        go(parentPath(resolved.path), false)
        selected.value = resolved.path
    }

    function select(entryPath: string | null) {
        selected.value = entryPath
    }

    function setSearch(value: string) {
        search.value = value
    }

    return {
        path,
        selected,
        search,
        select,
        setSearch,
        sortKey,
        sortAscending,
        current,
        entries,
        canBack: computed(() => backStack.value.length > 0),
        canForward: computed(() => forwardStack.value.length > 0),
        go,
        back,
        forward,
        up,
        moveSelection,
        toggleSort,
        reset,
    }
}

export type MacBrowser = ReturnType<typeof useMacBrowser>

/** The sidebar of Finder and the open panel. */
export const SIDEBAR_FAVORITES: { path: string; icon: MacIconName }[] = [
    { path: '/Applications', icon: 'applications' },
    { path: `${HOME}/Desktop`, icon: 'desktop' },
    { path: `${HOME}/Documents`, icon: 'documents' },
    { path: `${HOME}/Downloads`, icon: 'downloads' },
    { path: HOME, icon: 'home' },
]

/** Expands `~` the way Go to Folder does. */
export function expandHome(input: string): string {
    const trimmed = input.trim()
    if (trimmed === '~') return HOME
    if (trimmed.startsWith('~/')) return `${HOME}${trimmed.slice(1)}`
    return trimmed
}
