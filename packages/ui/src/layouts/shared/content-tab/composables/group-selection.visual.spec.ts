import { expect, it } from 'vitest'
import { ref } from 'vue'

import type { ContentCardTableItem } from '../types'
import { useGroupSelection } from './group-selection'

it('selects collapsed group children without synthetic IDs and tracks partial selection', () => {
    const group = {
        id: 'duplicate:verified',
        isGroupHeader: true,
        groupChildIds: ['a', 'b'],
    } as ContentCardTableItem
    const items = ref([group, { id: 'c' } as ContentCardTableItem])
    const selectedIds = ref<string[]>([])
    const selection = useGroupSelection({ items, selectedIds })
    const last = ref<number | null>(null)
    selection.toggleItemSelection(group.id, true, last, 0, undefined, group)
    expect(selectedIds.value).toEqual(['a', 'b'])
    expect(selection.someSelected.value).toBe(true)
    expect(selection.getGroupCheckboxState(group)).toEqual({ checked: true, indeterminate: false })
    selectedIds.value = ['a']
    expect(selection.getGroupCheckboxState(group)).toEqual({ checked: false, indeterminate: true })
    selection.toggleItemSelection(group.id, false, last, 0, undefined, group)
    expect(selectedIds.value).toEqual([])
    selection.toggleSelectAll()
    expect(selectedIds.value).toEqual(['a', 'b', 'c'])
    expect(selection.allSelected.value).toBe(true)
    items.value = [
        group,
        { id: 'a' } as ContentCardTableItem,
        { id: 'b' } as ContentCardTableItem,
        { id: 'c' } as ContentCardTableItem,
    ]
    expect(selection.allSelected.value).toBe(true)
    selection.toggleSelectAll()
    expect(selectedIds.value).toEqual([])
    selection.toggleItemSelection(
        'c',
        true,
        last,
        3,
        { shiftKey: true } as MouseEvent,
        items.value[3],
    )
    expect(selectedIds.value).toEqual(['a', 'b', 'c'])
    items.value = [{ id: 'd' } as ContentCardTableItem]
    selection.toggleSelectAll()
    expect(selectedIds.value).toEqual(['d'])
})
