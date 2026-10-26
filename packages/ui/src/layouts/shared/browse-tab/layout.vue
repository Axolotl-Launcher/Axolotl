<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { ArrowUpDownIcon, EyeIcon, SearchIcon } from '@modrinth/assets'
import { computed, ref, toValue } from 'vue'

import Button from '#ui/components/base/buttons/Button.vue'
import Combobox, { type ComboboxOption } from '#ui/components/base/Combobox.vue'
import EmptyState from '#ui/components/base/EmptyState.vue'
import NavTabs from '#ui/components/base/NavTabs.vue'
import Pagination from '#ui/components/base/Pagination.vue'
import PopoutMenu from '#ui/components/base/PopoutMenu.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import ProjectCard from '#ui/components/project/card/ProjectCard.vue'
import ContentCardReveal from '#ui/components/project/ContentCardReveal.vue'
import ProjectCardList from '#ui/components/project/ProjectCardList.vue'
import ProjectCardSkeleton from '#ui/components/project/ProjectCardSkeleton.vue'
import SearchFilterControl from '#ui/components/search/SearchFilterControl.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useStickyObserver } from '#ui/composables/sticky-observer'
import { commonMessages, formatProjectTypeSentence } from '#ui/utils/common-messages'
import type { SortType } from '#ui/utils/search'

import SelectedProjectsFloatingBar from './components/SelectedProjectsFloatingBar.vue'
import { projectCardDates } from './composables/project-dates'
import BrowseInstallHeader from './header.vue'
import { injectBrowseManager } from './providers/browse-manager'

const ctx = injectBrowseManager()
const { formatMessage } = useVIntl()
const lockedMessages = computed(() => toValue(ctx.lockedFilterMessages))
const stickyChromeRef = ref<HTMLElement | null>(null)
const { isStuck: isChromeStuck } = useStickyObserver(stickyChromeRef, 'BrowseToolbarChrome')
const showInstallHeader = computed(
    () =>
        !!ctx.installContext?.value &&
        ctx.installContext.value.showInstallHeader !== false &&
        ctx.variant !== 'web',
)

const maxResultsValues = computed<number[]>(
    () => ctx.maxResultsOptions?.value ?? [5, 10, 15, 20, 50, 100],
)

const messages = defineMessages({
    searchPlaceholder: {
        id: 'browse.search.placeholder',
        defaultMessage: 'Search {projectType}...',
    },
    viewPrefix: {
        id: 'browse.view-prefix',
        defaultMessage: 'View:',
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
    loadingLabel: {
        id: 'browse.loading-results',
        defaultMessage: 'Loading results…',
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

const maxResultsOptions = computed<ComboboxOption<number>[]>(() =>
    maxResultsValues.value.map((value) => ({
        value,
        label: String(value),
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

const selectedDisplayMode = computed(() =>
    ctx.displayModeOptions?.value.find((option) => option.id === ctx.displayMode?.value),
)

const resultContext = computed(() => ctx.committed?.value)
const resultProjectType = computed(() => resultContext.value?.projectType ?? ctx.projectType.value)
const resultIsServer = computed(() => resultProjectType.value === 'server')
const initialLoading = computed(() => ctx.loading.value && !resultContext.value)

const skeletonCount = computed(() => {
    const layout = ctx.effectiveLayout.value
    if (layout === 'grid' || layout === 'gallery') return 6
    // Match the visible list density without stacking a full viewport of rows.
    return Math.min(ctx.maxResults?.value ?? 8, 8)
})
</script>

<template>
    <div
        ref="stickyChromeRef"
        class="browse-toolbar-chrome sticky top-0 z-20 flex flex-col gap-3 border-b border-solid border-surface-5"
        :class="[
            isChromeStuck && ctx.variant !== 'web'
                ? 'border-t shadow-[0_8px_16px_-12px_rgba(0,0,0,0.35)]'
                : '',
            ctx.variant === 'web'
                ? 'mb-3 rounded-xl bg-surface-1 p-3'
                : '-mx-6 -mt-6 border-0 bg-[color-mix(in_srgb,var(--surface-1-opaque,var(--surface-1))_20%,transparent)] backdrop-blur-xl',
        ]"
    >
        <div v-if="showInstallHeader" class="p-3 pb-0">
            <BrowseInstallHeader />
        </div>
        <div class="flex flex-col gap-3" :class="ctx.variant === 'web' ? '' : 'p-3'">
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
                    :disabled="ctx.filtersReady?.value === false"
                    :icon="SearchIcon"
                    type="text"
                    autocomplete="off"
                    :placeholder="
                        formatMessage(messages.searchPlaceholder, {
                            projectType: formatProjectTypeSentence(
                                formatMessage,
                                ctx.projectType.value,
                                2,
                            ),
                        })
                    "
                    clearable
                    wrapper-class="min-w-0 flex-[1_1_16rem]"
                    :input-class="ctx.variant === 'web' ? '!h-12' : 'h-12'"
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
                    :disabled="ctx.filtersReady?.value === false"
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
                    v-tooltip="{ content: formatMessage(messages.viewPrefix), triggers: ['hover'] }"
                    :disabled="ctx.filtersReady?.value === false"
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

                <div v-if="ctx.filtersMenuOpen && !ctx.filtersMenuOpen.value" class="lg:hidden">
                    <Button @click="ctx.filtersMenuOpen.value = true"
                        >{{ formatMessage(messages.filterResults) }}
                    </Button>
                </div>

                <PopoutMenu
                    v-if="
                        ctx.displayMode &&
                        ctx.displayModeOptions?.value.length &&
                        ctx.setDisplayMode
                    "
                    :tooltip="ctx.displayModeTooltip?.value"
                    placement="bottom-end"
                >
                    <Button circular icon-only :aria-label="ctx.displayModeTooltip?.value"
                        ><component :is="selectedDisplayMode?.icon" />
                    </Button>
                    <template #menu>
                        <div class="flex w-44 flex-col gap-1 p-1">
                            <Button
                                v-for="option in ctx.displayModeOptions.value"
                                :key="option.id"
                                :type="ctx.displayMode.value === option.id ? 'base' : 'quiet'"
                                class="flex w-full items-center gap-2 !justify-start text-left"
                                :aria-pressed="ctx.displayMode.value === option.id"
                                @click="ctx.setDisplayMode!(option.id)"
                            >
                                <component :is="option.icon" class="h-4 w-4" />
                                <span>{{ option.label }}</span>
                            </Button>
                        </div>
                    </template>
                </PopoutMenu>

                <Pagination
                    :page="resultContext?.page ?? ctx.currentPage.value"
                    :count="ctx.pageCount.value"
                    :loading="ctx.loading.value"
                    :class="ctx.variant === 'web' ? 'mx-auto sm:ml-auto sm:mr-0' : 'ml-auto'"
                    @switch-page="ctx.setPage"
                />
            </div>

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
        </div>
    </div>

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
    <p
        v-if="ctx.loading.value && !initialLoading"
        role="status"
        class="my-2 text-sm text-[var(--color-text-tertiary)]"
    >
        {{ formatMessage(messages.loadingLabel) }}
    </p>
    <div class="search relative" :aria-busy="ctx.loading.value">
        <section v-if="initialLoading" class="py-1" aria-busy="true" aria-live="polite">
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
                                    :size="ctx.effectiveLayout.value === 'compact' ? '2xs' : 'md'"
                                    :circular="action.circular"
                                    :icon-only="action.circular"
                                    :label="action.circular ? action.label : undefined"
                                    :disabled="action.disabled || ctx.loading.value"
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
                                    :size="ctx.effectiveLayout.value === 'compact' ? '2xs' : 'md'"
                                    :circular="action.circular"
                                    :icon-only="action.circular"
                                    :label="action.circular ? action.label : undefined"
                                    :disabled="action.disabled || ctx.loading.value"
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

        <div :class="ctx.variant === 'web' ? 'pagination-after mt-3' : 'flex justify-end mt-3'">
            <Pagination
                :page="resultContext?.page ?? ctx.currentPage.value"
                :count="ctx.pageCount.value"
                :loading="ctx.loading.value"
                :class="ctx.variant === 'web' ? 'justify-end' : 'pagination-after'"
                @switch-page="ctx.setPage"
            />
        </div>
    </div>

    <slot name="after" />
</template>
