import type { ContentItem } from '../types'

export function isPresentContentItem(item: ContentItem): boolean {
    return (
        item.instanceMaterializationState === null ||
        item.instanceMaterializationState === undefined ||
        item.instanceMaterializationState === 'present'
    )
}

export function isEnabledContentItem(item: ContentItem): boolean {
    return isPresentContentItem(item) && item.enabled === true
}

export function isDisabledContentItem(item: ContentItem): boolean {
    return isPresentContentItem(item) && item.enabled === false
}

export function canToggleContentItem(item: ContentItem): boolean {
    return (
        isPresentContentItem(item) &&
        item.enabled !== undefined &&
        item.instanceCapabilities?.canToggle !== false
    )
}

export interface ContentFilterOption {
    id: string
    label: string
}

export function hasContentUpdate(item: ContentItem): boolean {
    return (
        (item.resourceUpdate !== null && item.resourceUpdate !== undefined) ||
        (item.update !== null && item.update !== undefined)
    )
}
