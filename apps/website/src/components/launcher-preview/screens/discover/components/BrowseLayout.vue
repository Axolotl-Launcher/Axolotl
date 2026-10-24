<script setup lang="ts">
import ArrowUpDownIcon from '@modrinth/assets/icons/arrow-up-down.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox, { type ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import Pagination from '@modrinth/ui/src/components/base/Pagination.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import ContentCardReveal from '@modrinth/ui/src/components/project/ContentCardReveal.vue'
import ProjectCardList from '@modrinth/ui/src/components/project/ProjectCardList.vue'
import ProjectCardSkeleton from '@modrinth/ui/src/components/project/ProjectCardSkeleton.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useStickyObserver } from '@modrinth/ui/src/composables/sticky-observer.ts'
import {
    commonMessages,
    formatProjectTypeSentence,
} from '@modrinth/ui/src/utils/common-messages.ts'
import type { SortType } from '@modrinth/ui/src/utils/search.ts'
import { computed, ref } from 'vue'

import { injectPreviewWindow } from '../../../lib/window'
import { injectBrowseContext } from '../browse-context'
import BrowseInstallHeader from './BrowseInstallHeader.vue'
import ProjectCard from './ProjectCard.vue'
import SearchFilterControl from './SearchFilterControl.vue'
import SelectedProjectsFloatingBar from './SelectedProjectsFloatingBar.vue'

/**
 * Port of `BrowsePageLayout` (packages/ui/src/layouts/shared/browse-tab/layout.vue) in its app
 * variant: sticky toolbar chrome, tabs, search, sort / page size / display mode, pagination,
 * active filter chips, then loading skeletons, empty and error states or the result cards.
 */
const ctx = injectBrowseContext()
const { formatMessage } = useVIntl()
const { layer } = injectPreviewWindow()
const stickyChromeRef = ref<HTMLElement | null>(null)
const { isStuck: isChromeStuck } = useStickyObserver(stickyChromeRef, 'BrowseToolbarChrome')
const showInstallHeader = computed(
    () => !!ctx.installContext.value && ctx.installContext.value.showInstallHeader !== false,
)

const text = defineMessages({
    searchPlaceholder: {
        id: 'browse.search.placeholder',
        defaultMessage: 'Search {projectType}...',
    },
    viewPrefix: { id: 'browse.view-prefix', defaultMessage: 'View:' },
    offline: {
        id: 'browse.offline',
        defaultMessage: 'You are currently offline. Connect to the internet to browse Modrinth!',
    },
    offlineHeading: { id: 'browse.offline.heading', defaultMessage: 'No connection' },
    noResults: { id: 'browse.no-results', defaultMessage: 'No results found for your query!' },
    noResultsHeading: { id: 'browse.no-results.heading', defaultMessage: 'Nothing here yet' },
    clearSearchLabel: { id: 'browse.clear-search', defaultMessage: 'Clear search' },
    retryLabel: { id: 'browse.retry', defaultMessage: 'Try again' },
    sortRelevance: { id: 'browse.sort.relevance', defaultMessage: 'Relevance' },
    sortDownloads: { id: 'browse.sort.downloads', defaultMessage: 'Downloads' },
    sortFollowers: { id: 'browse.sort.followers', defaultMessage: 'Followers' },
    sortDatePublished: { id: 'browse.sort.date-published', defaultMessage: 'Date published' },
    sortProjectPublished: {
        id: 'browse.sort.project-published',
        defaultMessage: 'Project first published',
    },
    sortDateUpdated: { id: 'browse.sort.date-updated', defaultMessage: 'Date updated' },
    sortCreated: { id: 'browse.sort.project-created', defaultMessage: 'Project created' },
    sortModified: { id: 'browse.sort.project-modified', defaultMessage: 'Project modified' },
    sortFirstRelease: {
        id: 'browse.sort.first-release',
        defaultMessage: 'First release published',
    },
    sortLatestRelease: {
        id: 'browse.sort.latest-release',
        defaultMessage: 'Latest release published',
    },
    sortVerifiedPlays: { id: 'browse.sort.verified-plays', defaultMessage: 'Verified plays' },
    sortPlayers: { id: 'browse.sort.players', defaultMessage: 'Players' },
})

function formatSortType(sortType: SortType): string {
    const sortMessages: Record<string, { id: string; defaultMessage: string }> = {
        relevance: text.sortRelevance,
        downloads: text.sortDownloads,
        follows: text.sortFollowers,
        newest: text.sortProjectPublished,
        created: text.sortCreated,
        updated: text.sortModified,
        firstPublished: text.sortFirstRelease,
        latestPublished: text.sortLatestRelease,
        'minecraft_java_server.verified_plays_2w': text.sortVerifiedPlays,
        'minecraft_java_server.ping.data.players_online': text.sortPlayers,
        date_created: text.sortDatePublished,
        date_modified: text.sortDateUpdated,
    }
    const message = sortMessages[sortType.name]
    return message ? formatMessage(message) : sortType.display
}

const sortTypeOptions = computed<ComboboxOption<string>[]>(() =>
    ctx.effectiveSortTypes.value.map((sortType) => ({
        value: sortType.name,
        label: formatSortType(sortType),
    })),
)

const maxResultsOptions = computed<ComboboxOption<number>[]>(() =>
    [5, 10, 15, 20, 50, 100].map((value) => ({ value, label: String(value) })),
)

const currentSortTypeName = computed<string>({
    get: () => ctx.effectiveCurrentSortType.value?.name ?? '',
    set: (name: string) => {
        const sortType = ctx.effectiveSortTypes.value.find((candidate) => candidate.name === name)
        if (sortType) ctx.effectiveCurrentSortType.value = sortType
    },
})

const selectedDisplayMode = computed(() =>
    ctx.displayModeOptions.value.find((option) => option.id === ctx.displayMode.value),
)

const skeletonCount = computed(() => {
    if (ctx.effectiveLayout.value === 'grid') return 6
    return Math.min(ctx.maxResults.value ?? 8, 8)
})

function actionType(type?: string, color?: string) {
    return type === 'transparent'
        ? 'quiet'
        : type === 'outlined'
          ? 'outlined'
          : color
            ? 'colored'
            : 'base'
}

function cardEnvironment(result: { client_side: string; server_side: string }) {
    return ['mod', 'modpack'].includes(ctx.projectType.value)
        ? {
              clientSide: result.client_side as 'required' | 'optional' | 'unsupported' | 'unknown',
              serverSide: result.server_side as 'required' | 'optional' | 'unsupported' | 'unknown',
          }
        : undefined
}

function cardDates(result: (typeof ctx.projectHits.value)[number]) {
    const dates = result.resource_dates
    switch (ctx.effectiveCurrentSortType.value.name) {
        case 'newest':
            return {
                datePublished: dates.projectPublishedAt ?? undefined,
                displayedDate: 'projectPublished' as const,
            }
        case 'created':
            return {
                datePublished: dates.createdAt ?? undefined,
                displayedDate: 'created' as const,
            }
        case 'firstPublished':
            return {
                datePublished: dates.firstPublishedAt ?? undefined,
                displayedDate: 'firstRelease' as const,
            }
        case 'latestPublished':
            return {
                dateUpdated: dates.latestPublishedAt ?? undefined,
                displayedDate: 'releaseUpdated' as const,
            }
        default:
            return {
                dateUpdated: dates.updatedAt ?? undefined,
                displayedDate: 'projectModified' as const,
            }
    }
}
</script>

<template>
    <div
        ref="stickyChromeRef"
        class="browse-toolbar-chrome sticky top-0 z-20 -mx-6 -mt-6 flex flex-col gap-3 border-0 border-b border-solid border-surface-5 bg-[color-mix(in_srgb,var(--surface-1-opaque,var(--surface-1))_20%,transparent)] backdrop-blur-xl"
        :class="isChromeStuck ? 'border-t shadow-[0_8px_16px_-12px_rgba(0,0,0,0.35)]' : ''"
    >
        <div v-if="showInstallHeader" class="p-3 pb-0">
            <BrowseInstallHeader :install-context="ctx.installContext.value" />
        </div>
        <div class="flex flex-col gap-3 p-3">
            <div class="flex min-w-0 flex-wrap items-center gap-2">
                <NavTabs
                    v-if="ctx.showProjectTypeTabs.value"
                    :links="ctx.selectableProjectTypes.value"
                    class="min-w-0 max-w-full"
                />
                <div class="ml-auto shrink-0">
                    <slot name="nav-tabs-actions" />
                </div>
            </div>

            <div class="flex min-w-0 flex-wrap items-center gap-2">
                <StyledInput
                    v-model="ctx.query.value"
                    :icon="SearchIcon"
                    type="text"
                    autocomplete="off"
                    :placeholder="
                        formatMessage(text.searchPlaceholder, {
                            projectType: formatProjectTypeSentence(
                                formatMessage,
                                ctx.projectType.value,
                                2,
                            ),
                        })
                    "
                    clearable
                    wrapper-class="min-w-0 flex-[1_1_16rem]"
                    input-class="h-12"
                    @clear="ctx.clearSearch()"
                />
                <slot name="search-bar-actions" />
            </div>

            <div class="flex flex-wrap items-center gap-2">
                <Combobox
                    v-model="currentSortTypeName"
                    v-tooltip="{
                        content: formatMessage(commonMessages.sortByLabel),
                        triggers: ['hover'],
                    }"
                    width="content"
                    :options="sortTypeOptions"
                    trigger-class="!min-h-10"
                >
                    <template #prefix>
                        <ArrowUpDownIcon class="size-5 shrink-0 text-[var(--color-text-default)]" />
                    </template>
                    <template #selected="{ label }">
                        <span class="font-semibold text-[var(--color-text-tertiary)]">{{
                            label
                        }}</span>
                    </template>
                </Combobox>

                <Combobox
                    v-model="ctx.maxResults.value"
                    v-tooltip="{ content: formatMessage(text.viewPrefix), triggers: ['hover'] }"
                    width="content"
                    :options="maxResultsOptions"
                    trigger-class="!min-h-10"
                >
                    <template #prefix>
                        <EyeIcon class="size-5 shrink-0 text-[var(--color-text-default)]" />
                    </template>
                    <template #selected="{ label }">
                        <span class="font-semibold text-[var(--color-text-tertiary)]">{{
                            label
                        }}</span>
                    </template>
                </Combobox>

                <PopoutMenu
                    :tooltip="ctx.displayModeTooltip.value"
                    placement="bottom-end"
                    :container="layer ?? undefined"
                >
                    <Button circular icon-only :aria-label="ctx.displayModeTooltip.value"
                        ><component :is="selectedDisplayMode?.icon" />
                    </Button>
                    <template #menu>
                        <div class="flex w-44 flex-col gap-1 p-1">
                            <Button
                                v-for="option in ctx.displayModeOptions.value"
                                :key="option.id"
                                :type="ctx.displayMode.value === option.id ? 'base' : 'quiet'"
                                class="flex w-full items-center !justify-start gap-2 text-left"
                                :aria-pressed="ctx.displayMode.value === option.id"
                                @click="ctx.setDisplayMode(option.id)"
                            >
                                <component :is="option.icon" class="h-4 w-4" />
                                <span>{{ option.label }}</span>
                            </Button>
                        </div>
                    </template>
                </PopoutMenu>

                <Pagination
                    :page="ctx.currentPage.value"
                    :count="ctx.pageCount.value"
                    :loading="ctx.loading.value"
                    class="ml-auto"
                    @switch-page="ctx.setPage"
                />
            </div>

            <SearchFilterControl
                v-if="ctx.isServerType.value"
                v-model:selected-filters="ctx.serverCurrentFilters.value"
                :filters="ctx.serverFilterTypes.value"
                :provided-filters="[]"
                :overridden-provided-filter-types="[]"
                :project-type="ctx.projectType.value"
            />
            <SearchFilterControl
                v-else
                v-model:selected-filters="ctx.currentFilters.value"
                :filters="ctx.filters.value.filter((filter) => filter.display !== 'none')"
                :provided-filters="ctx.providedFilters.value"
                :overridden-provided-filter-types="ctx.overriddenProvidedFilterTypes.value"
                :project-type="ctx.projectType.value"
                :provided-message="ctx.lockedFilterMessages.value.providedBy"
            />
        </div>
    </div>

    <SelectedProjectsFloatingBar
        v-if="ctx.installContext.value"
        :install-context="ctx.installContext.value"
    />

    <slot name="above-results" />

    <div
        v-if="ctx.searchError.value"
        role="alert"
        class="mb-3 flex items-center gap-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-3"
    >
        <span>{{ ctx.searchError.value }}</span>
        <Button @click="ctx.refreshSearch()">{{ formatMessage(text.retryLabel) }}</Button>
    </div>
    <div class="search relative" :aria-busy="ctx.loading.value">
        <section v-if="ctx.loading.value" class="py-1" aria-busy="true" aria-live="polite">
            <ProjectCardList :layout="ctx.effectiveLayout.value">
                <ProjectCardSkeleton
                    v-for="index in skeletonCount"
                    :key="`skeleton-${index}`"
                    :layout="ctx.effectiveLayout.value"
                />
            </ProjectCardList>
        </section>
        <section
            v-else-if="
                !ctx.searchError.value &&
                (ctx.isServerType.value
                    ? ctx.serverHits.value.length === 0
                    : ctx.projectHits.value.length === 0)
            "
            class="py-8"
        >
            <EmptyState
                type="no-results"
                compact
                :heading="formatMessage(text.noResultsHeading)"
                :description="formatMessage(text.noResults)"
            >
                <template v-if="ctx.query.value" #actions>
                    <Button @click="ctx.clearSearch()">{{
                        formatMessage(text.clearSearchLabel)
                    }}</Button>
                </template>
            </EmptyState>
        </section>

        <ProjectCardList v-else :layout="ctx.effectiveLayout.value">
            <template v-if="!ctx.isServerType.value">
                <ContentCardReveal
                    v-for="result in ctx.projectHits.value"
                    :key="`${result.provider}:${result.project_id}`"
                    class="[content-visibility:auto]"
                    :style="{
                        containIntrinsicSize:
                            ctx.effectiveLayout.value === 'compact' ? 'auto 72px' : 'auto 180px',
                    }"
                >
                    <ProjectCard
                        :link="ctx.getProjectLink(result)"
                        :title="result.title"
                        :icon-url="result.icon_url"
                        v-bind="cardDates(result)"
                        :downloads="result.downloads"
                        :summary="result.description"
                        :tags="result.display_categories"
                        :all-tags="result.categories"
                        :deprioritized-tags="ctx.deprioritizedTags.value"
                        :exclude-loaders="ctx.excludeLoaders.value"
                        :banner="result.featured_gallery ?? undefined"
                        :color="result.color ?? undefined"
                        :provider="result.provider"
                        :providers="result.providers"
                        :environment="cardEnvironment(result)"
                        :layout="ctx.effectiveLayout.value"
                        @contextmenu.prevent.stop="
                            (event: MouseEvent) => ctx.onContextMenu(event, result)
                        "
                    >
                        <template
                            v-if="ctx.getCardActions(result, ctx.projectType.value).length"
                            #actions
                        >
                            <div class="flex gap-2">
                                <Button
                                    v-for="action in ctx.getCardActions(
                                        result,
                                        ctx.projectType.value,
                                    )"
                                    :key="action.key"
                                    v-tooltip="action.tooltip"
                                    :color="action.color"
                                    :type="actionType(action.type, action.color)"
                                    :size="ctx.effectiveLayout.value === 'compact' ? '2xs' : 'md'"
                                    :circular="action.circular"
                                    :icon-only="action.circular"
                                    :label="action.circular ? action.label : undefined"
                                    :disabled="action.disabled"
                                    @click.stop="action.onClick"
                                >
                                    <component :is="action.icon" :class="action.iconClass" />
                                    <template v-if="!action.circular">{{
                                        ctx.effectiveLayout.value === 'compact'
                                            ? (action.compactLabel ?? action.label)
                                            : action.label
                                    }}</template>
                                </Button>
                            </div>
                        </template>
                    </ProjectCard>
                </ContentCardReveal>
            </template>
        </ProjectCardList>

        <div class="mt-3 flex justify-end">
            <Pagination
                :page="ctx.currentPage.value"
                :count="ctx.pageCount.value"
                :loading="ctx.loading.value"
                class="pagination-after"
                @switch-page="ctx.setPage"
            />
        </div>
    </div>

    <slot name="after" />
</template>
