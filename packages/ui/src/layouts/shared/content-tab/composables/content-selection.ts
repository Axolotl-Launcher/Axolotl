import type { Ref } from 'vue'
import { computed, ref, watch } from 'vue'

import type { ContentItem } from '../types'

export function useContentSelection(
    items: Ref<ContentItem[]>,
    getItemId: (item: ContentItem) => string,
    visibleItems?: Ref<ContentItem[]>,
) {
    const selectedIds = ref<string[]>([])

    const selectedItems = computed(() => {
        const selectedIdSet = new Set(selectedIds.value)
        const seenIds = new Set<string>()
        const result: ContentItem[] = []

        for (const item of (visibleItems ?? items).value) {
            const id = getItemId(item)
            if (selectedIdSet.has(id) && !seenIds.has(id)) {
                seenIds.add(id)
                result.push(item)
            }
        }

        return result
    })

    watch(
        () => items.value.map(getItemId),
        (newIds) => {
            if (selectedIds.value.length === 0) return
            const validIds = new Set(newIds)
            const pruned = selectedIds.value.filter((id) => validIds.has(id))
            if (pruned.length !== selectedIds.value.length) {
                selectedIds.value = pruned
            }
        },
    )

    function clearSelection() {
        selectedIds.value = []
    }

    function removeFromSelection(id: string) {
        selectedIds.value = selectedIds.value.filter((i) => i !== id)
    }

    return { selectedIds, selectedItems, clearSelection, removeFromSelection }
}
