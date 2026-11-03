import { expect, it } from 'vitest'
import { computed, nextTick, ref } from 'vue'

import type { ContentCardTableItem, ContentItem } from '../types'
import { useContentFolderGroups } from './use-content-folder-groups'

function item(id: string, fileName: string): ContentItem {
    return {
        id,
        file_name: fileName,
        project_type: 'mod',
        update: null,
        origin_provider: null,
    }
}

function setup(initialExpanded: string[] = [], initialItems: ContentItem[] = defaultItems()) {
    const items = ref(initialItems)
    const expandedGroups = ref(new Set(initialExpanded))
    const groups = useContentFolderGroups({
        filteredItems: computed(() => items.value),
        modpackChildIdSet: computed(() => new Set<string>()),
        searchQuery: ref(''),
        expandedGroups,
        getItemId: (entry) => entry.id,
        mapToTableItem: (entry, group) =>
            ({
                id: entry.id,
                project: {
                    id: entry.id,
                    slug: null,
                    title: entry.file_name,
                    icon_url: null,
                },
                enabled: true,
                group,
            }) as ContentCardTableItem,
        isGroupedItem: (entry) => entry.file_name.includes('/'),
        treePath: (entry) => entry.file_name,
        folderGroupId: (path) => `folder:${path}`,
        folderGroupIdPrefix: 'folder:',
        locale: ref('en-US'),
    })
    return { groups, expandedGroups, items }
}

function defaultItems() {
    return [item('one', '辅助/one.jar'), item('two', '辅助/two.jar'), item('root', 'root.jar')]
}

it('keeps newly discovered folders collapsed', async () => {
    const { groups, expandedGroups, items } = setup([], [])

    await nextTick()
    items.value = [
        item('one', '辅助/one.jar'),
        item('two', '辅助/two.jar'),
        item('root', 'root.jar'),
    ]
    await nextTick()

    expect(expandedGroups.value.size).toBe(0)
    expect(
        groups.folderRows.value.map((row) => ({ id: row.id, header: row.isGroupHeader })),
    ).toEqual([{ id: 'folder:辅助', header: true }])
})

it('still renders folders the user explicitly expanded', async () => {
    const { groups } = setup(['folder:辅助'])

    await nextTick()

    expect(
        groups.folderRows.value.map((row) => ({ id: row.id, header: row.isGroupHeader })),
    ).toEqual([
        { id: 'folder:辅助', header: true },
        { id: 'one', header: undefined },
        { id: 'two', header: undefined },
    ])
})
