<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { computed, toValue } from 'vue'

import Button from '#ui/components/base/buttons/Button.vue'
import type { ComboboxOption } from '#ui/components/base/Combobox.vue'
import EmptyState from '#ui/components/base/EmptyState.vue'
import Pagination from '#ui/components/base/Pagination.vue'
import ProjectCard from '#ui/components/project/card/ProjectCard.vue'
import ContentCardReveal from '#ui/components/project/ContentCardReveal.vue'
import ProjectCardList from '#ui/components/project/ProjectCardList.vue'
import ProjectCardSkeleton from '#ui/components/project/ProjectCardSkeleton.vue'
import SearchFilterControl from '#ui/components/search/SearchFilterControl.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { formatProjectTypeSentence } from '#ui/utils/common-messages'
import type { SortType } from '#ui/utils/search'

import BrowseResultControls from './components/BrowseResultControls.vue'
import SelectedProjectsFloatingBar from './components/SelectedProjectsFloatingBar.vue'
import { projectCardDates } from './composables/project-dates'
import BrowseInstallHeader from './header.vue'
import { injectBrowseManager } from './providers/browse-manager'
import BrowseToolbar from './toolbar.vue'
import type { BrowseDisplayMode } from './types'

const ctx = injectBrowseManager()
// The provider context narrows this to list/grid, but the composable can also
// produce compact and gallery layouts through the same value.
const effectiveLayout = computed<BrowseDisplayMode>(() => ctx.effectiveLayout.value)
const { formatMessage } = useVIntl()
const lockedMessages = computed(() => toValue(ctx.lockedFilterMessages))
const showInstallHeader = computed(
    () =>
        !!ctx.installContext?.value &&
        ctx.installContext.value.showInstallHeader !== false &&
        ctx.variant !== 'web',
)

const messages = defineMessages({
    searchPlaceholder: {
        id: 'browse.search.placeholder',
        defaultMessage: 'Search {projectType}...',
    },
    filterResults: {
        id: 'browse.filter-results',
        defaultMessage: 'Filter results...',
    },
    offline: {
        id: 'browse.offline',
        defaultMessage: 'You are currently offline. Connect to the internet to browse Modrinth!',
    },
    offlineHeading: {
        id: 'browse.offline.heading',
        defaultMessage: 'No connection',
    },
    noResults: {
        id: 'browse.no-results',
        defaultMessage: 'No results found for your query!',
    },
    noResultsHeading: {
        id: 'browse.no-results.heading',
        defaultMessage: 'Nothing here yet',
    },
    clearSearchLabel: {
        id: 'browse.clear-search',
        defaultMessage: 'Clear search',
    },
    retryLabel: {
        id: 'browse.retry',
        defaultMessage: 'Try again',
    },
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
    const sortMessages = {
        relevance: messages.sortRelevance,
        downloads: messages.sortDownloads,
        follows: messages.sortFollowers,
        newest: ctx.variant === 'app' ? messages.sortProjectPublished : messages.sortDatePublished,
        created: messages.sortCreated,
        updated: ctx.variant === 'app' ? messages.sortModified : messages.sortDateUpdated,
        firstPublished: messages.sortFirstRelease,
        latestPublished: messages.sortLatestRelease,
        'minecraft_java_server.verified_plays_2w': messages.sortVerifiedPlays,
        'minecraft_java_server.ping.data.players_online': messages.sortPlayers,
        date_created: messages.sortDatePublished,
        date_modified: messages.sortDateUpdated,
    }

    const message = sortMessages[sortType.name as keyof typeof sortMessages]
    return message ? formatMessage(message) : sortType.display
}

const sortTypeOptions = computed<ComboboxOption<string>[]>(() =>
    ctx.effectiveSortTypes.value.map((sortType) => ({
        value: sortType.name,
        label: formatSortType(sortType),
    })),
)

const currentSortTypeName = computed<string>({
    get: () => ctx.effectiveCurrentSortType.value?.name ?? '',
    set: (name: string) => {
        const sortType = ctx.effectiveSortTypes.value.find((st) => st.name === name)
        if (sortType) {
            ctx.effectiveCurrentSortType.value = sortType
        }
    },
})

const resultContext = computed(() => ctx.committed?.value)
const resultProjectType = computed(() => resultContext.value?.projectType ?? ctx.projectType.value)
const resultIsServer = computed(() => resultProjectType.value === 'server')

const skeletonCount = computed(() => {
    const layout = effectiveLayout.value
    if (layout === 'grid' || layout === 'gallery') return 6
    // Match the visible list density without stacking a full viewport of rows.
    return Math.min(ctx.maxResults?.value ?? 8, 8)
})
</script>

<template>
    <BrowseToolbar
        v-model:query="ctx.query.value"
        :variant="ctx.variant"
        :tabs="ctx.selectableProjectTypes.value"
        :show-tabs="ctx.showProjectTypeTabs.value"
        :disabled="ctx.filtersReady?.value === false"
        :placeholder="
            formatMessage(messages.searchPlaceholder, {
                projectType: formatProjectTypeSentence(formatMessage, ctx.projectType.value, 2),
            })
        "
        @clear="ctx.clearSearch()"
    >
        <template v-if="showInstallHeader" #install-header><BrowseInstallHeader /></template>
        <template #nav-tabs-actions><slot name="nav-tabs-actions" /></template>
        <template #search-bar-actions><slot name="search-bar-actions" /></template>
        <template #controls>
            <BrowseResultControls
                v-model:sort="currentSortTypeName"
                v-model:page-size="ctx.maxResults.value"
                :variant="ctx.variant"
                :sort-options="sortTypeOptions"
                :page-size-options="ctx.maxResultsOptions?.value"
                :disabled="ctx.filtersReady?.value === false"
                :display-mode="ctx.displayMode?.value"
                :display-mode-options="
                    ctx.setDisplayMode ? ctx.displayModeOptions?.value : undefined
                "
                :display-mode-tooltip="ctx.displayModeTooltip?.value"
                :page="resultContext?.page ?? ctx.currentPage.value"
                :page-count="ctx.pageCount.value"
                :page-count-known="ctx.pageCountKnown?.value ?? true"
                :loading="ctx.loading.value"
                @display-mode="ctx.setDisplayMode?.($event)"
                @switch-page="ctx.setPage"
            >
                <template #filters-button>
                    <div v-if="ctx.filtersMenuOpen && !ctx.filtersMenuOpen.value" class="lg:hidden">
                        <Button @click="ctx.filtersMenuOpen.value = true">{{
                            formatMessage(messages.filterResults)
                        }}</Button>
                    </div>
                </template>
            </BrowseResultControls>
        </template>
        <template #filters>
            <SearchFilterControl
                v-if="ctx.filtersReady?.value !== false && ctx.isServerType.value"
                v-model:selected-filters="ctx.serverCurrentFilters.value"
                :filters="ctx.serverFilterTypes.value"
                :provided-filters="[]"
                :overridden-provided-filter-types="[]"
                :project-type="ctx.projectType.value"
            />
            <SearchFilterControl
                v-else-if="ctx.filtersReady?.value !== false"
                v-model:selected-filters="ctx.currentFilters.value"
                :filters="
                    ctx.filters.value.filter(
                        (f) =>
                            f.display !== 'none' &&
                            !(ctx.hiddenFilterTypes?.value ?? []).includes(f.id),
                    )
                "
                :provided-filters="ctx.providedFilters?.value ?? []"
                :overridden-provided-filter-types="ctx.overriddenProvidedFilterTypes.value"
                :project-type="ctx.projectType.value"
                :provided-message="lockedMessages?.providedBy"
            />
        </template>
    </BrowseToolbar>

    <SelectedProjectsFloatingBar v-if="ctx.installContext?.value && ctx.variant !== 'web'" />

    <slot name="above-results" />

    <div
        v-if="ctx.searchError?.value"
        role="alert"
        class="mb-3 flex items-center gap-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-3"
    >
        <span>{{ ctx.searchError.value }}</span>
        <Button @click="ctx.refreshSearch()">{{ formatMessage(messages.retryLabel) }}</Button>
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
        <section v-else-if="ctx.offline?.value && ctx.totalHits.value === 0" class="py-8">
            <EmptyState type="offline" compact :heading="formatMessage(messages.offlineHeading)">
                <template #description>{{ formatMessage(messages.offline) }}</template>
                <template #actions>
                    <Button @click="ctx.refreshSearch()"
                        >{{ formatMessage(messages.retryLabel) }}
                    </Button>
                </template>
            </EmptyState>
        </section>
        <section
            v-else-if="
                !ctx.searchError?.value &&
                (resultIsServer
                    ? ctx.serverHits.value.length === 0
                    : ctx.projectHits.value.length === 0)
            "
            class="py-8"
        >
            <EmptyState
                type="no-results"
                compact
                :heading="formatMessage(messages.noResultsHeading)"
                :description="formatMessage(messages.noResults)"
            >
                <template v-if="ctx.query.value" #actions>
                    <Button @click="ctx.clearSearch()"
                        >{{ formatMessage(messages.clearSearchLabel) }}
                    </Button>
                </template>
            </EmptyState>
        </section>

        <ProjectCardList v-else :layout="ctx.effectiveLayout.value">
            <template v-if="resultIsServer">
                <ContentCardReveal
                    v-for="result in ctx.serverHits.value"
                    :key="`server-card-${result.project_id}`"
                >
                    <ProjectCard
                        :title="result.name"
                        :icon-url="result.icon_url || undefined"
                        :summary="result.summary"
                        :tags="result.categories"
                        :link="ctx.getServerProjectLink(result)"
                        :server-online-players="
                            result.minecraft_java_server?.ping?.data?.players_online ?? 0
                        "
                        :server-region="result.minecraft_server?.region"
                        :server-recent-plays="result.minecraft_java_server?.verified_plays_2w ?? 0"
                        :server-modpack-content="ctx.getServerModpackContent?.(result)"
                        :server-ping="ctx.serverPings?.value?.[result.project_id]"
                        :server-status-online="!!result.minecraft_java_server?.ping?.data"
                        :hide-online-players-label="ctx.variant === 'app'"
                        :hide-recent-plays-label="ctx.variant === 'app'"
                        :layout="ctx.effectiveLayout.value"
                        :max-tags="2"
                        is-server-project
                        exclude-loaders
                        :color="result.color ?? undefined"
                        :banner="result.featured_gallery ?? undefined"
                        @contextmenu.prevent.stop="
                            (event: MouseEvent) => ctx.onContextMenu?.(event, result)
                        "
                        @mouseenter="ctx.onServerProjectHover?.(result)"
                        @mouseleave="ctx.onProjectHoverEnd?.()"
                    >
                        <template
                            v-if="ctx.getCardActions?.(result, resultProjectType)?.length"
                            #actions
                        >
                            <div class="flex gap-2">
                                <Button
                                    v-for="action in ctx.getCardActions(result, resultProjectType)"
                                    :key="action.key"
                                    v-tooltip="action.tooltip"
                                    :color="action.color"
                                    :type="
                                        action.type === 'transparent'
                                            ? 'quiet'
                                            : action.type === 'outlined'
                                              ? 'outlined'
                                              : action.color
                                                ? 'colored'
                                                : 'base'
                                    "
                                    :size="effectiveLayout === 'compact' ? '2xs' : 'md'"
                                    :circular="action.circular"
                                    :icon-only="action.circular"
                                    :label="action.circular ? action.label : undefined"
                                    :disabled="action.disabled || ctx.loading.value"
                                    @click.stop="action.onClick"
                                >
                                    <component :is="action.icon" :class="action.iconClass" />
                                    <template v-if="!action.circular">{{
                                        effectiveLayout === 'compact'
                                            ? (action.compactLabel ?? action.label)
                                            : action.label
                                    }}</template>
                                </Button>
                            </div>
                        </template>
                    </ProjectCard>
                </ContentCardReveal>
            </template>
            <template v-else>
                <ContentCardReveal
                    v-for="result in ctx.projectHits.value"
                    :key="`${result.provider}:${result.project_id}`"
                >
                    <ProjectCard
                        :link="ctx.getProjectLink(result)"
                        :title="result.title"
                        :icon-url="result.icon_url"
                        :author="{
                            name: result.organization == null ? result.author : result.organization,
                            link:
                                result.provider === 'curseforge'
                                    ? result.author_url
                                    : result.provider === 'modrinth'
                                      ? result.organization_id == null
                                          ? ctx.variant === 'web'
                                              ? `/user/${result.author_id ?? result.author}`
                                              : `https://modrinth.com/user/${result.author_id ?? result.author}`
                                          : ctx.variant === 'web'
                                            ? `/organization/${result.organization_id}`
                                            : `https://modrinth.com/organization/${result.organization_id}`
                                      : undefined,
                        }"
                        v-bind="
                            projectCardDates(
                                result,
                                resultContext?.sortType.name ??
                                    ctx.effectiveCurrentSortType.value.name,
                            )
                        "
                        :downloads="result.downloads"
                        :summary="result.description"
                        :tags="result.display_categories"
                        :all-tags="result.categories"
                        :deprioritized-tags="
                            resultContext?.deprioritizedTags ?? ctx.deprioritizedTags.value
                        "
                        :exclude-loaders="resultContext?.excludeLoaders ?? ctx.excludeLoaders.value"
                        :banner="result.featured_gallery ?? undefined"
                        :color="result.color ?? undefined"
                        :provider="result.provider"
                        :providers="result.providers"
                        :environment="
                            ['mod', 'modpack'].includes(resultProjectType)
                                ? {
                                      clientSide:
                                          result.client_side as Labrinth.Projects.v2.Environment,
                                      serverSide:
                                          result.server_side as Labrinth.Projects.v2.Environment,
                                  }
                                : undefined
                        "
                        :layout="ctx.effectiveLayout.value"
                        @contextmenu.prevent.stop="
                            (event: MouseEvent) => ctx.onContextMenu?.(event, result)
                        "
                        @mouseenter="ctx.onProjectHover?.(result)"
                        @mouseleave="ctx.onProjectHoverEnd?.()"
                    >
                        <template
                            v-if="ctx.getCardActions?.(result, resultProjectType)?.length"
                            #actions
                        >
                            <div class="flex gap-2">
                                <Button
                                    v-for="action in ctx.getCardActions(result, resultProjectType)"
                                    :key="action.key"
                                    v-tooltip="action.tooltip"
                                    :color="action.color"
                                    :type="
                                        action.type === 'transparent'
                                            ? 'quiet'
                                            : action.type === 'outlined'
                                              ? 'outlined'
                                              : action.color
                                                ? 'colored'
                                                : 'base'
                                    "
                                    :size="effectiveLayout === 'compact' ? '2xs' : 'md'"
                                    :circular="action.circular"
                                    :icon-only="action.circular"
                                    :label="action.circular ? action.label : undefined"
                                    :disabled="action.disabled || ctx.loading.value"
                                    @click.stop="action.onClick"
                                >
                                    <component :is="action.icon" :class="action.iconClass" />
                                    <template v-if="!action.circular">{{
                                        effectiveLayout === 'compact'
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

        <div :class="ctx.variant === 'web' ? 'pagination-after mt-3' : 'flex justify-end mt-3'">
            <Pagination
                :page="resultContext?.page ?? ctx.currentPage.value"
                :count="ctx.pageCount.value"
                :count-known="ctx.pageCountKnown?.value ?? true"
                :loading="ctx.loading.value"
                :class="ctx.variant === 'web' ? 'justify-end' : 'pagination-after'"
                @switch-page="ctx.setPage"
            />
        </div>
    </div>

    <slot name="after" />
</template>
