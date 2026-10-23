import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import { computed, type ComputedRef, type Ref, ref } from 'vue'
import { useRoute } from 'vue-router'

import type { PreviewSettings } from './state/settings'

export interface PreviewShellContext {
    /** Whether the 300px right sidebar is open (App.vue `sidebarVisible`). */
    sidebarVisible: ComputedRef<boolean>
    /** Routes that always show the sidebar (App.vue `forceSidebar`). */
    forceSidebar: ComputedRef<boolean>
    /** Routes that never show the sidebar (App.vue `forceSidebarHidden`). */
    forceSidebarHidden: ComputedRef<boolean>
    sidebarToggled: Ref<boolean>
    toggleSidebar: () => void
}

export const [injectPreviewShell, providePreviewShell] = createContext<PreviewShellContext>(
    'PreviewShell',
    'launcher-preview-shell',
)

export function usePreviewShell(): PreviewShellContext {
    return injectPreviewShell()
}

/** The sidebar rules of the app shell (apps/app-frontend/src/App.vue). */
export function createPreviewShell(settings: PreviewSettings): PreviewShellContext {
    const route = useRoute()
    const sidebarToggled = ref(settings.sidebar_expanded)
    const forceSidebar = computed(
        () => route.path.startsWith('/browse') || route.path.startsWith('/project'),
    )
    const forceSidebarHidden = computed(() => route.path === '/settings')
    const sidebarVisible = computed(
        () => !forceSidebarHidden.value && (sidebarToggled.value || forceSidebar.value),
    )

    function toggleSidebar() {
        sidebarToggled.value = !sidebarToggled.value
        settings.sidebar_expanded = sidebarToggled.value
    }

    return { sidebarVisible, forceSidebar, forceSidebarHidden, sidebarToggled, toggleSidebar }
}
