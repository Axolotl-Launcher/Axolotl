import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import type { FilterType, FilterValue, SortType, Tags } from '@modrinth/ui/src/utils/search.ts'
import type { Component, ComputedRef, Ref, ShallowRef } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import type { SearchHit } from './search-engine'

/** packages/ui/src/layouts/shared/browse-tab/types.ts `BrowseSelectedProject`. */
export interface BrowseSelectedProject {
    id: string
    name: string
    iconUrl?: string | null
}

/** packages/ui/src/layouts/shared/browse-tab/types.ts `BrowseInstallContext`. */
export interface BrowseInstallContext {
    showInstallHeader?: boolean
    name: string
    loader: string
    gameVersion: string
    iconSrc?: string | null
    iconFrameless?: boolean
    backUrl: string | RouteLocationRaw
    backLabel: string
    heading: string
    warning?: string
    selectedProjects?: BrowseSelectedProject[]
    isInstallingSelected?: boolean
    installProgress?: { completed: number; total: number }
    installButtonLabel?: string
    processingLabel?: string
    clearSelected?: () => void | Promise<void>
    installSelected?: () => boolean | void | Promise<boolean | void>
}

/** packages/ui/src/layouts/shared/browse-tab/types.ts `CardAction`. */
export interface CardAction {
    key: string
    label: string
    compactLabel?: string
    icon: Component
    iconClass?: string
    disabled?: boolean
    color?: 'brand' | 'red' | 'green'
    type?: 'standard' | 'outlined' | 'transparent'
    circular?: boolean
    tooltip?: string
    onClick: () => void | Promise<void>
}

export interface BrowseDisplayModeOption {
    id: 'list' | 'compact' | 'grid'
    label: string
    icon: Component
}

/** The browse page's state shared with the layout and the sidebar (`BrowseManagerContext`). */
export interface BrowseContext {
    tags: Ref<Tags>
    projectType: Ref<string>
    query: Ref<string>
    filters: ComputedRef<FilterType[]>
    currentFilters: Ref<FilterValue[]>
    toggledGroups: Ref<string[]>
    overriddenProvidedFilterTypes: Ref<string[]>
    serverFilterTypes: ComputedRef<FilterType[]>
    serverCurrentFilters: Ref<FilterValue[]>
    serverToggledGroups: Ref<string[]>
    effectiveSortTypes: ComputedRef<readonly SortType[]>
    effectiveCurrentSortType: Ref<SortType>
    loading: Ref<boolean>
    searchError: Ref<string | null>
    projectHits: ShallowRef<SearchHit[]>
    serverHits: ShallowRef<unknown[]>
    totalHits: Ref<number>
    pageCount: ComputedRef<number>
    maxResults: Ref<number>
    currentPage: Ref<number>
    isServerType: ComputedRef<boolean>
    effectiveLayout: ComputedRef<'list' | 'compact' | 'grid'>
    deprioritizedTags: ComputedRef<string[]>
    excludeLoaders: ComputedRef<boolean>
    refreshSearch: () => Promise<void>
    setPage: (page: number) => Promise<void>
    clearSearch: () => void
    onFilterChange: () => void
    getProjectLink: (result: SearchHit) => RouteLocationRaw
    selectableProjectTypes: ComputedRef<
        { label: string; href: string; shown?: boolean; onboardingId?: string }[]
    >
    showProjectTypeTabs: ComputedRef<boolean>
    getCardActions: (result: SearchHit, projectType: string) => CardAction[]
    installContext: ComputedRef<BrowseInstallContext | null>
    providedFilters: ComputedRef<FilterValue[]>
    hideInstalled: Ref<boolean>
    showHideInstalled: ComputedRef<boolean>
    hideInstalledLabel: ComputedRef<string>
    advancedFiltersCollapsed: Ref<boolean>
    displayMode: Ref<'list' | 'compact' | 'grid'>
    displayModeOptions: ComputedRef<BrowseDisplayModeOption[]>
    displayModeTooltip: ComputedRef<string>
    setDisplayMode: (mode: 'list' | 'compact' | 'grid') => void
    onContextMenu: (event: MouseEvent, result: SearchHit) => void
    lockedFilterMessages: ComputedRef<{
        gameVersion?: string
        modLoader?: string
        environment?: string
        syncButton?: string
        providedBy?: string
    }>
    /** Resolves a dependency project id to its snapshot card (the app asks the Labrinth API). */
    findProject: (id: string) => { id: string; title: string; icon_url: string } | null
    searchProjects: (
        query: string,
        projectTypes: string[],
    ) => Promise<{ project_id: string; title: string; icon_url: string; project_type: string }[]>
}

export const [injectBrowseContext, provideBrowseContext] = createContext<BrowseContext>(
    'DiscoverBrowse',
    'launcherPreviewBrowseContext',
)
