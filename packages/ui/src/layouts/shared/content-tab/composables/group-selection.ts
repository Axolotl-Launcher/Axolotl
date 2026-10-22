import type { Ref } from 'vue'
import { computed } from 'vue'

import type { ContentCardTableItem } from '../types'

export interface UseGroupSelectionOptions {
    items: Ref<ContentCardTableItem[]>
    selectedIds: Ref<string[]>
}

export interface GroupCheckboxState {
    checked: boolean
    indeterminate: boolean
}

export function useGroupSelection(options: UseGroupSelectionOptions) {
    const { items, selectedIds } = options
    const selectionIds = (item: ContentCardTableItem) =>
        item.isGroupHeader ? (item.groupChildIds ?? []) : [item.id]
    const allIds = computed(() => [...new Set(items.value.flatMap(selectionIds))])

    const allSelected = computed(() => {
        return allIds.value.length > 0 && allIds.value.every((id) => selectedIds.value.includes(id))
    })

    const someSelected = computed(() => {
        return allIds.value.some((id) => selectedIds.value.includes(id)) && !allSelected.value
    })

    function getGroupCheckboxState(item: ContentCardTableItem): GroupCheckboxState {
        if (!item.isGroupHeader || !item.groupChildIds) {
            return { checked: false, indeterminate: false }
        }
        if (item.groupChildIds.length === 0) {
            return { checked: false, indeterminate: false }
        }
        const selectedCount = item.groupChildIds.filter((id) =>
            selectedIds.value.includes(id),
        ).length
        if (selectedCount === item.groupChildIds.length) {
            return { checked: true, indeterminate: false }
        }
        if (selectedCount > 0) {
            return { checked: false, indeterminate: true }
        }
        return { checked: false, indeterminate: false }
    }

    function isItemSelected(itemId: string): boolean {
        return selectedIds.value.includes(itemId)
    }

    function toggleSelectAll() {
        if (allSelected.value || someSelected.value) {
            selectedIds.value = []
        } else {
            selectedIds.value = allIds.value
        }
    }

    function toggleItemSelection(
        itemId: string,
        selected: boolean,
        lastSelectedIndex: Ref<number | null>,
        index?: number,
        event?: MouseEvent,
        item?: ContentCardTableItem,
    ) {
        if (
            selected &&
            event?.shiftKey &&
            lastSelectedIndex.value !== null &&
            index !== undefined
        ) {
            const start = Math.min(lastSelectedIndex.value, index)
            const end = Math.max(lastSelectedIndex.value, index)
            const rangeIds = items.value.slice(start, end + 1).flatMap(selectionIds)
            const merged = new Set([...selectedIds.value, ...rangeIds])
            selectedIds.value = [...merged]
        } else if (selected) {
            if (!item?.isGroupHeader && !selectedIds.value.includes(itemId)) {
                selectedIds.value = [...selectedIds.value, itemId]
            }
        } else {
            selectedIds.value = selectedIds.value.filter((id) => id !== itemId)
        }

        if (item?.isGroupHeader && item.groupChildIds) {
            if (selected) {
                const merged = new Set([...selectedIds.value, ...item.groupChildIds])
                selectedIds.value = [...merged]
            } else {
                const childIds = new Set(item.groupChildIds)
                selectedIds.value = selectedIds.value.filter((id) => !childIds.has(id))
            }
        }

        if (index !== undefined) {
            lastSelectedIndex.value = index
        }
    }

    return {
        allSelected,
        someSelected,
        getGroupCheckboxState,
        isItemSelected,
        toggleSelectAll,
        toggleItemSelection,
    }
}
