import { computed, type Ref } from 'vue'

import type { GameInstance } from '../../lib/types'
import {
    defaultSortDirections,
    type GridDisplayState,
    type GridGroupingOption,
    type GridSortDirection,
    type GridSortOption,
    UNGROUPED_GROUP_KEY,
} from './state'

function timeOf(value: string | number | null | undefined) {
    if (value === null || value === undefined || value === '') return Number.NaN
    return typeof value === 'number' ? value : Date.parse(value)
}

/** dayjs(a).diff(dayjs(b)): NaN for missing dates, which `Array.sort` treats as equal. */
function diff(a: string | number | null | undefined, b: string | number | null | undefined) {
    return timeOf(a) - timeOf(b)
}

/**
 * Port of `composables/useGridGrouping.ts`. The persisted state is the
 * library's per-window record instead of localStorage.
 */
export function useGridGrouping(
    state: GridDisplayState,
    instances: Ref<GameInstance[]>,
    options: { formatLoader?: (loader: string) => string } = {},
) {
    const grouping = computed(() => state.group)
    const collapsedSectionKeys = computed(() => new Set(state.collapsedGroups ?? []))

    const getSectionKey = (sectionName: string) => `${state.group}:${sectionName}`

    const isSectionCollapsed = (sectionName: string) =>
        collapsedSectionKeys.value.has(getSectionKey(sectionName))

    const setSectionCollapsed = (sectionName: string, collapsed: boolean) => {
        const sectionKey = getSectionKey(sectionName)
        const collapsedSections = new Set(state.collapsedGroups ?? [])
        if (collapsed) collapsedSections.add(sectionKey)
        else collapsedSections.delete(sectionKey)
        state.collapsedGroups = [...collapsedSections]
    }

    function sortInstances(list: GameInstance[], sortBy: GridSortOption, ascending: boolean) {
        const sorted = [...list]
        switch (sortBy) {
            case 'Name':
                sorted.sort((a, b) => (a.name ?? '').localeCompare(b.name ?? ''))
                break
            case 'Game version':
                sorted.sort((a, b) =>
                    (a.game_version ?? '').localeCompare(b.game_version ?? '', undefined, {
                        numeric: true,
                    }),
                )
                break
            case 'Last played':
                sorted.sort((a, b) => diff(a.last_played ?? 0, b.last_played ?? 0))
                break
            case 'Date created':
                sorted.sort((a, b) => diff(a.created, b.created))
                break
            case 'Date modified':
                sorted.sort((a, b) => diff(a.modified, b.modified))
                break
        }
        if (!ascending) sorted.reverse()
        return sorted
    }

    function groupInstances(list: GameInstance[], group: GridGroupingOption) {
        const instanceMap = new Map<string, GameInstance[]>()
        const formatLoader = options.formatLoader ?? ((loader: string) => loader)

        switch (group) {
            case 'Loader':
                for (const instance of list) {
                    const loader = formatLoader(instance.loader ?? '')
                    if (!instanceMap.has(loader)) instanceMap.set(loader, [])
                    instanceMap.get(loader)!.push(instance)
                }
                break
            case 'Game version':
                for (const instance of list) {
                    const gameVersion = instance.game_version ?? ''
                    if (!instanceMap.has(gameVersion)) instanceMap.set(gameVersion, [])
                    instanceMap.get(gameVersion)!.push(instance)
                }
                break
            case 'Group':
                for (const instance of list) {
                    const groups = instance.groups ?? []
                    const categories = groups.length > 0 ? new Set(groups) : [UNGROUPED_GROUP_KEY]
                    for (const category of categories) {
                        if (!instanceMap.has(category)) instanceMap.set(category, [])
                        instanceMap.get(category)!.push(instance)
                    }
                }
                break
            case 'None':
            default:
                instanceMap.set('None', list)
                break
        }
        return instanceMap
    }

    function sortSections(
        instanceMap: Map<string, GameInstance[]>,
        group: GridGroupingOption,
        sortBy: GridSortOption,
    ) {
        if (group !== 'Group' && group !== 'Game version') return instanceMap

        if (group === 'Group' && sortBy !== 'Name') {
            if (instanceMap.has(UNGROUPED_GROUP_KEY) && instanceMap.size > 1) {
                const ungrouped = instanceMap.get(UNGROUPED_GROUP_KEY)!
                instanceMap.delete(UNGROUPED_GROUP_KEY)
                const next = new Map<string, GameInstance[]>([[UNGROUPED_GROUP_KEY, ungrouped]])
                for (const [key, value] of instanceMap) next.set(key, value)
                return next
            }
            return instanceMap
        }

        const sortedEntries = [...instanceMap.entries()].sort((a, b) => {
            if (group === 'Group') {
                if (a[0] === UNGROUPED_GROUP_KEY && b[0] !== UNGROUPED_GROUP_KEY) return -1
                if (a[0] !== UNGROUPED_GROUP_KEY && b[0] === UNGROUPED_GROUP_KEY) return 1
            }
            return a[0].localeCompare(b[0], undefined, { numeric: true })
        })
        return new Map(sortedEntries)
    }

    const sortDirection = computed<GridSortDirection>(
        () => state.sortDirections[state.sortBy] ?? defaultSortDirections[state.sortBy],
    )
    const isSortAscending = computed(() => sortDirection.value === 'asc')

    const toggleSortDirection = () => {
        const next: GridSortDirection = sortDirection.value === 'asc' ? 'desc' : 'asc'
        state.sortDirections = { ...state.sortDirections, [state.sortBy]: next }
    }

    const filteredResults = computed(() => {
        const { group = 'Group', sortBy = 'Name' } = state
        const sorted = sortInstances(instances.value, sortBy, isSortAscending.value)
        const grouped = groupInstances(sorted, group)
        return sortSections(grouped, group, sortBy)
    })

    return {
        grouping,
        filteredResults,
        isSectionCollapsed,
        setSectionCollapsed,
        sortDirection,
        isSortAscending,
        toggleSortDirection,
    }
}
