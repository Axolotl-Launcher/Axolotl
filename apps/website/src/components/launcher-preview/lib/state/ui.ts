import { type Component, computed, markRaw, ref, shallowRef } from 'vue'

export type LibraryView = 'grid' | 'list'

/**
 * An app-level overlay (the onboarding tour, the restart splash, …) that the
 * app renders from App.vue. The shell renders these inside the window, so they
 * keep running across navigation; `close` from the component removes it.
 */
export interface AppOverlay {
    key: string
    component: Component
    props?: Record<string, unknown>
}
export type DiscoverView = 'list' | 'grid' | 'table'

/**
 * Presentation state shared by screens: view modes, filters and the lab
 * favorites, mirroring the app's local UI preferences.
 */
export function createUiState() {
    const libraryView = ref<LibraryView>('grid')
    const librarySort = ref<'name' | 'last_played' | 'created' | 'time_played'>('last_played')
    const librarySortAscending = ref(false)
    const libraryGroupMode = ref<'none' | 'custom' | 'loader' | 'game_version'>('custom')
    const libraryCollapsed = ref<string[]>([])
    const libraryQuery = ref('')
    const discoverView = ref<DiscoverView>('list')
    const installTarget = ref<string | null>(null)
    const labFavorites = ref<string[]>([])
    const dismissedHints = ref<string[]>([])
    const appOverlays = shallowRef<AppOverlay[]>([])

    function showAppOverlay(overlay: AppOverlay) {
        appOverlays.value = [
            ...appOverlays.value.filter((entry) => entry.key !== overlay.key),
            { ...overlay, component: markRaw(overlay.component) },
        ]
    }

    function hideAppOverlay(key: string) {
        appOverlays.value = appOverlays.value.filter((entry) => entry.key !== key)
    }

    function toggleCollapsed(group: string) {
        libraryCollapsed.value = libraryCollapsed.value.includes(group)
            ? libraryCollapsed.value.filter((entry) => entry !== group)
            : [...libraryCollapsed.value, group]
    }

    function toggleLabFavorite(id: string) {
        labFavorites.value = labFavorites.value.includes(id)
            ? labFavorites.value.filter((entry) => entry !== id)
            : [...labFavorites.value, id]
    }

    const collapsedSet = computed(() => new Set(libraryCollapsed.value))

    return {
        libraryView,
        librarySort,
        librarySortAscending,
        libraryGroupMode,
        libraryCollapsed,
        collapsedSet,
        libraryQuery,
        discoverView,
        installTarget,
        labFavorites,
        dismissedHints,
        appOverlays,
        showAppOverlay,
        hideAppOverlay,
        toggleCollapsed,
        toggleLabFavorite,
    }
}

export type UiState = ReturnType<typeof createUiState>
