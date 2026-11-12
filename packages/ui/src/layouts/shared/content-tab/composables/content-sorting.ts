import type { ContentItem } from '../types'

export type ContentSortMode =
    | 'project-name-asc'
    | 'project-name-desc'
    | 'file-name-asc'
    | 'file-name-desc'
    | 'date-added-newest'
    | 'date-added-oldest'

function fileNameSortKey(fileName: string): string {
    return fileName.replace(/\.disabled$/i, '')
}

type CompareText = (left: string, right: string) => number

function compareByFileName(
    left: ContentItem,
    right: ContentItem,
    compareText: CompareText,
    getStableId: (item: ContentItem) => string = (item) => item.id,
): number {
    return (
        compareText(fileNameSortKey(left.file_name), fileNameSortKey(right.file_name)) ||
        compareText(left.file_name, right.file_name) ||
        compareText(left.project?.title ?? '', right.project?.title ?? '') ||
        compareText(getStableId(left), getStableId(right))
    )
}

function compareByProjectName(
    left: ContentItem,
    right: ContentItem,
    compareText: CompareText,
    getStableId?: (item: ContentItem) => string,
): number {
    return (
        compareText(
            left.project?.title ?? left.file_name,
            right.project?.title ?? right.file_name,
        ) || compareByFileName(left, right, compareText, getStableId)
    )
}

export function sortContentItems(
    items: ContentItem[],
    mode: ContentSortMode,
    locale?: string,
    getStableId?: (item: ContentItem) => string,
): ContentItem[] {
    const sorted = [...items]
    const compareText = new Intl.Collator(locale, {
        numeric: true,
        sensitivity: 'base',
    }).compare

    return sorted.sort((left, right) => {
        switch (mode) {
            case 'project-name-desc':
                return -compareByProjectName(left, right, compareText, getStableId)
            case 'file-name-asc':
                return compareByFileName(left, right, compareText, getStableId)
            case 'file-name-desc':
                return -compareByFileName(left, right, compareText, getStableId)
            case 'date-added-newest':
                return (
                    (right.date_added ?? '').localeCompare(left.date_added ?? '') ||
                    compareByFileName(left, right, compareText, getStableId)
                )
            case 'date-added-oldest':
                return (
                    (left.date_added ?? '').localeCompare(right.date_added ?? '') ||
                    compareByFileName(left, right, compareText, getStableId)
                )
            case 'project-name-asc':
            default:
                return compareByProjectName(left, right, compareText, getStableId)
        }
    })
}
