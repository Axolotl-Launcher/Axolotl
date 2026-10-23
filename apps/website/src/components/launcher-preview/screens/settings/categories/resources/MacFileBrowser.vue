<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import { messages } from '../../messages'
import { ancestors, type FsNode, HOME } from './mac-fs'
import {
    displayName,
    formatFinderDate,
    formatFinderSize,
    type MacBrowser,
    type MacIconName,
    type MacListEntry,
    SIDEBAR_FAVORITES,
    type SortKey,
} from './mac-ui'
import MacIcon from './MacIcon.vue'

/**
 * The browser shared by the preview Mac's open panel and Finder window:
 * sidebar (Favorites, Locations), toolbar (back/forward, path pop-up or title,
 * search) and the list view (name, date modified, size; sortable, keyboard
 * selection, double-click to enter folders).
 */
const props = defineProps<{
    browser: MacBrowser
    variant: 'panel' | 'finder'
    appearance: 'dark' | 'light'
}>()

const emit = defineEmits<{
    /** Double-click or Command-Down on an entry. */
    activate: [entry: MacListEntry]
    close: []
}>()

const { formatMessage, locale } = useVIntl()

const listBody = ref<HTMLElement | null>(null)
const pathMenuOpen = ref(false)
const pathMenu = ref<HTMLElement | null>(null)

const currentName = computed(() => displayName(props.browser.path.value, formatMessage))
const pathChoices = computed(() => ancestors(props.browser.path.value))

const MIN_ROWS = 18
const fillerRows = computed(() => Math.max(0, MIN_ROWS - props.browser.entries.value.length))

function iconFor(node: FsNode): MacIconName {
    if (node.kind === 'dir') return node.package ? 'app' : 'folder'
    const extension = node.name.split('.').pop()?.toLowerCase()
    if (extension === 'png' || extension === 'jpg' || extension === 'jpeg') return 'image'
    if (extension === 'dmg') return 'disk-image'
    return 'document'
}

function pathIcon(path: string): MacIconName {
    return path === '/' ? 'drive' : 'folder'
}

function select(entry: MacListEntry) {
    if (entry.disabled) return
    props.browser.select(entry.path)
}

function activate(entry: MacListEntry) {
    if (entry.node.kind === 'dir' && !entry.node.package) {
        props.browser.go(entry.path)
        return
    }
    emit('activate', entry)
}

function headerSort(key: SortKey) {
    props.browser.toggleSort(key)
}

function choosePath(path: string) {
    pathMenuOpen.value = false
    props.browser.go(path)
}

function onPointerDown(event: PointerEvent) {
    if (pathMenu.value?.contains(event.target as Node)) return
    pathMenuOpen.value = false
}

watch(pathMenuOpen, (open) => {
    if (open) document.addEventListener('pointerdown', onPointerDown, true)
    else document.removeEventListener('pointerdown', onPointerDown, true)
})

/** Keeps the selected row visible while moving through the list with the keyboard. */
watch(
    () => props.browser.selected.value,
    async (path) => {
        await nextTick()
        if (!path || !listBody.value) return
        const row = Array.from(
            listBody.value.querySelectorAll<HTMLElement>('[data-mac-path]'),
        ).find((element) => element.dataset.macPath === path)
        row?.scrollIntoView({ block: 'nearest' })
    },
)

watch(
    () => props.browser.path.value,
    () => listBody.value?.scrollTo({ top: 0 }),
)

/** Closes the path pop-up; true when it was open (Escape is consumed). */
function closeMenus(): boolean {
    if (!pathMenuOpen.value) return false
    pathMenuOpen.value = false
    return true
}

onBeforeUnmount(() => document.removeEventListener('pointerdown', onPointerDown, true))

defineExpose({ closeMenus })
</script>

<template>
    <div class="lp-mac-browser" :class="`lp-mac-${appearance}`">
        <nav class="lp-mac-sidebar">
            <div v-if="variant === 'finder'" class="lp-mac-traffic-lights">
                <button
                    type="button"
                    class="lp-mac-traffic lp-mac-traffic-close"
                    :aria-label="formatMessage(messages.launcherPreviewSettingsMacClose)"
                    @click="emit('close')"
                />
                <span class="lp-mac-traffic lp-mac-traffic-idle" />
                <span class="lp-mac-traffic lp-mac-traffic-idle" />
            </div>
            <div class="lp-mac-sidebar-section">
                <div class="lp-mac-sidebar-heading">
                    {{ formatMessage(messages.launcherPreviewSettingsMacFavorites) }}
                </div>
                <button
                    v-for="favorite in SIDEBAR_FAVORITES"
                    :key="favorite.path"
                    type="button"
                    class="lp-mac-sidebar-item"
                    :class="{ 'is-active': browser.path.value === favorite.path }"
                    @click="browser.go(favorite.path)"
                >
                    <MacIcon :name="favorite.icon" class="lp-mac-sidebar-icon" />
                    <span class="truncate">{{
                        favorite.path === HOME ? 'steve' : displayName(favorite.path, formatMessage)
                    }}</span>
                </button>
            </div>
            <div class="lp-mac-sidebar-section">
                <div class="lp-mac-sidebar-heading">
                    {{ formatMessage(messages.launcherPreviewSettingsMacLocations) }}
                </div>
                <button
                    type="button"
                    class="lp-mac-sidebar-item"
                    :class="{ 'is-active': browser.path.value === '/' }"
                    @click="browser.go('/')"
                >
                    <MacIcon name="drive" class="lp-mac-sidebar-icon lp-mac-sidebar-icon-muted" />
                    <span class="truncate">{{
                        formatMessage(messages.launcherPreviewSettingsMacMacintoshHd)
                    }}</span>
                </button>
            </div>
        </nav>

        <div class="lp-mac-main">
            <div class="lp-mac-toolbar" :class="`is-${variant}`">
                <div class="lp-mac-nav">
                    <button
                        type="button"
                        class="lp-mac-tool"
                        :disabled="!browser.canBack.value"
                        :aria-label="formatMessage(messages.launcherPreviewSettingsMacBack)"
                        @click="browser.back()"
                    >
                        <MacIcon name="chevron-left" class="size-4" />
                    </button>
                    <button
                        type="button"
                        class="lp-mac-tool"
                        :disabled="!browser.canForward.value"
                        :aria-label="formatMessage(messages.launcherPreviewSettingsMacForward)"
                        @click="browser.forward()"
                    >
                        <MacIcon name="chevron-right" class="size-4" />
                    </button>
                </div>
                <span v-if="variant === 'finder'" class="lp-mac-title truncate">{{
                    currentName
                }}</span>
                <div v-else ref="pathMenu" class="relative min-w-0">
                    <button
                        type="button"
                        class="lp-mac-path-button"
                        :aria-expanded="pathMenuOpen"
                        aria-haspopup="menu"
                        @click="pathMenuOpen = !pathMenuOpen"
                    >
                        <MacIcon :name="pathIcon(browser.path.value)" class="size-4 shrink-0" />
                        <span class="truncate">{{ currentName }}</span>
                        <MacIcon name="chevron-up-down" class="size-3 shrink-0 opacity-70" />
                    </button>
                    <div v-if="pathMenuOpen" class="lp-mac-menu" role="menu">
                        <button
                            v-for="choice in pathChoices"
                            :key="choice"
                            type="button"
                            role="menuitem"
                            class="lp-mac-menu-item"
                            @click="choosePath(choice)"
                        >
                            <MacIcon :name="pathIcon(choice)" class="size-4 shrink-0" />
                            <span class="truncate">{{ displayName(choice, formatMessage) }}</span>
                        </button>
                    </div>
                </div>
                <div class="flex-1" />
                <label class="lp-mac-search">
                    <MacIcon name="search" class="size-3.5 shrink-0 opacity-60" />
                    <input
                        :value="browser.search.value"
                        type="search"
                        class="lp-mac-search-input"
                        :placeholder="formatMessage(messages.launcherPreviewSettingsMacSearch)"
                        spellcheck="false"
                        @input="browser.setSearch(($event.target as HTMLInputElement).value)"
                    />
                </label>
            </div>

            <div class="lp-mac-list" role="grid">
                <div class="lp-mac-list-header" role="row">
                    <button
                        type="button"
                        class="lp-mac-header-cell lp-mac-col-name"
                        :class="{ 'is-sorted': browser.sortKey.value === 'name' }"
                        @click="headerSort('name')"
                    >
                        {{ formatMessage(messages.launcherPreviewSettingsMacName) }}
                        <MacIcon
                            v-if="browser.sortKey.value === 'name'"
                            name="chevron-down"
                            class="lp-mac-sort-icon"
                            :class="{ 'rotate-180': !browser.sortAscending.value }"
                        />
                    </button>
                    <button
                        type="button"
                        class="lp-mac-header-cell lp-mac-col-date"
                        :class="{ 'is-sorted': browser.sortKey.value === 'modified' }"
                        @click="headerSort('modified')"
                    >
                        {{ formatMessage(messages.launcherPreviewSettingsMacDateModified) }}
                        <MacIcon
                            v-if="browser.sortKey.value === 'modified'"
                            name="chevron-down"
                            class="lp-mac-sort-icon"
                            :class="{ 'rotate-180': browser.sortAscending.value }"
                        />
                    </button>
                    <button
                        type="button"
                        class="lp-mac-header-cell lp-mac-col-size"
                        :class="{ 'is-sorted': browser.sortKey.value === 'size' }"
                        @click="headerSort('size')"
                    >
                        {{ formatMessage(messages.launcherPreviewSettingsMacSize) }}
                        <MacIcon
                            v-if="browser.sortKey.value === 'size'"
                            name="chevron-down"
                            class="lp-mac-sort-icon"
                            :class="{ 'rotate-180': browser.sortAscending.value }"
                        />
                    </button>
                </div>
                <div ref="listBody" class="lp-mac-list-body" @mousedown.self="browser.select(null)">
                    <div
                        v-for="(entry, index) in browser.entries.value"
                        :key="entry.path"
                        role="row"
                        class="lp-mac-row"
                        :class="{
                            'is-odd': index % 2 === 1,
                            'is-selected': browser.selected.value === entry.path,
                            'is-disabled': entry.disabled,
                            'is-hidden': entry.node.hidden,
                        }"
                        :data-mac-path="entry.path"
                        :aria-selected="browser.selected.value === entry.path"
                        @mousedown="select(entry)"
                        @dblclick="activate(entry)"
                    >
                        <span class="lp-mac-cell lp-mac-col-name">
                            <MacIcon :name="iconFor(entry.node)" class="lp-mac-row-icon" />
                            <span class="truncate">{{
                                displayName(entry.path, formatMessage)
                            }}</span>
                        </span>
                        <span class="lp-mac-cell lp-mac-col-date lp-mac-secondary">{{
                            formatFinderDate(entry.node.modified, locale, formatMessage)
                        }}</span>
                        <span class="lp-mac-cell lp-mac-col-size lp-mac-secondary">{{
                            formatFinderSize(entry.node, locale, formatMessage)
                        }}</span>
                    </div>
                    <div
                        v-for="filler in fillerRows"
                        :key="`filler-${filler}`"
                        class="lp-mac-row lp-mac-row-filler"
                        :class="{ 'is-odd': (browser.entries.value.length + filler - 1) % 2 === 1 }"
                        @mousedown="browser.select(null)"
                    />
                </div>
            </div>
        </div>
    </div>
</template>

<style lang="scss">
/* The preview Mac's own chrome (Finder, open panel). It follows the system
   appearance rather than the launcher theme, so it carries literal macOS
   colors. Shared by MacOpenPanel and MacFinderWindow. */
.lp-mac-surface {
    font-family:
        -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', 'PingFang SC',
        sans-serif;
    font-size: 13px;
    line-height: 16px;
    letter-spacing: -0.08px;
    color: rgba(255, 255, 255, 0.86);
    -webkit-font-smoothing: antialiased;
    user-select: none;

    &.lp-mac-light {
        color: rgba(0, 0, 0, 0.85);
    }
}

.lp-mac-browser {
    display: flex;
    min-height: 0;
    flex: 1;
}

.lp-mac-sidebar {
    display: flex;
    width: 170px;
    flex-shrink: 0;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
    padding: 10px 8px;
    background: #262626;
    border-right: 1px solid rgba(0, 0, 0, 0.45);

    .lp-mac-light & {
        background: #ebebeb;
        border-right-color: rgba(0, 0, 0, 0.1);
    }
}

.lp-mac-traffic-lights {
    display: flex;
    gap: 8px;
    padding: 4px 6px 8px;
}

.lp-mac-traffic {
    width: 12px;
    height: 12px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.25);
}

.lp-mac-traffic-close {
    background: #ff5f57;
    cursor: default;

    &:hover::after {
        content: '×';
        display: block;
        margin-top: -3px;
        color: rgba(77, 0, 0, 0.75);
        font-size: 12px;
        font-weight: 700;
        line-height: 16px;
        text-align: center;
    }
}

.lp-mac-traffic-idle {
    background: rgba(255, 255, 255, 0.2);

    .lp-mac-light & {
        background: rgba(0, 0, 0, 0.12);
    }
}

.lp-mac-sidebar-section {
    display: flex;
    flex-direction: column;
    gap: 1px;
}

.lp-mac-sidebar-heading {
    padding: 2px 8px 4px;
    color: rgba(255, 255, 255, 0.4);
    font-size: 11px;
    font-weight: 600;

    .lp-mac-light & {
        color: rgba(0, 0, 0, 0.4);
    }
}

.lp-mac-sidebar-item {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;

    &.is-active {
        background: rgba(255, 255, 255, 0.1);

        .lp-mac-light & {
            background: rgba(0, 0, 0, 0.08);
        }
    }
}

.lp-mac-sidebar-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    color: #0a84ff;

    .lp-mac-light & {
        color: #007aff;
    }
}

.lp-mac-sidebar-icon-muted {
    color: rgba(255, 255, 255, 0.6);

    .lp-mac-light & {
        color: rgba(0, 0, 0, 0.5);
    }
}

.lp-mac-main {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    background: #1e1e1e;

    .lp-mac-light & {
        background: #fff;
    }
}

.lp-mac-toolbar {
    display: flex;
    height: 44px;
    flex-shrink: 0;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    background: #2a2a2a;
    border-bottom: 1px solid rgba(0, 0, 0, 0.5);

    &.is-finder {
        height: 52px;
        background: #1e1e1e;
        border-bottom-color: rgba(255, 255, 255, 0.08);
    }

    .lp-mac-light & {
        background: #f6f6f6;
        border-bottom-color: rgba(0, 0, 0, 0.1);

        &.is-finder {
            background: #fff;
        }
    }
}

.lp-mac-nav {
    display: flex;
    gap: 2px;
}

.lp-mac-tool {
    display: flex;
    width: 28px;
    height: 26px;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.72);
    cursor: default;

    &:hover:not(:disabled) {
        background: rgba(255, 255, 255, 0.08);
    }

    &:disabled {
        opacity: 0.3;
    }

    .lp-mac-light & {
        color: rgba(0, 0, 0, 0.6);

        &:hover:not(:disabled) {
            background: rgba(0, 0, 0, 0.06);
        }
    }
}

.lp-mac-title {
    min-width: 0;
    font-size: 15px;
    font-weight: 700;
}

.lp-mac-path-button {
    display: flex;
    max-width: 260px;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.07);
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: default;

    &:hover {
        background: rgba(255, 255, 255, 0.12);
    }

    .lp-mac-light & {
        background: rgba(0, 0, 0, 0.05);

        &:hover {
            background: rgba(0, 0, 0, 0.08);
        }
    }
}

.lp-mac-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 5;
    display: flex;
    min-width: 220px;
    flex-direction: column;
    padding: 5px;
    border-radius: 8px;
    background: rgba(40, 40, 40, 0.98);
    box-shadow:
        0 0 0 0.5px rgba(0, 0, 0, 0.8),
        inset 0 0 0 0.5px rgba(255, 255, 255, 0.18),
        0 10px 28px rgba(0, 0, 0, 0.45);

    .lp-mac-light & {
        background: rgba(246, 246, 246, 0.98);
        box-shadow:
            0 0 0 0.5px rgba(0, 0, 0, 0.18),
            0 10px 28px rgba(0, 0, 0, 0.2);
    }
}

.lp-mac-menu-item {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 7px;
    height: 24px;
    padding: 0 9px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;

    &:hover {
        background: #0a84ff;
        color: #fff;

        .lp-mac-light & {
            background: #007aff;
        }
    }
}

.lp-mac-search {
    display: flex;
    width: 170px;
    height: 26px;
    align-items: center;
    gap: 5px;
    padding: 0 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);

    .lp-mac-light & {
        background: rgba(0, 0, 0, 0.05);
    }
}

.lp-mac-search-input {
    min-width: 0;
    height: 100%;
    flex: 1;
    padding: 0;
    border: 0;
    background: transparent;
    box-shadow: none;
    color: inherit;
    font: inherit;
    outline: none;

    &::placeholder {
        color: rgba(255, 255, 255, 0.35);
    }

    &::-webkit-search-decoration {
        appearance: none;
    }

    .lp-mac-light &::placeholder {
        color: rgba(0, 0, 0, 0.35);
    }
}

.lp-mac-list {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
}

.lp-mac-list-header {
    display: flex;
    height: 24px;
    flex-shrink: 0;
    align-items: stretch;
    padding: 0 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);

    .lp-mac-light & {
        border-bottom-color: rgba(0, 0, 0, 0.1);
    }
}

.lp-mac-header-cell {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px;
    border: 0;
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    background: transparent;
    color: rgba(255, 255, 255, 0.55);
    font: inherit;
    font-size: 11px;
    font-weight: 600;
    cursor: default;

    &:first-child {
        border-left: 0;
    }

    &.is-sorted {
        color: rgba(255, 255, 255, 0.85);
    }

    .lp-mac-light & {
        border-left-color: rgba(0, 0, 0, 0.1);
        color: rgba(0, 0, 0, 0.5);

        &.is-sorted {
            color: rgba(0, 0, 0, 0.85);
        }
    }
}

.lp-mac-sort-icon {
    width: 10px;
    height: 10px;
    margin-left: auto;
    opacity: 0.6;
}

.lp-mac-col-name {
    min-width: 0;
    flex: 1;
}

.lp-mac-col-date {
    width: 190px;
    flex-shrink: 0;
}

.lp-mac-col-size {
    width: 86px;
    flex-shrink: 0;
    justify-content: flex-end;
    text-align: right;
}

.lp-mac-list-body {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    padding: 0 10px 6px;
}

.lp-mac-row {
    display: flex;
    height: 24px;
    align-items: center;
    border-radius: 5px;

    &.is-odd {
        background: rgba(255, 255, 255, 0.035);

        .lp-mac-light & {
            background: #f4f5f5;
        }
    }

    &.is-selected {
        background: #0058d0;
        color: #fff;

        .lp-mac-secondary {
            color: rgba(255, 255, 255, 0.85);
        }

        .lp-mac-light & {
            background: #0064e1;
        }
    }

    &.is-disabled {
        color: rgba(255, 255, 255, 0.3);

        .lp-mac-secondary,
        .lp-mac-row-icon {
            opacity: 0.45;
        }

        .lp-mac-light & {
            color: rgba(0, 0, 0, 0.3);
        }
    }

    &.is-hidden:not(.is-selected) {
        opacity: 0.55;
    }
}

.lp-mac-cell {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
}

.lp-mac-secondary {
    color: rgba(255, 255, 255, 0.5);

    .lp-mac-light & {
        color: rgba(0, 0, 0, 0.5);
    }
}

.lp-mac-row-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
}

/* Push buttons, text fields and the inner dialogs of the open panel. */
.lp-mac-button {
    display: inline-flex;
    min-width: 76px;
    height: 24px;
    align-items: center;
    justify-content: center;
    padding: 0 12px;
    border: 0;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.18);
    box-shadow:
        inset 0 0.5px 0 rgba(255, 255, 255, 0.12),
        0 0.5px 1px rgba(0, 0, 0, 0.4);
    color: rgba(255, 255, 255, 0.9);
    font: inherit;
    cursor: default;

    &:active:not(:disabled) {
        filter: brightness(1.2);
    }

    &:disabled {
        opacity: 0.4;
    }

    &.is-default {
        background: linear-gradient(#3b8cff, #0a6cf5);
        color: #fff;
    }

    .lp-mac-light & {
        background: #fff;
        box-shadow:
            0 0 0 0.5px rgba(0, 0, 0, 0.18),
            0 0.5px 1.5px rgba(0, 0, 0, 0.18);
        color: rgba(0, 0, 0, 0.85);

        &:active:not(:disabled) {
            filter: brightness(0.94);
        }

        &.is-default {
            background: linear-gradient(#2a8cff, #0070f0);
            box-shadow: 0 0.5px 1.5px rgba(0, 0, 0, 0.25);
            color: #fff;
        }
    }
}

.lp-mac-field {
    width: 100%;
    height: 24px;
    min-height: 0;
    padding: 0 7px;
    border: 0;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.1);
    box-shadow:
        0 0 0 0.5px rgba(255, 255, 255, 0.12),
        0 0 0 3.5px rgba(10, 132, 255, 0.55);
    color: inherit;
    font: inherit;
    outline: none;

    .lp-mac-light & {
        background: #fff;
        box-shadow:
            0 0 0 0.5px rgba(0, 0, 0, 0.25),
            0 0 0 3.5px rgba(0, 122, 255, 0.45);
    }
}
</style>
