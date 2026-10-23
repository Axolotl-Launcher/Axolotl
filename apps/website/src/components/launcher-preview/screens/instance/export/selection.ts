import type { ExportCategory, ExportFileSnapshot } from './backend'

export interface ExportSelectionDraft {
    categories: string[]
    included: string[]
    excluded: string[]
}

export function categoryLeaves(category: ExportCategory): ExportCategory[] {
    return category.children.length ? category.children.flatMap(categoryLeaves) : [category]
}

export function defaultCategories(categories: ExportCategory[]): string[] {
    return categories
        .flatMap(categoryLeaves)
        .filter((category) => category.defaultSelected)
        .map((category) => category.id)
}

export function selectedFiles(files: ExportFileSnapshot[], draft: ExportSelectionDraft): string[] {
    const categories = new Set(draft.categories)
    const included = new Set(draft.included)
    const excluded = new Set(draft.excluded)
    return files
        .filter(
            (file) =>
                !excluded.has(file.relativePath) &&
                (included.has(file.relativePath) ||
                    (file.categorySelected && categories.has(file.categoryId))),
        )
        .map((file) => file.relativePath)
}

export function reviseFileSelection(
    files: ExportFileSnapshot[],
    draft: ExportSelectionDraft,
    next: string[],
): ExportSelectionDraft {
    const previous = new Set(selectedFiles(files, draft))
    const selection = new Set(next)
    const included = new Set(draft.included)
    const excluded = new Set(draft.excluded)
    for (const file of files) {
        const path = file.relativePath
        if (selection.has(path) === previous.has(path)) continue
        if (selection.has(path)) {
            included.add(path)
            excluded.delete(path)
        } else {
            excluded.add(path)
            included.delete(path)
        }
    }
    return { categories: draft.categories, included: [...included], excluded: [...excluded] }
}

export function reviseCategorySelection(
    draft: ExportSelectionDraft,
    category: ExportCategory,
    selected: boolean,
): ExportSelectionDraft {
    const categories = new Set(draft.categories)
    for (const leaf of categoryLeaves(category)) {
        if (selected) categories.add(leaf.id)
        else categories.delete(leaf.id)
    }
    return { ...draft, categories: [...categories] }
}
