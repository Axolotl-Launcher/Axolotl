<script setup lang="ts">
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { messages } from './messages'
import {
    getVisibleSettingsCategories,
    getVisibleSettingsGroups,
    type SettingsCategory,
    settingsPageTitle,
} from './registry'
import { filterSettingsSearchDocuments, normalizeSettingsSearchText } from './search'
import {
    getSettingsSearchTargetId,
    type SettingsSearchEntry,
    type SettingsSearchMessage,
} from './search-index'
import {
    PREVIEW_APP_VERSION,
    PREVIEW_OS_NAME,
    PREVIEW_OS_VERSION,
    PREVIEW_PRODUCT_NAME,
    useSettingsPage,
} from './state'

/**
 * The settings page (apps/app-frontend/src/pages/Settings.vue): a searchable,
 * grouped category sidebar beside the selected category, which loads lazily
 * under a short skeleton. The page manages its own two scrollers, so while it
 * is mounted the shell's viewport stops scrolling (the app does this with a
 * global `:has(.settings-fixed-render)` rule).
 */
interface SettingsSearchResult {
    category: SettingsCategory
    entry?: SettingsSearchEntry
    label: string
    breadcrumb: string
}

const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { preview, screen, simulate } = useSettingsPage()
const { clock } = preview

const devMode = computed(() => !!preview.settings.developer_mode)
const devModeCounter = ref(0)
const searchQuery = ref('')
const selectedCategoryId = ref(route.hash.slice(1) || 'interface')
const settingsContentPending = ref(false)
const contentContainer = ref<HTMLElement | null>(null)
const pendingSearchTarget = ref<{ categoryId: string; targetId: string; query: string } | null>(
    null,
)
const searchHighlightTarget = ref<HTMLElement | null>(null)
const expandedGroups = ref<Record<string, boolean>>({
    launcher: true,
    game: true,
    'data-privacy': true,
    support: true,
    developer: false,
})
const hasSearchQuery = computed(() => !!normalizeSettingsSearchText(searchQuery.value))
const debouncedSearchQuery = ref('')
const searchResultsPending = ref(false)
const showContentSkeleton = ref(false)
let cancelSearchDebounce: (() => void) | undefined
let cancelSearchHighlight: (() => void) | undefined
let categoryTransitionToken = 0

const SEARCH_DEBOUNCE_MS = 120
const CATEGORY_SKELETON_MIN_MS = 160
/** Hard cap so a stuck category chunk cannot pin the skeleton forever. */
const CATEGORY_SKELETON_MAX_MS = 4000

const isContentLoading = computed(() => showContentSkeleton.value || settingsContentPending.value)

watch(searchQuery, (value) => {
    if (pendingSearchTarget.value && value !== pendingSearchTarget.value.query) {
        pendingSearchTarget.value = null
    }
    searchResultsPending.value = true
    cancelSearchDebounce?.()
    cancelSearchDebounce = clock.after(SEARCH_DEBOUNCE_MS, () => {
        debouncedSearchQuery.value = value
        void nextTick(() => {
            searchResultsPending.value = false
        })
    })
})

watch(selectedCategoryId, (categoryId) => {
    if (pendingSearchTarget.value?.categoryId !== categoryId) pendingSearchTarget.value = null
    screen.selectedCategoryId = categoryId
    const token = ++categoryTransitionToken
    showContentSkeleton.value = true
    const startedAt = Date.now()

    // Suspense may enter pending a tick after the category changes. Poll until
    // it settles, then keep the skeleton up for its minimum visible duration.
    const tick = () => {
        if (token !== categoryTransitionToken) return

        const elapsed = Date.now() - startedAt
        if (elapsed >= CATEGORY_SKELETON_MAX_MS) {
            showContentSkeleton.value = false
            return
        }

        if (settingsContentPending.value) {
            clock.after(32, tick)
            return
        }

        const remaining = Math.max(0, CATEGORY_SKELETON_MIN_MS - elapsed)
        clock.after(remaining || 16, () => {
            if (token !== categoryTransitionToken) return
            if (!settingsContentPending.value) {
                showContentSkeleton.value = false
                return
            }
            tick()
        })
    }

    clock.after(40, tick)
})

const visibleCategories = computed(() => getVisibleSettingsCategories(devMode.value))
const visibleGroups = computed(() => getVisibleSettingsGroups(devMode.value))
const activeCategory = computed(
    () =>
        visibleCategories.value.find((category) => category.id === selectedCategoryId.value) ??
        visibleCategories.value[0],
)

const searchResults = computed<SettingsSearchResult[]>(() => {
    const documents = visibleGroups.value.flatMap((group) =>
        group.categories.flatMap((category) => {
            const categoryLabel = categoryName(category)
            const groupLabel = formatMessage(group.name)

            return [
                {
                    item: {
                        category,
                        label: categoryLabel,
                        breadcrumb: groupLabel,
                    } as SettingsSearchResult,
                    text: searchTextVariants([group.name, category.name]),
                },
                ...category.entries.map((entry) => ({
                    item: {
                        category,
                        entry,
                        label: entryName(entry),
                        breadcrumb: `${groupLabel} > ${categoryLabel}`,
                    } as SettingsSearchResult,
                    text: [
                        ...searchTextVariants([
                            group.name,
                            category.name,
                            entry.label,
                            ...(entry.keywords ?? []),
                        ]),
                        ...messageSearchTexts(entry.description),
                    ],
                })),
            ]
        }),
    )

    return filterSettingsSearchDocuments(debouncedSearchQuery.value, documents).map(
        ({ item }) => item,
    )
})

watch(visibleCategories, (categories) => {
    if (!categories.some((category) => category.id === selectedCategoryId.value)) {
        selectedCategoryId.value = categories[0]?.id ?? 'interface'
    }
})

watch(
    () => route.hash,
    (hash) => {
        const categoryId = hash.slice(1)
        if (categoryId) selectedCategoryId.value = categoryId
    },
)

function selectCategory(categoryId: string) {
    selectedCategoryId.value = categoryId
    const category = visibleCategories.value.find((item) => item.id === categoryId)
    if (category) expandedGroups.value[category.group] = true
    contentContainer.value?.scrollTo({ top: 0 })
    // Keep the hash in sync so coming back through history restores this tab.
    if (route.hash !== `#${categoryId}` || route.query.page) {
        const { page: _page, ...query } = route.query
        void router.replace({ query, hash: `#${categoryId}` })
    }
}

function toggleGroup(groupId: string) {
    expandedGroups.value[groupId] = !expandedGroups.value[groupId]
}

async function revealSearchTarget() {
    await nextTick()
    const pending = pendingSearchTarget.value
    if (!pending || isContentLoading.value || activeCategory.value?.id !== pending.categoryId)
        return
    const target = contentContainer.value?.querySelector<HTMLElement>(`#${pending.targetId}`)
    if (!target) return
    target.scrollIntoView({ behavior: 'smooth', block: 'center' })
    flashSearchTarget(target)
    pendingSearchTarget.value = null
    if (searchQuery.value === pending.query) searchQuery.value = ''
}

watch(isContentLoading, () => void revealSearchTarget(), { flush: 'post' })

function onSettingsContentResolved() {
    settingsContentPending.value = false
    void revealSearchTarget()
}

async function selectSearchResult(result: SettingsSearchResult) {
    pendingSearchTarget.value = result.entry
        ? {
              categoryId: result.category.id,
              targetId: getSettingsSearchTargetId(result.entry),
              query: searchQuery.value,
          }
        : null
    selectedCategoryId.value = result.category.id
    expandedGroups.value[result.category.group] = true
    if (!result.entry) searchQuery.value = ''
    const { page: _page, ...query } = route.query
    await router.replace({ query, hash: `#${result.category.id}` })
    if (selectedCategoryId.value !== result.category.id) return
    contentContainer.value?.scrollTo({ top: 0 })
    await revealSearchTarget()
}

async function toggleDeveloperMode() {
    devModeCounter.value++
    if (devModeCounter.value <= 5) return

    try {
        await simulate()
        preview.settings.developer_mode = !preview.settings.developer_mode
    } catch (error) {
        handleError(error as Error)
    }
    devModeCounter.value = 0
}

function categoryName(category: SettingsCategory): string {
    return formatMessage(category.name)
}

function entryName(entry: SettingsSearchEntry): string {
    return formatMessage(entry.label)
}

function messageSearchTexts(message?: SettingsSearchMessage | MessageDescriptor): string[] {
    // Search metadata comes from several registries; a malformed entry must not
    // abort the whole settings render.
    if (!message || typeof message !== 'object' || typeof message.id !== 'string') return []

    // The app adds the descriptor's own default: for index entries that is the
    // index's fallback text.
    const fallback = 'indexDefault' in message ? message.indexDefault : message.defaultMessage
    const texts = [formatMessage(message), fallback].filter((text): text is string => !!text)
    return [...new Set(texts)]
}

function searchTextVariants(
    descriptors: Array<SettingsSearchMessage | MessageDescriptor | undefined>,
): string[] {
    return descriptors.flatMap(messageSearchTexts)
}

function searchResultKey(result: SettingsSearchResult): string {
    return result.entry?.id ?? `category-${result.category.id}`
}

function searchMatchSegments(text: string) {
    const query = normalizeSettingsSearchText(debouncedSearchQuery.value)
    if (!query) return [{ text, matched: false }]

    const index = text.toLocaleLowerCase().indexOf(query)
    if (index < 0) return [{ text, matched: false }]

    return [
        { text: text.slice(0, index), matched: false },
        { text: text.slice(index, index + query.length), matched: true },
        { text: text.slice(index + query.length), matched: false },
    ].filter((segment) => segment.text)
}

function flashSearchTarget(target: HTMLElement) {
    const highlightTarget = target.closest<HTMLElement>('.settings-row') ?? target
    searchHighlightTarget.value?.classList.remove('settings-search-result-highlight')
    cancelSearchHighlight?.()

    highlightTarget.classList.add('settings-search-result-highlight')
    searchHighlightTarget.value = highlightTarget
    cancelSearchHighlight = clock.after(1800, () => {
        highlightTarget.classList.remove('settings-search-result-highlight')
        if (searchHighlightTarget.value === highlightTarget) searchHighlightTarget.value = null
    })
}

const pageTitle: MessageDescriptor = settingsPageTitle

const root = ref<HTMLElement | null>(null)

onMounted(() => {
    // The viewport stops scrolling while the page is up (see the unscoped rule
    // below), so it must not keep the previous page's offset.
    const viewport = root.value?.closest<HTMLElement>('.app-viewport')
    if (viewport) viewport.scrollTop = 0
})

onBeforeUnmount(() => {
    categoryTransitionToken++
    cancelSearchDebounce?.()
    cancelSearchHighlight?.()
})
</script>

<template>
    <div ref="root" class="settings-fixed-render h-full min-h-0 p-6">
        <div class="settings-layout h-full min-h-0">
            <aside class="settings-sidebar">
                <div class="relative shrink-0">
                    <SearchIcon
                        class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--color-text-tertiary)]"
                    />
                    <input
                        v-model="searchQuery"
                        type="search"
                        :placeholder="formatMessage(messages.appSettingsSearchPlaceholder)"
                        :aria-label="formatMessage(messages.appSettingsSearchPlaceholder)"
                        class="settings-search-input w-full rounded-lg border border-surface-4 bg-surface-3 py-2 pl-9 pr-9 text-sm text-[var(--color-text-primary)] outline-none transition-colors placeholder:text-[var(--color-text-tertiary)] focus:border-surface-5"
                        @keydown.escape="searchQuery = ''"
                    />
                    <button
                        v-if="searchQuery"
                        type="button"
                        class="absolute inset-y-0 right-1.5 z-10 my-auto flex size-7 items-center justify-center rounded-md border-0 bg-transparent text-[var(--color-text-tertiary)] transition-colors hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
                        :aria-label="formatMessage(messages.appSettingsSearchClear)"
                        @click="searchQuery = ''"
                    >
                        <XIcon class="size-4" />
                    </button>
                </div>

                <div
                    v-if="hasSearchQuery"
                    class="settings-sidebar-list settings-search-results"
                    :aria-label="formatMessage(messages.appSettingsSearchResults)"
                    :aria-busy="searchResultsPending"
                >
                    <div
                        v-if="searchResultsPending"
                        class="settings-search-skeleton"
                        aria-hidden="true"
                    >
                        <div v-for="row in 5" :key="row" class="settings-search-skeleton-row">
                            <div class="settings-search-skeleton-icon animate-pulse" />
                            <div class="settings-search-skeleton-copy">
                                <div
                                    class="settings-search-skeleton-line animate-pulse"
                                    :class="row % 2 === 0 ? 'is-wide' : 'is-medium'"
                                />
                                <div class="settings-search-skeleton-line is-short animate-pulse" />
                            </div>
                        </div>
                    </div>

                    <TransitionGroup v-else name="settings-search">
                        <button
                            v-for="(result, index) in searchResults"
                            :key="searchResultKey(result)"
                            type="button"
                            class="settings-search-result"
                            :class="result.entry ? 'is-entry' : 'is-category'"
                            :style="{ '--search-stagger': `${Math.min(index * 28, 168)}ms` }"
                            @click="selectSearchResult(result)"
                        >
                            <component
                                :is="result.category.icon"
                                class="settings-search-result-icon"
                                aria-hidden="true"
                            />
                            <span class="settings-search-result-copy">
                                <span class="settings-search-result-label">
                                    <template
                                        v-for="segment in searchMatchSegments(result.label)"
                                        :key="segment.text"
                                    >
                                        <mark
                                            v-if="segment.matched"
                                            class="settings-search-match"
                                            >{{ segment.text }}</mark
                                        >
                                        <span v-else>{{ segment.text }}</span>
                                    </template>
                                </span>
                                <span
                                    v-if="result.breadcrumb"
                                    class="settings-search-result-breadcrumb"
                                >
                                    {{ result.breadcrumb }}
                                </span>
                            </span>
                            <ChevronRightIcon
                                v-if="!result.entry"
                                class="settings-search-result-chevron"
                                aria-hidden="true"
                            />
                        </button>
                    </TransitionGroup>
                    <p
                        v-if="!searchResultsPending && searchResults.length === 0"
                        class="m-0 px-3 py-4 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.appSettingsSearchEmpty) }}
                    </p>
                </div>

                <nav v-else class="settings-sidebar-list" :aria-label="formatMessage(pageTitle)">
                    <section
                        v-for="group in visibleGroups"
                        :key="group.id"
                        class="settings-nav-group"
                    >
                        <button
                            type="button"
                            class="settings-group-button hover:bg-surface-3 hover:text-[var(--color-text-primary)]"
                            :aria-expanded="expandedGroups[group.id]"
                            @click="toggleGroup(group.id)"
                        >
                            <component :is="group.icon" class="size-3.5 shrink-0" />
                            <span class="truncate">{{ formatMessage(group.name) }}</span>
                            <ChevronDownIcon
                                class="ml-auto size-3.5 shrink-0 transition-transform"
                                :class="expandedGroups[group.id] ? 'rotate-180' : ''"
                            />
                        </button>
                        <div v-show="expandedGroups[group.id]" class="settings-nav-items">
                            <button
                                v-for="category in group.categories"
                                :key="category.id"
                                type="button"
                                :data-onboarding-id="category.onboardingId"
                                class="settings-category-button hover:bg-surface-3 hover:text-[var(--color-text-primary)]"
                                :class="{ 'is-active': activeCategory?.id === category.id }"
                                @click="selectCategory(category.id)"
                            >
                                <component :is="category.icon" class="size-4 shrink-0" />
                                <span class="truncate">{{ categoryName(category) }}</span>
                            </button>
                        </div>
                    </section>
                </nav>

                <footer class="mt-auto shrink-0 pt-4 text-sm text-[var(--color-text-tertiary)]">
                    <div
                        v-if="
                            screen.updateDownload.progress > 0 && screen.updateDownload.progress < 1
                        "
                        class="mb-4"
                    >
                        <p class="m-0 mb-2">
                            {{
                                formatMessage(messages.appSettingsDownloading, {
                                    version: screen.updateDownload.version,
                                })
                            }}
                        </p>
                        <ProgressBar :progress="screen.updateDownload.progress" />
                    </div>
                    <p v-if="devMode" class="m-0 mb-3 font-semibold text-brand">
                        {{ formatMessage(messages.appSettingsDeveloperModeEnabled) }}
                    </p>
                    <div class="settings-footer-identity flex items-start gap-3">
                        <button
                            type="button"
                            class="m-0 flex size-9 shrink-0 items-center justify-center rounded-lg border-0 bg-transparent p-0 transition-colors hover:bg-surface-3"
                            :class="devMode ? 'text-brand' : 'text-[var(--color-text-tertiary)]'"
                            @click="toggleDeveloperMode"
                        >
                            <img class="size-8 object-contain" src="/axolotl.png" alt="" />
                        </button>
                        <div class="settings-footer-version min-w-0">
                            <p class="m-0 break-words">
                                {{ PREVIEW_PRODUCT_NAME }} {{ PREVIEW_APP_VERSION }}
                            </p>
                            <p class="m-0 truncate">
                                {{ PREVIEW_OS_NAME }} {{ PREVIEW_OS_VERSION }}
                            </p>
                        </div>
                    </div>
                </footer>
            </aside>

            <section class="settings-content" :aria-label="formatMessage(pageTitle)">
                <Transition name="settings-content-header" mode="out-in">
                    <header :key="activeCategory?.id ?? 'settings'" class="settings-content-header">
                        <component
                            :is="activeCategory?.icon"
                            class="size-5 text-[var(--color-text-tertiary)]"
                        />
                        <h1 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                            {{
                                activeCategory
                                    ? categoryName(activeCategory)
                                    : formatMessage(pageTitle)
                            }}
                        </h1>
                    </header>
                </Transition>
                <div
                    ref="contentContainer"
                    class="settings-content-scroll min-h-0 flex-1"
                    :class="activeCategory?.flushContent ? 'overflow-hidden' : 'overflow-y-auto'"
                >
                    <div
                        class="settings-content-stage relative min-h-0"
                        :class="activeCategory?.flushContent ? 'is-flush' : ''"
                    >
                        <Transition name="settings-content-skeleton">
                            <div
                                v-if="isContentLoading"
                                class="settings-content-skeleton"
                                aria-hidden="true"
                            >
                                <div class="settings-content-skeleton-inner">
                                    <div class="h-6 w-40 rounded bg-surface-3" />
                                    <div class="h-3 w-72 max-w-full rounded bg-surface-2" />
                                    <div class="mt-2 flex flex-col gap-3">
                                        <div
                                            v-for="row in 5"
                                            :key="row"
                                            class="flex items-center gap-3 rounded-lg border border-solid p-4"
                                            :style="{
                                                borderColor: 'var(--surface-4)',
                                                background:
                                                    'color-mix(in srgb, var(--surface-3) var(--opacity-ratio-keep-35), transparent)',
                                            }"
                                        >
                                            <div class="flex min-w-0 flex-1 flex-col gap-2">
                                                <div
                                                    class="h-4 rounded bg-surface-3"
                                                    :class="row % 2 === 0 ? 'w-2/5' : 'w-1/3'"
                                                />
                                                <div class="h-3 w-3/5 rounded bg-surface-2" />
                                            </div>
                                            <div
                                                class="h-8 w-16 shrink-0 rounded-lg bg-surface-3"
                                            />
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Transition>

                        <div
                            v-if="activeCategory"
                            :id="`settings-category-${activeCategory.id}`"
                            class="settings-content-body min-h-0"
                            :class="[
                                activeCategory.flushContent
                                    ? 'h-full'
                                    : 'mx-auto max-w-5xl px-6 pb-6',
                                isContentLoading ? 'is-loading' : 'is-ready',
                            ]"
                            tabindex="-1"
                        >
                            <Suspense
                                @pending="settingsContentPending = true"
                                @resolve="onSettingsContentResolved"
                            >
                                <component :is="activeCategory.content" :key="activeCategory.id" />
                                <template #fallback>
                                    <div class="settings-content-fallback" aria-hidden="true" />
                                </template>
                            </Suspense>
                        </div>
                    </div>
                </div>
            </section>
        </div>
    </div>
</template>

<style scoped>
.settings-layout {
    --settings-divider: color-mix(
        in srgb,
        var(--surface-4) var(--opacity-ratio-keep-55),
        transparent
    );
    display: grid;
    grid-template-columns: minmax(18rem, 20rem) minmax(0, 1fr);
    gap: var(--gap-lg);
    min-height: 0;
    overflow: hidden;
}

.settings-sidebar {
    /* The shell paints its own surface, so the page is a component layer rather
       than a window onto the content pane. */
    background: var(--surface-2);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-lg);
    display: flex;
    height: 100%;
    flex-direction: column;
    gap: 0.75rem;
    min-height: 0;
    overflow: hidden;
    padding: var(--gap-lg);
}

/* The page draws its own clear button, as the app does. */
.settings-search-input::-webkit-search-cancel-button,
.settings-search-input::-webkit-search-decoration {
    appearance: none;
    display: none;
}

.settings-sidebar-list {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    gap: var(--gap-lg);
    overflow-y: auto;
}

.settings-nav-group {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: var(--gap-xs);
}

.settings-nav-items {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: var(--gap-xs);
}

.settings-group-button {
    display: flex;
    width: 100%;
    align-items: center;
    gap: var(--gap-sm);
    min-height: 1.75rem;
    padding: 0 var(--gap-sm);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
    transition:
        background-color 120ms ease,
        color 120ms ease;
}

.settings-category-button {
    display: flex;
    width: 100%;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
    transition:
        background-color 120ms ease,
        color 120ms ease;
    align-items: center;
    min-height: 2.25rem;
    gap: var(--gap-sm);
    padding: 0 var(--gap-sm);
    font-size: 0.875rem;
    font-weight: 600;
}

.settings-category-button.is-active {
    /* A brand token on purpose: the active item stays emphasised when the
       panels fade over a background image. */
    background: var(--color-button-bg-selected);
    color: var(--color-button-text-selected);
}

.settings-search-results {
    position: relative;
    gap: var(--gap-xs);
}

.settings-search-result {
    display: flex;
    width: 100%;
    align-items: flex-start;
    gap: var(--gap-sm);
    padding: 0.55rem 0.65rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
    transition:
        background-color 140ms ease,
        color 140ms ease,
        transform 140ms ease,
        box-shadow 140ms ease;
}

.settings-search-result:hover {
    background: var(--surface-3);
    color: var(--color-text-primary);
}

.settings-search-result:active {
    background: var(--surface-4);
    transform: scale(0.985);
}

.settings-search-result:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--color-brand) 55%, transparent);
    outline-offset: 1px;
}

.settings-search-result.is-entry {
    padding-left: 0.85rem;
}

.settings-search-result-icon {
    width: 1rem;
    height: 1rem;
    flex-shrink: 0;
    margin-top: 0.15rem;
    color: var(--color-text-tertiary);
    transition: color 140ms ease;
}

.settings-search-result.is-category .settings-search-result-icon {
    color: var(--color-text-primary);
}

.settings-search-result:hover .settings-search-result-icon {
    color: var(--color-brand);
}

.settings-search-result-copy {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.15rem;
}

.settings-search-result-label {
    overflow: hidden;
    color: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.settings-search-result.is-entry .settings-search-result-label {
    font-weight: 500;
}

.settings-search-result-breadcrumb {
    overflow: hidden;
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.settings-search-result-chevron {
    width: 0.875rem;
    height: 0.875rem;
    flex-shrink: 0;
    margin-top: 0.2rem;
    color: var(--color-text-tertiary);
    opacity: 0.7;
    transition:
        color 140ms ease,
        opacity 140ms ease,
        transform 140ms ease;
}

.settings-search-result:hover .settings-search-result-chevron {
    color: var(--color-brand);
    opacity: 1;
    transform: translateX(1px);
}

.settings-search-match {
    background: transparent;
    color: var(--color-brand);
    padding: 0;
    font-weight: 700;
}

.settings-search-enter-active {
    transition:
        opacity 180ms ease var(--search-stagger, 0ms),
        transform 180ms ease var(--search-stagger, 0ms);
}

.settings-search-enter-from {
    opacity: 0;
    transform: translateY(-6px);
}

.settings-search-leave-active {
    position: absolute;
    width: 100%;
    pointer-events: none;
    transition: opacity 100ms ease;
}

.settings-search-leave-to {
    opacity: 0;
}

.settings-search-move {
    transition: transform 160ms ease;
}

.settings-search-skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--gap-xs);
    padding: 0.15rem 0;
}

.settings-search-skeleton-row {
    display: flex;
    align-items: flex-start;
    gap: var(--gap-sm);
    padding: 0.55rem 0.65rem;
}

.settings-search-skeleton-icon {
    width: 1rem;
    height: 1rem;
    flex-shrink: 0;
    margin-top: 0.15rem;
    border-radius: 9999px;
    background: var(--surface-3);
}

.settings-search-skeleton-copy {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.35rem;
}

.settings-search-skeleton-line {
    height: 0.75rem;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
}

.settings-search-skeleton-line.is-wide {
    width: 72%;
    height: 0.875rem;
}

.settings-search-skeleton-line.is-medium {
    width: 55%;
    height: 0.875rem;
}

.settings-search-skeleton-line.is-short {
    width: 40%;
    background: var(--surface-2);
}

.settings-content {
    /* The component layer the category content sits on. The height chain the
       flush categories rely on resolves against this box, so it takes padding
       and a radius but never a margin. */
    background: var(--surface-2);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-lg);
    display: flex;
    min-width: 0;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
}

.settings-content-stage {
    min-height: 100%;
}

/* Flush categories (AI features) manage their own inner scrolling, so the stage
   needs a definite height for their percentage heights to resolve against. */
.settings-content-stage.is-flush {
    height: 100%;
    min-height: 0;
}

.settings-content-skeleton {
    position: absolute;
    inset: 0;
    z-index: 2;
    padding: 0 1.5rem 1.5rem;
    /* A full-cover overlay on the content panel, so it takes the panel's own
       surface rather than flashing the page colour on every switch. */
    background: var(--surface-2);
}

.settings-content-skeleton-inner {
    display: flex;
    max-width: 64rem;
    margin: 0 auto;
    flex-direction: column;
    gap: 0.75rem;
}

.settings-content-body {
    transition: opacity 180ms ease;
}

.settings-content-body:focus,
.settings-content-body:focus-visible {
    outline: none;
}

.settings-content-body.is-loading {
    opacity: 0;
    pointer-events: none;
}

.settings-content-body.is-ready {
    opacity: 1;
}

.settings-content-skeleton-enter-active,
.settings-content-skeleton-leave-active {
    transition: opacity 160ms ease;
}

.settings-content-skeleton-enter-from,
.settings-content-skeleton-leave-to {
    opacity: 0;
}

.settings-content-header-enter-active,
.settings-content-header-leave-active {
    transition:
        opacity 140ms ease,
        transform 140ms ease;
}

.settings-content-header-enter-from,
.settings-content-header-leave-to {
    opacity: 0;
    transform: translateY(-4px);
}

.settings-content-fallback {
    min-height: 100%;
}

.settings-footer-identity {
    min-width: 0;
}

.settings-footer-version {
    min-width: 0;
    line-height: 1.4;
}

.settings-content-header {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: var(--gap-md);
    padding: var(--gap-xl) var(--gap-xl) var(--gap-lg);
}

.settings-content-scroll :deep([id^='settings-target-']),
.settings-content-scroll :deep([id^='settings-category-']) {
    scroll-margin-top: 1.5rem;
}

.settings-content-scroll :deep(.settings-search-result-highlight) {
    border-radius: var(--radius-sm);
    animation: settings-search-result-highlight 0.9s ease-in-out 2;
}

@keyframes settings-search-result-highlight {
    0%,
    100% {
        background: transparent;
    }

    50% {
        background: var(--surface-3);
    }
}
</style>

<style>
/* The app's global rule: while the settings page is mounted, the shell's
   viewport does not scroll and its transition layers fill it, so the page's
   own two panels own scrolling. */
.launcher-preview-window .app-viewport:has(.settings-fixed-render) {
    overflow: hidden;
    scrollbar-gutter: auto;
}

.launcher-preview-window .app-viewport:has(.settings-fixed-render) .page-transition-grid,
.launcher-preview-window .app-viewport:has(.settings-fixed-render) .page-transition-layer {
    height: 100%;
    min-height: 0;
}

/* The app's global `input` / `textarea` rules (apps/app-frontend/src/assets/
   stylesheets/global.scss), which every raw field and StyledInput of the
   settings pages sits on. The website does not load them, so they are given
   here at the same element-level specificity, for the page and for the
   settings modals (which carry `settings-app-inputs`). */
:where(.settings-fixed-render, .settings-app-inputs)
    input:where(
        :not([type='checkbox'], [type='radio'], [type='range'], [type='file'], [type='color'])
    ),
:where(.settings-fixed-render, .settings-app-inputs) textarea {
    background: var(--surface-3);
    color: var(--color-text-default);
    padding: 0.5rem 1rem;
    font-weight: var(--font-weight-medium, 500);
    border: none;
    border-radius: var(--radius-md);
    box-sizing: border-box;
    appearance: none !important;
    outline: 2px solid transparent;
    box-shadow:
        var(--shadow-inset-sm),
        0 0 0 0 transparent;
    transition: box-shadow 0.1s ease-in-out;
    min-height: 36px;
}

:where(.settings-fixed-render, .settings-app-inputs)
    input:where(
        :not([type='checkbox'], [type='radio'], [type='range'], [type='file'], [type='color'])
    ):is(:focus, :focus-visible),
:where(.settings-fixed-render, .settings-app-inputs) textarea:is(:focus, :focus-visible) {
    box-shadow:
        inset 0 0 0 transparent,
        0 0 0 0.25rem var(--color-brand-shadow);
    color: var(--color-button-text-active);
}

:where(.settings-fixed-render, .settings-app-inputs) :is(input, textarea):disabled {
    opacity: 0.6;
    pointer-events: none;
    cursor: not-allowed;
}

:where(.settings-fixed-render, .settings-app-inputs) :is(input, textarea)::placeholder {
    color: var(--color-button-text);
    opacity: 0.6;
}

:where(.settings-fixed-render, .settings-app-inputs) :is(input, textarea):focus::placeholder {
    opacity: 0.8;
}
</style>
