<script setup lang="ts">
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, ref, watch } from 'vue'

import { messages } from '../../messages'
import type { FinderApi } from './finder'
import {
    baseName,
    favoriteOf,
    favoritePath,
    FINDER_FAVORITES,
    type FinderFavorite,
    type FinderItem,
    pathSegments,
} from './finder-model'

/**
 * The Finder window macOS opens for the launcher's folder commands, in list
 * view with the path bar shown. It belongs to another application, so it
 * follows the system chrome rather than the launcher's theme tokens, and a
 * click on the launcher window behind it brings the launcher forward again.
 */
const props = defineProps<{ finder: FinderApi }>()

const { formatMessage, locale } = useVIntl()

const windowEl = ref<HTMLElement | null>(null)
const zoomed = ref(false)

const state = computed(() => props.finder.state)
const items = computed<FinderItem[]>(() => {
    // Re-list whenever the window is (re)opened or navigates.
    void state.value.opened
    return props.finder.items(state.value.path)
})

const favoriteLabels: Record<FinderFavorite, MessageDescriptor> = {
    Desktop: messages.launcherPreviewSettingsFinderDesktop,
    Documents: messages.launcherPreviewSettingsFinderDocuments,
    Downloads: messages.launcherPreviewSettingsFinderDownloads,
}

const favoriteIcons = {
    Desktop: MonitorIcon,
    Documents: FileTextIcon,
    Downloads: DownloadIcon,
}

/** Folder names macOS shows localized (the file system keeps the English ones). */
const localizedFolders: Record<string, MessageDescriptor> = {
    Users: messages.launcherPreviewSettingsFinderUsers,
    Library: messages.launcherPreviewSettingsFinderLibrary,
    ...favoriteLabels,
}

function folderLabel(name: string, depth: number) {
    // Only the standard folders in their standard places are localized.
    const standard = depth === 1 ? name === 'Users' : depth === 3
    const descriptor = standard ? localizedFolders[name] : undefined
    return descriptor ? formatMessage(descriptor) : name
}

const title = computed(() => {
    const segments = pathSegments(state.value.path)
    return folderLabel(baseName(state.value.path), segments.length)
})

const pathBar = computed(() => [
    formatMessage(messages.launcherPreviewSettingsFinderStartupDisk),
    ...pathSegments(state.value.path).map((segment, index) => folderLabel(segment, index + 1)),
])

const activeFavorite = computed(() => favoriteOf(state.value.path))

const kindLabels: Record<FinderItem['kind'], MessageDescriptor> = {
    log: messages.launcherPreviewSettingsFinderKindLog,
    zip: messages.launcherPreviewSettingsFinderKindZip,
}

/** Finder sizes: decimal units, whole kilobytes, one decimal from a megabyte up. */
function formatSize(bytes: number) {
    if (bytes < 1_000_000) return `${Math.max(1, Math.round(bytes / 1000))} KB`
    const value = bytes / 1_000_000
    const digits = value < 100 ? 1 : 0
    return `${value.toLocaleString(locale.value, {
        minimumFractionDigits: digits,
        maximumFractionDigits: digits,
    })} MB`
}

function startOfDay(time: number) {
    const date = new Date(time)
    date.setHours(0, 0, 0, 0)
    return date.getTime()
}

/** "Today at 11:42 AM", "Yesterday at …", then the medium date. */
function formatModified(time: number) {
    const clock = new Intl.DateTimeFormat(locale.value, {
        hour: 'numeric',
        minute: '2-digit',
    }).format(time)
    const today = startOfDay(Date.now())
    if (time >= today)
        return formatMessage(messages.launcherPreviewSettingsFinderTodayAt, { time: clock })
    if (time >= today - 24 * 60 * 60 * 1000) {
        return formatMessage(messages.launcherPreviewSettingsFinderYesterdayAt, { time: clock })
    }
    const date = new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium' }).format(time)
    return formatMessage(messages.launcherPreviewSettingsFinderDateAt, { date, time: clock })
}

function itemIcon(item: FinderItem) {
    return item.kind === 'zip' ? FileArchiveIcon : FileTextIcon
}

function moveSelection(delta: number) {
    const list = items.value
    if (!list.length) return
    const index = list.findIndex((item) => item.name === state.value.selected)
    const next =
        index === -1
            ? delta > 0
                ? 0
                : list.length - 1
            : Math.min(list.length - 1, Math.max(0, index + delta))
    props.finder.select(list[next].name)
}

function onKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        moveSelection(event.key === 'ArrowDown' ? 1 : -1)
    } else if (event.metaKey && event.key === '[') {
        event.preventDefault()
        props.finder.goBack()
    } else if (event.metaKey && event.key === ']') {
        event.preventDefault()
        props.finder.goForward()
    }
}

function minimize() {
    // Minimized to the Dock, out of the launcher window's way.
    props.finder.close()
}

watch(
    () => state.value.opened,
    async () => {
        await nextTick()
        windowEl.value?.focus({ preventScroll: true })
        if (!state.value.selected) return
        windowEl.value
            ?.querySelector<HTMLElement>('.settings-finder-row.is-selected')
            ?.scrollIntoView({ block: 'nearest' })
    },
)
</script>

<template>
    <Transition name="settings-finder">
        <div
            v-if="state.visible"
            class="settings-finder-layer"
            @pointerdown.self="props.finder.close()"
        >
            <div
                ref="windowEl"
                class="settings-finder-window"
                :class="{ 'is-zoomed': zoomed }"
                role="dialog"
                :aria-label="title"
                tabindex="-1"
                @keydown="onKeydown"
            >
                <aside class="settings-finder-sidebar">
                    <div class="settings-finder-traffic">
                        <button
                            type="button"
                            class="settings-finder-light is-close"
                            :aria-label="formatMessage(messages.launcherPreviewSettingsFinderClose)"
                            @click="props.finder.close()"
                        >
                            <svg viewBox="0 0 8 8" aria-hidden="true">
                                <path d="M1.5 1.5l5 5m0-5l-5 5" />
                            </svg>
                        </button>
                        <button
                            type="button"
                            class="settings-finder-light is-minimize"
                            :aria-label="
                                formatMessage(messages.launcherPreviewSettingsFinderMinimize)
                            "
                            @click="minimize"
                        >
                            <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.25 4h5.5" /></svg>
                        </button>
                        <button
                            type="button"
                            class="settings-finder-light is-zoom"
                            :aria-label="formatMessage(messages.launcherPreviewSettingsFinderZoom)"
                            @click="zoomed = !zoomed"
                        >
                            <svg viewBox="0 0 8 8" aria-hidden="true">
                                <path d="M2 2h3L2 5zM6 6H3l3-3z" class="is-filled" />
                            </svg>
                        </button>
                    </div>
                    <p class="settings-finder-sidebar-heading">
                        {{ formatMessage(messages.launcherPreviewSettingsFinderFavorites) }}
                    </p>
                    <button
                        v-for="folder in FINDER_FAVORITES"
                        :key="folder"
                        type="button"
                        class="settings-finder-sidebar-item"
                        :class="{ 'is-active': activeFavorite === folder }"
                        @click="props.finder.navigate(favoritePath(folder))"
                    >
                        <component :is="favoriteIcons[folder]" aria-hidden="true" />
                        <span>{{ formatMessage(favoriteLabels[folder]) }}</span>
                    </button>
                </aside>

                <section class="settings-finder-main">
                    <header class="settings-finder-toolbar">
                        <div class="settings-finder-nav">
                            <button
                                type="button"
                                class="settings-finder-tool"
                                :disabled="!state.back.length"
                                :aria-label="
                                    formatMessage(messages.launcherPreviewSettingsFinderBack)
                                "
                                @click="props.finder.goBack()"
                            >
                                <ChevronLeftIcon aria-hidden="true" />
                            </button>
                            <button
                                type="button"
                                class="settings-finder-tool"
                                :disabled="!state.forward.length"
                                :aria-label="
                                    formatMessage(messages.launcherPreviewSettingsFinderForward)
                                "
                                @click="props.finder.goForward()"
                            >
                                <ChevronRightIcon aria-hidden="true" />
                            </button>
                        </div>
                        <span class="settings-finder-title">{{ title }}</span>
                    </header>

                    <div class="settings-finder-list" role="grid" :aria-label="title">
                        <div class="settings-finder-row is-header" role="row">
                            <span class="settings-finder-cell is-name" role="columnheader">
                                {{
                                    formatMessage(messages.launcherPreviewSettingsFinderColumnName)
                                }}
                            </span>
                            <span class="settings-finder-cell is-modified" role="columnheader">
                                {{
                                    formatMessage(
                                        messages.launcherPreviewSettingsFinderColumnModified,
                                    )
                                }}
                            </span>
                            <span class="settings-finder-cell is-size" role="columnheader">
                                {{
                                    formatMessage(messages.launcherPreviewSettingsFinderColumnSize)
                                }}
                            </span>
                            <span class="settings-finder-cell is-kind" role="columnheader">
                                {{
                                    formatMessage(messages.launcherPreviewSettingsFinderColumnKind)
                                }}
                            </span>
                        </div>
                        <div
                            class="settings-finder-rows"
                            @pointerdown.self="props.finder.select(null)"
                        >
                            <div
                                v-for="item in items"
                                :key="item.name"
                                class="settings-finder-row"
                                :class="{ 'is-selected': state.selected === item.name }"
                                role="row"
                                :aria-selected="state.selected === item.name"
                                @pointerdown="props.finder.select(item.name)"
                            >
                                <span class="settings-finder-cell is-name" role="gridcell">
                                    <component
                                        :is="itemIcon(item)"
                                        class="settings-finder-file-icon"
                                        aria-hidden="true"
                                    />
                                    <span class="settings-finder-file-name">{{ item.name }}</span>
                                </span>
                                <span class="settings-finder-cell is-modified" role="gridcell">
                                    {{ formatModified(item.modified) }}
                                </span>
                                <span class="settings-finder-cell is-size" role="gridcell">
                                    {{ formatSize(item.bytes) }}
                                </span>
                                <span class="settings-finder-cell is-kind" role="gridcell">
                                    {{ formatMessage(kindLabels[item.kind]) }}
                                </span>
                            </div>
                        </div>
                    </div>

                    <footer class="settings-finder-pathbar">
                        <template v-for="(segment, index) in pathBar" :key="`${index}-${segment}`">
                            <ChevronRightIcon
                                v-if="index > 0"
                                class="settings-finder-path-chevron"
                                aria-hidden="true"
                            />
                            <span class="settings-finder-path-segment">
                                <FolderIcon aria-hidden="true" />
                                {{ segment }}
                            </span>
                        </template>
                    </footer>
                </section>
            </div>
        </div>
    </Transition>
</template>

<style scoped>
/*
 * macOS system chrome (a Finder window in list view, Sequoia). The desktop
 * app gets this window from the operating system, so it uses the system's
 * colours rather than the launcher's theme tokens.
 */
.settings-finder-layer {
    position: absolute;
    inset: 0;
    z-index: 300;
    font-family:
        -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', system-ui, sans-serif;
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
}

.settings-finder-window {
    position: absolute;
    top: 132px;
    left: calc(50% - 450px);
    display: flex;
    width: 900px;
    height: 460px;
    overflow: hidden;
    border: 0.5px solid rgb(255 255 255 / 16%);
    border-radius: 12px;
    background: #1e1e1e;
    color: rgb(255 255 255 / 86%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 70%),
        0 26px 80px 6px rgb(0 0 0 / 55%);
    outline: none;
    transition:
        top 220ms cubic-bezier(0.22, 1, 0.36, 1),
        left 220ms cubic-bezier(0.22, 1, 0.36, 1),
        width 220ms cubic-bezier(0.22, 1, 0.36, 1),
        height 220ms cubic-bezier(0.22, 1, 0.36, 1);
}

.settings-finder-window.is-zoomed {
    top: 40px;
    left: 24px;
    width: calc(100% - 48px);
    height: calc(100% - 64px);
}

.launcher-preview-window.light-mode .settings-finder-window {
    border-color: rgb(0 0 0 / 14%);
    background: #fff;
    color: rgb(0 0 0 / 86%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 16%),
        0 26px 80px 6px rgb(0 0 0 / 26%);
}

.settings-finder-sidebar {
    display: flex;
    width: 176px;
    flex-shrink: 0;
    flex-direction: column;
    gap: 1px;
    padding: 0 10px;
    border-right: 0.5px solid rgb(0 0 0 / 55%);
    background: rgb(44 44 46 / 96%);
}

.launcher-preview-window.light-mode .settings-finder-sidebar {
    border-right-color: rgb(0 0 0 / 10%);
    background: rgb(233 231 236 / 96%);
}

.settings-finder-traffic {
    display: flex;
    height: 52px;
    flex-shrink: 0;
    align-items: center;
    gap: 8px;
    padding-left: 10px;
}

.settings-finder-light {
    display: flex;
    width: 12px;
    height: 12px;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 0.5px solid rgb(0 0 0 / 22%);
    border-radius: 50%;
    cursor: default;
}

.settings-finder-light.is-close {
    background: #ff5f57;
}

.settings-finder-light.is-minimize {
    background: #febc2e;
}

.settings-finder-light.is-zoom {
    background: #28c840;
}

.settings-finder-light svg {
    width: 8px;
    height: 8px;
    opacity: 0;
    fill: none;
    stroke: rgb(0 0 0 / 60%);
    stroke-width: 1.2;
    stroke-linecap: round;
}

.settings-finder-light svg .is-filled {
    fill: rgb(0 0 0 / 60%);
    stroke: none;
}

.settings-finder-traffic:hover .settings-finder-light svg,
.settings-finder-light:focus-visible svg {
    opacity: 1;
}

.settings-finder-light:active {
    filter: brightness(0.85);
}

.settings-finder-sidebar-heading {
    margin: 6px 0 4px;
    padding: 0 8px;
    color: rgb(255 255 255 / 45%);
    font-size: 11px;
    font-weight: 600;
}

.launcher-preview-window.light-mode .settings-finder-sidebar-heading {
    color: rgb(0 0 0 / 42%);
}

.settings-finder-sidebar-item {
    display: flex;
    height: 28px;
    align-items: center;
    gap: 7px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
}

.settings-finder-sidebar-item svg {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    color: #0a84ff;
}

.launcher-preview-window.light-mode .settings-finder-sidebar-item svg {
    color: #007aff;
}

.settings-finder-sidebar-item.is-active {
    background: rgb(255 255 255 / 11%);
}

.launcher-preview-window.light-mode .settings-finder-sidebar-item.is-active {
    background: rgb(0 0 0 / 8%);
}

.settings-finder-main {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
}

.settings-finder-toolbar {
    display: flex;
    height: 52px;
    flex-shrink: 0;
    align-items: center;
    gap: 10px;
    padding: 0 14px 0 10px;
    border-bottom: 0.5px solid rgb(0 0 0 / 60%);
    background: #282828;
}

.launcher-preview-window.light-mode .settings-finder-toolbar {
    border-bottom-color: rgb(0 0 0 / 10%);
    background: #f6f6f6;
}

.settings-finder-nav {
    display: flex;
    gap: 2px;
}

.settings-finder-tool {
    display: flex;
    width: 28px;
    height: 28px;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    cursor: default;
}

.settings-finder-tool svg {
    width: 18px;
    height: 18px;
    opacity: 0.8;
}

.settings-finder-tool:not(:disabled):hover {
    background: rgb(255 255 255 / 9%);
}

.launcher-preview-window.light-mode .settings-finder-tool:not(:disabled):hover {
    background: rgb(0 0 0 / 6%);
}

.settings-finder-tool:disabled {
    opacity: 0.3;
}

.settings-finder-title {
    overflow: hidden;
    font-size: 15px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.settings-finder-list {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
}

.settings-finder-row {
    display: grid;
    height: 24px;
    flex-shrink: 0;
    grid-template-columns: minmax(0, 1fr) 172px 68px 104px;
    align-items: center;
    margin: 0 10px;
    padding: 0 6px;
    border-radius: 5px;
    cursor: default;
}

.settings-finder-row.is-header {
    height: 28px;
    margin: 0;
    padding: 0 16px;
    border-radius: 0;
    border-bottom: 0.5px solid rgb(255 255 255 / 9%);
    color: rgb(255 255 255 / 52%);
    font-size: 11px;
}

.launcher-preview-window.light-mode .settings-finder-row.is-header {
    border-bottom-color: rgb(0 0 0 / 9%);
    color: rgb(0 0 0 / 48%);
}

.settings-finder-row.is-header .settings-finder-cell + .settings-finder-cell {
    border-left: 0.5px solid rgb(255 255 255 / 10%);
}

.launcher-preview-window.light-mode
    .settings-finder-row.is-header
    .settings-finder-cell
    + .settings-finder-cell {
    border-left-color: rgb(0 0 0 / 9%);
}

.settings-finder-rows {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    padding-top: 4px;
    /* The alternating stripes of the list continue below the last item. */
    background-image: repeating-linear-gradient(
        to bottom,
        transparent 0 24px,
        rgb(255 255 255 / 3.5%) 24px 48px
    );
    background-position: 0 4px;
    background-origin: border-box;
}

.launcher-preview-window.light-mode .settings-finder-rows {
    background-image: repeating-linear-gradient(to bottom, transparent 0 24px, #f4f5f5 24px 48px);
}

.settings-finder-row.is-selected {
    background: #0b5cd5;
    color: #fff;
}

.launcher-preview-window.light-mode .settings-finder-row.is-selected {
    background: #0064e1;
}

.settings-finder-cell {
    overflow: hidden;
    padding: 0 6px;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.settings-finder-row:not(.is-header, .is-selected) .settings-finder-cell:not(.is-name) {
    color: rgb(255 255 255 / 55%);
}

.launcher-preview-window.light-mode
    .settings-finder-row:not(.is-header, .is-selected)
    .settings-finder-cell:not(.is-name) {
    color: rgb(0 0 0 / 52%);
}

.settings-finder-cell.is-name {
    display: flex;
    align-items: center;
    gap: 6px;
}

.settings-finder-cell.is-size {
    text-align: right;
}

.settings-finder-file-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    opacity: 0.85;
}

.settings-finder-file-name {
    overflow: hidden;
    text-overflow: ellipsis;
}

.settings-finder-pathbar {
    display: flex;
    height: 26px;
    flex-shrink: 0;
    align-items: center;
    gap: 3px;
    overflow: hidden;
    padding: 0 12px;
    border-top: 0.5px solid rgb(255 255 255 / 9%);
    color: rgb(255 255 255 / 70%);
    font-size: 11px;
    white-space: nowrap;
}

.launcher-preview-window.light-mode .settings-finder-pathbar {
    border-top-color: rgb(0 0 0 / 9%);
    color: rgb(0 0 0 / 70%);
}

.settings-finder-path-segment {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 3px;
}

.settings-finder-path-segment svg {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    color: #0a84ff;
}

.settings-finder-path-chevron {
    width: 10px;
    height: 10px;
    flex-shrink: 0;
    opacity: 0.5;
}

.settings-finder-enter-active {
    transition: opacity 160ms ease;
}

.settings-finder-enter-active .settings-finder-window {
    transition: transform 220ms cubic-bezier(0.22, 1, 0.36, 1);
}

.settings-finder-leave-active {
    transition: opacity 120ms ease;
}

.settings-finder-enter-from,
.settings-finder-leave-to {
    opacity: 0;
}

.settings-finder-enter-from .settings-finder-window {
    transform: scale(0.94);
}

@media (prefers-reduced-motion: reduce) {
    .settings-finder-window,
    .settings-finder-enter-active .settings-finder-window {
        transition: none;
    }
}
</style>
