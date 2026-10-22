<template>
    <div class="flex w-full min-w-0 flex-col gap-2">
        <div
            class="flex w-full min-w-0 flex-col rounded-[20px] border border-solid border-surface-4 shadow-sm overflow-clip"
        >
            <div
                class="flex h-10 w-full min-w-0 select-none flex-row items-center justify-between bg-surface-3 px-3 text-sm font-medium"
            >
                <div class="flex min-w-0 flex-1 items-center gap-2">
                    <Checkbox
                        :model-value="allVisibleSelected"
                        :indeterminate="someVisibleSelected && !allVisibleSelected"
                        :disabled="visibleSelectableEntries.length === 0"
                        :description="formatMessage(messages.selectView)"
                        @update:model-value="toggleAllVisible"
                    />
                    <button
                        type="button"
                        class="flex min-w-0 appearance-none items-center gap-1.5 border-0 bg-transparent p-0 font-semibold hover:text-[var(--color-text-default)]"
                        :class="
                            sortField === 'name'
                                ? 'text-[var(--color-text-primary)]'
                                : 'text-[var(--color-text-tertiary)]'
                        "
                        @click="handleSort('name')"
                    >
                        <span class="min-w-0 truncate">{{ formatMessage(messages.name) }}</span>
                        <ChevronUpIcon
                            v-if="sortField === 'name' && !sortDesc"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                        <ChevronDownIcon
                            v-if="sortField === 'name' && sortDesc"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                    </button>
                </div>
                <div class="ml-2 flex shrink-0 items-center gap-4">
                    <button
                        type="button"
                        class="hidden w-[92px] appearance-none items-center gap-1 border-0 bg-transparent p-0 text-left font-semibold hover:text-[var(--color-text-default)] sm:flex"
                        :class="
                            sortField === 'size'
                                ? 'text-[var(--color-text-primary)]'
                                : 'text-[var(--color-text-tertiary)]'
                        "
                        @click="handleSort('size')"
                    >
                        <span>{{ formatMessage(messages.size) }}</span>
                        <ChevronUpIcon
                            v-if="sortField === 'size' && !sortDesc"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                        <ChevronDownIcon
                            v-if="sortField === 'size' && sortDesc"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                    </button>
                    <button
                        type="button"
                        class="hidden w-[132px] appearance-none items-center gap-1 border-0 bg-transparent p-0 text-left font-semibold hover:text-[var(--color-text-default)] sm:flex"
                        :class="
                            sortField === 'modified'
                                ? 'text-[var(--color-text-primary)]'
                                : 'text-[var(--color-text-tertiary)]'
                        "
                        @click="handleSort('modified')"
                    >
                        <span>{{ formatMessage(messages.modified) }}</span>
                        <ChevronUpIcon
                            v-if="sortField === 'modified' && !sortDesc"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                        <ChevronDownIcon
                            v-if="sortField === 'modified' && sortDesc"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                    </button>
                    <span class="size-4 shrink-0" aria-hidden="true" />
                </div>
            </div>
            <div
                v-if="!isHomePath"
                role="button"
                tabindex="0"
                class="group flex w-full min-w-0 cursor-pointer select-none items-center gap-2 border-0 border-t border-solid border-surface-4 bg-surface-2 px-3 py-2 text-left hover:bg-surface-2.5 focus:!outline-none"
                @click="navigateTo(parentPath)"
                @keydown="(event) => event.key === 'Enter' && navigateTo(parentPath)"
            >
                <span class="size-5 shrink-0" aria-hidden="true" />
                <div
                    class="flex size-4 shrink-0 items-center justify-center text-[var(--color-text-tertiary)]"
                >
                    <UndoIcon
                        class="size-4 group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                    />
                </div>
                <span
                    class="min-w-0 flex-1 truncate text-sm font-medium text-[var(--color-text-default)] group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.parentFolder) }}
                </span>
                <div class="ml-2 flex shrink-0 items-center gap-4">
                    <span
                        class="hidden w-[92px] text-left text-sm text-[var(--color-text-tertiary)] sm:block"
                    />
                    <span
                        class="hidden w-[132px] text-left text-sm text-[var(--color-text-tertiary)] sm:block"
                    />
                    <span class="size-4 shrink-0" aria-hidden="true" />
                </div>
            </div>
            <div class="max-h-[40vh] overflow-y-auto">
                <div ref="listContainer">
                    <div
                        v-if="topSpacerHeight > 0"
                        :style="{ height: `${topSpacerHeight}px` }"
                        aria-hidden="true"
                    />
                    <div
                        v-if="entries.length === 0"
                        class="flex items-center gap-2 bg-surface-2 px-3 py-2 text-sm text-[var(--color-text-tertiary)]"
                    >
                        <FileIcon class="size-4 shrink-0" />
                        <span>{{
                            formatMessage(
                                searching || selectedOnly
                                    ? messages.noMatches
                                    : messages.emptyFolderTitle,
                            )
                        }}</span>
                    </div>
                    <div
                        v-for="(entry, i) in visibleItems"
                        :key="`${entry.type}:${entry.path}`"
                        role="button"
                        tabindex="0"
                        class="group box-border flex h-10 w-full min-w-0 select-none items-center gap-2 border-0 border-t border-solid border-surface-4 px-3 py-2 text-left first:border-t-0 focus:!outline-none"
                        :class="[
                            getRowBackgroundClass(visibleRange.start + i + (isHomePath ? 0 : 1)),
                            entry.disabled && entry.type === 'file'
                                ? 'cursor-not-allowed opacity-50'
                                : 'cursor-pointer hover:bg-surface-2.5',
                        ]"
                        @click="selectEntry(entry)"
                        @keydown="(event) => event.key === 'Enter' && selectEntry(entry)"
                    >
                        <Checkbox
                            class="shrink-0"
                            :model-value="entry.checked"
                            :indeterminate="entry.indeterminate"
                            :disabled="entry.disabled"
                            :description="entry.name"
                            @click.stop
                            @update:model-value="toggleEntry(entry, $event)"
                        />
                        <div
                            class="flex size-4 shrink-0 items-center justify-center text-[var(--color-text-tertiary)]"
                        >
                            <component
                                :is="entry.icon"
                                class="size-4 group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                            />
                        </div>
                        <span
                            v-tooltip="{ content: entry.name, onlyWhenTruncated: true }"
                            class="min-w-0 flex-1 truncate text-sm font-medium text-[var(--color-text-default)] group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                        >
                            {{ entry.name }}
                        </span>
                        <div class="ml-2 flex shrink-0 items-center gap-4">
                            <span
                                class="hidden w-[92px] truncate text-left text-sm text-[var(--color-text-tertiary)] sm:block"
                            >
                                {{ formatSize(entry) }}
                            </span>
                            <span
                                class="hidden w-[132px] truncate text-left text-sm text-[var(--color-text-tertiary)] sm:block"
                            >
                                {{ formatModified(entry) }}
                            </span>
                            <ChevronRightIcon
                                class="size-4 shrink-0 text-[var(--color-text-tertiary)] group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                                :class="{ invisible: entry.type !== 'directory' }"
                                aria-hidden="true"
                            />
                        </div>
                    </div>
                    <div
                        v-if="bottomSpacerHeight > 0"
                        :style="{ height: `${bottomSpacerHeight}px` }"
                        aria-hidden="true"
                    />
                    <div
                        v-for="row in fillerRowCount"
                        :key="`filler:${row}`"
                        class="flex w-full min-w-0 select-none items-center gap-2 border-0 border-t border-solid border-surface-4 px-3 py-2 text-left"
                        :class="getRowBackgroundClass(visibleRowCount + row - 1)"
                        aria-hidden="true"
                    >
                        <span class="size-5 shrink-0" />
                        <span class="size-4 shrink-0" />
                        <span class="min-w-0 flex-1 truncate text-sm font-medium opacity-0">.</span>
                        <div class="ml-2 flex shrink-0 items-center gap-4">
                            <span class="hidden w-[92px] text-left text-sm sm:block" />
                            <span class="hidden w-[132px] text-left text-sm sm:block" />
                            <span class="size-4 shrink-0" />
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import {
    ChevronDownIcon,
    ChevronRightIcon,
    ChevronUpIcon,
    FileIcon,
    UndoIcon,
} from '@modrinth/assets'
import { type Component, computed, nextTick, ref, watch } from 'vue'

import { useFormatBytes } from '../../composables/format-bytes'
import { useFormatDateTime } from '../../composables/format-date-time'
import { defineMessages, useVIntl } from '../../composables/i18n'
import { findScrollableAncestor, useVirtualScroll } from '../../composables/virtual-scroll'
import { getDirectoryIcon, getFileIcon } from '../../utils/auto-icons'
import Checkbox from './Checkbox.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    selectView: { id: 'files.select-view', defaultMessage: 'Select all files in this view' },
    name: {
        id: 'files.table-header.name',
        defaultMessage: 'Name',
    },
    size: {
        id: 'files.table-header.size',
        defaultMessage: 'Size',
    },
    modified: {
        id: 'files.table-header.modified',
        defaultMessage: 'Modified',
    },
    itemCount: {
        id: 'files.row.item-count',
        defaultMessage: '{count, plural, one {# item} other {# items}}',
    },
    parentFolder: {
        id: 'files.row.parent-folder',
        defaultMessage: 'Parent folder',
    },
    emptyFolderTitle: {
        id: 'files.layout.empty-folder-title',
        defaultMessage: 'This folder is empty',
    },
    noMatches: { id: 'files.layout.no-matches', defaultMessage: 'No files match this view' },
})

export type FileTreeSelectItem = {
    path: string
    type?: 'directory' | 'file'
    disabled?: boolean
    size?: number
    modified?: number
    count?: number
    name?: string
}

type NormalizedFileTreeSelectItem = FileTreeSelectItem & {
    name: string
    normalizedPath: string
}

type FileTreeSelectEntry = {
    path: string
    name: string
    type: 'directory' | 'file'
    icon: Component
    checked: boolean
    indeterminate: boolean
    disabled: boolean
    size?: number
    modified?: number
    count?: number
    item?: NormalizedFileTreeSelectItem
}

type FileTreeSelectSortField = 'name' | 'size' | 'modified'

const formatBytes = useFormatBytes()
const formatDateTime = useFormatDateTime({
    year: '2-digit',
    month: '2-digit',
    day: '2-digit',
    hour: 'numeric',
    minute: 'numeric',
})

const props = withDefaults(
    defineProps<{
        items: FileTreeSelectItem[]
        modelValue: string[]
        currentPath?: string
        searchQuery?: string
        selectedOnly?: boolean
    }>(),
    {
        items: () => [],
        modelValue: () => [],
        currentPath: '',
        searchQuery: '',
        selectedOnly: false,
    },
)

const emit = defineEmits<{
    (e: 'update:modelValue', value: string[]): void
    (e: 'navigate' | 'update:currentPath', path: string): void
}>()

const localPath = ref(props.currentPath)
watch(
    () => props.currentPath,
    (value) => {
        localPath.value = value
    },
)
const activePath = computed({
    get: () => localPath.value,
    set: (value) => {
        localPath.value = value
        emit('update:currentPath', value)
    },
})
const searching = computed(() => props.searchQuery.trim().length > 0)
const sortField = ref<FileTreeSelectSortField>('name')
const sortDesc = ref(false)
const isHomePath = computed(() => searching.value || activePath.value === '')
const parentPath = computed(() => activePath.value.split('/').slice(0, -1).join('/'))
const initialMinimumRowCount = ref(0)

const normalizedItems = computed(() => {
    const items = new Map<string, NormalizedFileTreeSelectItem>()
    for (const item of props.items) {
        const normalizedPath = normalizePath(item.path)
        if (!normalizedPath) continue
        items.set(normalizedPath, {
            ...item,
            path: item.path,
            name: item.name ?? getName(normalizedPath),
            normalizedPath,
        })
    }
    return [...items.values()]
})

const selectedPaths = computed(() => new Set(props.modelValue.map((path) => normalizePath(path))))
const folderIndex = computed(() => {
    const descendants = new Map<string, NormalizedFileTreeSelectItem[]>()
    const children = new Map<string, Set<string>>()
    for (const item of normalizedItems.value) {
        const segments = item.normalizedPath.split('/')
        for (let depth = 1; depth < segments.length; depth++) {
            const path = segments.slice(0, depth).join('/')
            const entries = descendants.get(path) ?? []
            entries.push(item)
            descendants.set(path, entries)
            const names = children.get(path) ?? new Set<string>()
            names.add(segments[depth])
            children.set(path, names)
        }
    }
    return { descendants, children }
})

const folderPaths = computed(() => {
    const paths = new Set<string>()
    for (const item of normalizedItems.value) {
        const segments = item.normalizedPath.split('/')
        for (let i = 1; i < segments.length; i++) {
            paths.add(segments.slice(0, i).join('/'))
        }
        if (item.type === 'directory') {
            paths.add(item.normalizedPath)
        }
    }
    return paths
})

const entries = computed<FileTreeSelectEntry[]>(() => {
    const directories = new Map<string, FileTreeSelectEntry>()
    const files: FileTreeSelectEntry[] = []
    const currentSegments = activePath.value ? activePath.value.split('/') : []

    for (const item of normalizedItems.value) {
        if (searching.value) {
            if (
                item.type === 'directory' ||
                !item.normalizedPath
                    .toLocaleLowerCase()
                    .includes(props.searchQuery.trim().toLocaleLowerCase())
            )
                continue
            if (props.selectedOnly && !selectedPaths.value.has(item.normalizedPath)) continue
            files.push({ ...buildFileEntry(item), name: item.normalizedPath })
            continue
        }
        if (
            props.selectedOnly &&
            (item.type === 'directory' || !selectedPaths.value.has(item.normalizedPath))
        )
            continue
        const segments = item.normalizedPath.split('/')
        if (!isInCurrentPath(segments, currentSegments)) continue

        const remaining = segments.slice(currentSegments.length)
        if (remaining.length > 1) {
            const directoryName = remaining[0]
            const directoryPath = [...currentSegments, directoryName].join('/')
            if (!directories.has(directoryPath)) {
                directories.set(directoryPath, buildDirectoryEntry(directoryPath, directoryName))
            }
        } else if (remaining.length === 1) {
            if (item.type === 'directory') {
                directories.set(
                    item.normalizedPath,
                    buildDirectoryEntry(item.normalizedPath, item.name, item),
                )
            } else {
                files.push(buildFileEntry(item))
            }
        }
    }

    return [...directories.values(), ...files].sort(compareEntries)
})

const visibleSelectableEntries = computed(() => entries.value.filter((entry) => !entry.disabled))
const virtualized = computed(() => entries.value.length > 100)
const {
    listContainer,
    visibleItems,
    visibleRange,
    totalHeight,
    visibleTop: topSpacerHeight,
    syncScrollState,
} = useVirtualScroll(entries, { itemHeight: 40, enabled: virtualized })
const bottomSpacerHeight = computed(() =>
    virtualized.value
        ? Math.max(0, totalHeight.value - topSpacerHeight.value - visibleItems.value.length * 40)
        : 0,
)
watch(
    [activePath, () => props.searchQuery, () => props.selectedOnly, sortField, sortDesc],
    async () => {
        await nextTick()
        findScrollableAncestor(listContainer.value).scrollTo({ top: 0 })
        syncScrollState()
    },
)

const visibleSelectablePaths = computed(() => {
    const paths = new Set<string>()
    for (const entry of visibleSelectableEntries.value) {
        for (const path of getEntrySelectablePaths(entry)) {
            paths.add(path)
        }
    }
    return [...paths]
})

const allVisibleSelected = computed(
    () =>
        visibleSelectablePaths.value.length > 0 &&
        visibleSelectablePaths.value.every((path) => selectedPaths.value.has(path)),
)

const someVisibleSelected = computed(() =>
    visibleSelectablePaths.value.some((path) => selectedPaths.value.has(path)),
)

const visibleRowCount = computed(
    () => entries.value.length + (isHomePath.value ? 0 : 1) + (entries.value.length === 0 ? 1 : 0),
)

const fillerRowCount = computed(() =>
    Math.max(0, Math.min(10, initialMinimumRowCount.value) - visibleRowCount.value),
)

watch(
    normalizedItems,
    () => {
        if (activePath.value && !folderPaths.value.has(activePath.value)) {
            activePath.value = ''
        }
    },
    { immediate: true },
)

watch(
    visibleRowCount,
    (rowCount) => {
        if (isHomePath.value && rowCount > 1 && initialMinimumRowCount.value === 0) {
            initialMinimumRowCount.value = rowCount
        }
    },
    { immediate: true },
)

watch(
    isHomePath,
    () => {
        if (isHomePath.value && visibleRowCount.value > 1 && initialMinimumRowCount.value === 0) {
            initialMinimumRowCount.value = visibleRowCount.value
        }
    },
    { flush: 'post' },
)

function normalizePath(path: string) {
    return path.replaceAll('\\', '/').split('/').filter(Boolean).join('/')
}

function getName(path: string) {
    return path.split('/').pop() ?? path
}

function isInCurrentPath(segments: string[], currentSegments: string[]) {
    if (segments.length <= currentSegments.length) return false
    return currentSegments.every((segment, index) => segments[index] === segment)
}

function handleSort(field: FileTreeSelectSortField) {
    if (sortField.value === field) {
        sortDesc.value = !sortDesc.value
    } else {
        sortField.value = field
        sortDesc.value = false
    }
}

function getRowBackgroundClass(index: number) {
    return index % 2 === 0 ? 'bg-surface-2' : 'bg-surface-1.5'
}

function compareEntries(a: FileTreeSelectEntry, b: FileTreeSelectEntry) {
    if (a.type !== b.type) return a.type === 'directory' ? -1 : 1

    switch (sortField.value) {
        case 'modified':
            return sortDesc.value
                ? getModifiedSortValue(a) - getModifiedSortValue(b)
                : getModifiedSortValue(b) - getModifiedSortValue(a)
        case 'size':
            return sortDesc.value
                ? getSizeSortValue(a) - getSizeSortValue(b)
                : getSizeSortValue(b) - getSizeSortValue(a)
        default:
            return sortDesc.value
                ? b.name.localeCompare(a.name, undefined, { numeric: true, sensitivity: 'base' })
                : a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
    }
}

function getSizeSortValue(entry: FileTreeSelectEntry) {
    return entry.type === 'directory' ? (entry.count ?? 0) : (entry.size ?? 0)
}

function getModifiedSortValue(entry: FileTreeSelectEntry) {
    return entry.modified ?? 0
}

function getFolderDescendants(path: string) {
    return folderIndex.value.descendants.get(path) ?? []
}

function getFolderChildCount(path: string) {
    return folderIndex.value.children.get(path)?.size ?? 0
}

function getLatestModified(items: NormalizedFileTreeSelectItem[]) {
    const modified = items
        .map((item) => item.modified)
        .filter((value): value is number => typeof value === 'number' && Number.isFinite(value))

    if (modified.length === 0) return undefined
    return modified.reduce((latest, value) => Math.max(latest, value), 0)
}

function buildDirectoryEntry(
    path: string,
    name: string,
    item?: NormalizedFileTreeSelectItem,
): FileTreeSelectEntry {
    const descendants = getFolderDescendants(path).filter(
        (item) =>
            !item.disabled &&
            item.type !== 'directory' &&
            (!props.selectedOnly || selectedPaths.value.has(item.normalizedPath)),
    )
    const selectedCount = descendants.filter((item) =>
        selectedPaths.value.has(item.normalizedPath),
    ).length

    return {
        path,
        name,
        type: 'directory',
        icon: getDirectoryIcon(name),
        checked: descendants.length > 0 && selectedCount === descendants.length,
        indeterminate: selectedCount > 0 && selectedCount < descendants.length,
        disabled: !!item?.disabled || descendants.length === 0,
        modified: item?.modified ?? getLatestModified(descendants),
        count: item?.count ?? getFolderChildCount(path),
        item,
    }
}

function buildFileEntry(item: NormalizedFileTreeSelectItem): FileTreeSelectEntry {
    return {
        path: item.normalizedPath,
        name: item.name,
        type: 'file',
        icon: getFileIcon(item.name),
        checked: selectedPaths.value.has(item.normalizedPath),
        indeterminate: false,
        disabled: item.disabled ?? false,
        size: item.size,
        modified: item.modified,
        item,
    }
}

function navigateTo(path: string) {
    activePath.value = path
    emit('navigate', path)
}

function selectEntry(entry: FileTreeSelectEntry) {
    if (entry.type === 'directory') {
        navigateTo(entry.path)
    } else {
        toggleEntry(entry, !entry.checked)
    }
}

function toggleEntry(entry: FileTreeSelectEntry, selected: boolean) {
    if (entry.disabled) return

    const nextSelectedPaths = new Set(selectedPaths.value)
    const paths = getEntrySelectablePaths(entry)

    for (const path of paths) {
        if (selected) {
            nextSelectedPaths.add(path)
        } else {
            nextSelectedPaths.delete(path)
        }
    }

    emit('update:modelValue', [...nextSelectedPaths])
}

function getEntrySelectablePaths(entry: FileTreeSelectEntry) {
    if (entry.type === 'directory') {
        return getFolderDescendants(entry.path)
            .filter(
                (item) =>
                    !item.disabled &&
                    item.type !== 'directory' &&
                    (!props.selectedOnly || selectedPaths.value.has(item.normalizedPath)),
            )
            .map((item) => item.normalizedPath)
    }

    return [entry.path]
}

function toggleAllVisible(selected: boolean) {
    const nextSelectedPaths = new Set(selectedPaths.value)
    for (const path of visibleSelectablePaths.value) {
        if (selected) {
            nextSelectedPaths.add(path)
        } else {
            nextSelectedPaths.delete(path)
        }
    }

    emit('update:modelValue', [...nextSelectedPaths])
}

function formatSize(entry: FileTreeSelectEntry) {
    if (entry.type === 'directory') {
        return formatMessage(messages.itemCount, { count: entry.count ?? 0 })
    }

    if (entry.size === undefined) return ''
    return formatBytes(entry.size)
}

function formatModified(entry: FileTreeSelectEntry) {
    if (entry.modified === undefined) return ''
    return formatDateTime(new Date(entry.modified * 1000))
}
</script>
