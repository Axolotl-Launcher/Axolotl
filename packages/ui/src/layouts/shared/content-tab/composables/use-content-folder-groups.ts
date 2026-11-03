import type { ComputedRef, Ref } from 'vue'
import { computed } from 'vue'

import { buildFileTreeRows, type FileTreeEntry } from '#ui/utils/file-tree'

import type { ContentCardTableItem, ContentItem } from '../types'

export interface ContentFolderGroupPolicy {
    /** Whether an item participates in folder grouping. */
    isGroupedItem: (item: ContentItem) => boolean
    /** Folder-tree relative path used to build groups for a grouped item. */
    treePath: (item: ContentItem) => string
    /** Stable group id for a folder path, kept distinct from other group ids. */
    folderGroupId: (path: string) => string
    /** Prefix(es) used to recognize this policy's group ids in the shared expanded set. */
    folderGroupIdPrefix: string | string[]
}

export interface UseContentFolderGroupsOptions extends ContentFolderGroupPolicy {
    filteredItems: ComputedRef<ContentItem[]>
    modpackChildIdSet: ComputedRef<Set<string>>
    searchQuery: Ref<string>
    expandedGroups: Ref<Set<string>>
    getItemId: (item: ContentItem) => string
    mapToTableItem: (item: ContentItem, group?: string) => ContentCardTableItem
    locale: Ref<string>
}

type GroupedItem = FileTreeEntry & { item: ContentItem }

/**
 * Renders items that share a folder path as collapsible group rows while
 * keeping everything else flat. Folders stay collapsed until the user opens
 * them; expansion state is shared through the caller-provided
 * `expandedGroups` set.
 */
export function useContentFolderGroups(options: UseContentFolderGroupsOptions) {
    const {
        filteredItems,
        modpackChildIdSet,
        searchQuery,
        expandedGroups,
        getItemId,
        mapToTableItem,
        isGroupedItem,
        treePath,
        folderGroupId,
        folderGroupIdPrefix,
        locale,
    } = options

    const isModpackChild = (item: ContentItem) =>
        modpackChildIdSet.value.has(getItemId(item).replace(/\.disabled$/, ''))

    const groupedItems = computed(() =>
        filteredItems.value.filter((item) => !isModpackChild(item) && isGroupedItem(item)),
    )

    /** Filtered items that are not rendered by this composable. */
    const regularItems = computed(() =>
        filteredItems.value.filter((item) => !isModpackChild(item) && !isGroupedItem(item)),
    )

    const groupedEntries = computed<GroupedItem[]>(() =>
        groupedItems.value.map((item) => ({
            item,
            id: getItemId(item),
            relativePath: treePath(item),
            fileName: item.file_name,
        })),
    )

    const groupIdPrefixes = Array.isArray(folderGroupIdPrefix)
        ? folderGroupIdPrefix
        : [folderGroupIdPrefix]

    const expandedFolderPaths = computed(() => {
        const paths = new Set<string>()
        for (const id of expandedGroups.value) {
            const prefix = groupIdPrefixes.find((candidate) => id.startsWith(candidate))
            if (prefix) {
                paths.add(id.slice(prefix.length))
            }
        }
        return paths
    })

    /** Group headers and their children, or flat rows while searching. */
    const folderRows = computed<ContentCardTableItem[]>(() => {
        const entries = groupedEntries.value
        if (entries.length === 0) return []

        if (searchQuery.value.trim()) {
            return entries.map((entry) => mapToTableItem(entry.item))
        }

        const rows: ContentCardTableItem[] = []
        for (const row of buildFileTreeRows(entries, expandedFolderPaths.value, '', locale.value)) {
            if (row.kind === 'folder') {
                const groupId = folderGroupId(row.path)
                rows.push({
                    id: groupId,
                    isGroupHeader: true,
                    group: groupId,
                    groupDepth: row.depth,
                    groupItemCount: row.fileCount,
                    groupChildIds: row.childIds,
                    project: {
                        id: groupId,
                        slug: null,
                        title: row.name,
                        icon_url: null,
                    },
                    enabled: true,
                })
            } else {
                const parentGroup = row.parentPath ? folderGroupId(row.parentPath) : undefined
                rows.push({
                    ...mapToTableItem(row.file.item, parentGroup),
                    ...(row.depth > 1 ? { groupDepth: row.depth } : {}),
                })
            }
        }
        return rows
    })

    return { folderRows, regularItems }
}
