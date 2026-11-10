const FAVORITES_CATEGORY_FILTER_OPEN_STORAGE_KEY = 'axolotl-favorites-category-filter-open'

/**
 * Whether the favorites category filter starts expanded. The panel is open for
 * a first-time user; after that, the last toggle the user chose wins.
 */
export function getFavoriteCategoryFilterOpen(): boolean {
    const value = globalThis.localStorage?.getItem(FAVORITES_CATEGORY_FILTER_OPEN_STORAGE_KEY)
    return value == null ? true : value === 'true'
}

export function setFavoriteCategoryFilterOpen(open: boolean) {
    globalThis.localStorage?.setItem(FAVORITES_CATEGORY_FILTER_OPEN_STORAGE_KEY, String(open))
}
