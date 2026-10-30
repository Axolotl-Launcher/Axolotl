export type BrowseContentDisplayMode = 'list' | 'compact' | 'grid'
export type BrowseContentProjectType =
    'modpack' | 'mod' | 'resourcepack' | 'datapack' | 'shader' | 'world' | 'schematic'

const BROWSE_CONTENT_DISPLAY_MODE_STORAGE_KEY = 'axolotl-browse-content-display-mode'
const BROWSE_CONTENT_PROJECT_TYPE_STORAGE_KEY = 'axolotl-browse-content-project-type'

export function getLastBrowseContentDisplayMode(): BrowseContentDisplayMode {
    const value = globalThis.localStorage?.getItem(BROWSE_CONTENT_DISPLAY_MODE_STORAGE_KEY)
    return value === 'compact' || value === 'grid' ? value : 'list'
}

export function setLastBrowseContentDisplayMode(mode: BrowseContentDisplayMode) {
    globalThis.localStorage?.setItem(BROWSE_CONTENT_DISPLAY_MODE_STORAGE_KEY, mode)
}

export function isBrowseContentProjectType(value: string): value is BrowseContentProjectType {
    return ['modpack', 'mod', 'resourcepack', 'datapack', 'shader', 'world', 'schematic'].includes(value)
}

export function getLastBrowseContentProjectType(): BrowseContentProjectType {
    const value = globalThis.localStorage?.getItem(BROWSE_CONTENT_PROJECT_TYPE_STORAGE_KEY)
    return value && isBrowseContentProjectType(value) ? value : 'modpack'
}

export function setLastBrowseContentProjectType(type: BrowseContentProjectType) {
    globalThis.localStorage?.setItem(BROWSE_CONTENT_PROJECT_TYPE_STORAGE_KEY, type)
}
