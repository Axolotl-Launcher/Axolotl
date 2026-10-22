import type { ContentItem } from '@modrinth/ui'

export function matchesContentItem(
    item: ContentItem,
    target: ContentItem,
    originalFileName: string,
    originalFilePath?: string,
) {
    if (item === target) return true
    if (item.instanceEntryId || target.instanceEntryId) {
        return !!target.instanceEntryId && item.instanceEntryId === target.instanceEntryId
    }
    const path = originalFilePath ?? target.file_path
    if (path)
        return (
            item.file_path === path || (!!target.file_path && item.file_path === target.file_path)
        )
    return !item.file_path && item.file_name === originalFileName
}

export function applyContentItemUpdates(
    items: ContentItem[],
    target: ContentItem,
    originalFileName: string,
    originalFilePath: string | undefined,
    updates: Partial<ContentItem>,
) {
    for (const item of items) {
        if (matchesContentItem(item, target, originalFileName, originalFilePath)) {
            Object.assign(item, updates)
        }
    }
    Object.assign(target, updates)
}
