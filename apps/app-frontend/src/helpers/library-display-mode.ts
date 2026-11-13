export type LibraryDisplayMode = 'standard' | 'cards'
export type InstanceLibraryDisplayMode = LibraryDisplayMode | 'list'

const LIBRARY_DISPLAY_MODE_STORAGE_KEY = 'axolotl-library-display-mode'
const INSTANCE_LIBRARY_DISPLAY_MODE_STORAGE_KEY = 'axolotl-instance-library-display-mode'

export function getLastLibraryDisplayMode(): LibraryDisplayMode {
    const value = globalThis.localStorage?.getItem(LIBRARY_DISPLAY_MODE_STORAGE_KEY)
    return value === 'cards' ? value : 'standard'
}

export function setLastLibraryDisplayMode(mode: LibraryDisplayMode) {
    globalThis.localStorage?.setItem(LIBRARY_DISPLAY_MODE_STORAGE_KEY, mode)
}

export function getLastInstanceLibraryDisplayMode(): InstanceLibraryDisplayMode {
    const value = globalThis.localStorage?.getItem(INSTANCE_LIBRARY_DISPLAY_MODE_STORAGE_KEY)
    if (value === 'list' || value === 'cards' || value === 'standard') return value
    // The instance library used to share the server's key. Keep the user's
    // last choice until the library writes its own preference.
    return globalThis.localStorage?.getItem(LIBRARY_DISPLAY_MODE_STORAGE_KEY) === 'cards'
        ? 'cards'
        : 'standard'
}

export function setLastInstanceLibraryDisplayMode(mode: InstanceLibraryDisplayMode) {
    globalThis.localStorage?.setItem(INSTANCE_LIBRARY_DISPLAY_MODE_STORAGE_KEY, mode)
}
