import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import { reactive, shallowReactive } from 'vue'

import { createPreviewClock } from '../clock'
import { PreviewNotificationManager, PreviewPopupNotificationManager } from '../notifications'
import { createAccountsState } from './accounts'
import { createBreadcrumbsState } from './breadcrumbs'
import { createDownloadsState } from './downloads'
import { createInstancesState } from './instances'
import { createProcessesState } from './processes'
import { createScreenshotsState } from './screenshots'
import { createServersState } from './servers'
import { createSettingsState } from './settings'
import { createUiState } from './ui'

/**
 * The preview's in-memory launcher state. One instance is created per mounted
 * preview window and disposed with it; reopening the window starts over from
 * the snapshot data.
 */
export function createPreviewState() {
    const clock = createPreviewClock()
    const { settings, theme, dispose: disposeSettings } = createSettingsState()
    const breadcrumbs = createBreadcrumbsState()
    const notifications = new PreviewNotificationManager()
    const popups = new PreviewPopupNotificationManager()
    const ui = createUiState()
    const instances = createInstancesState(clock)
    const accounts = createAccountsState(clock)
    const downloads = createDownloadsState(clock, instances)
    const processes = createProcessesState(clock, instances, {
        playerName: () => accounts.selected.value?.profile.name ?? null,
        mods: (instanceId) =>
            (downloads.contents.value[instanceId] ?? [])
                .filter((item) => item.enabled && !item.removed && item.project_type === 'mod')
                .map((item) => ({
                    id: item.file_name.replace(/[-_]?\d.*$/, '').toLowerCase() || item.name,
                    version: item.version_number ?? '',
                })),
        prepare: () => downloads.ensureContents(),
    })
    const servers = createServersState()
    const screenshots = createScreenshotsState(clock)
    // Reactive, so the shell sees a domain that a screen creates later.
    const domains = shallowReactive(new Map<string, { value: unknown; dispose?: () => void }>())

    function dispose() {
        for (const domain of domains.values()) domain.dispose?.()
        domains.clear()
        clock.dispose()
        disposeSettings()
        notifications.clearAllNotifications()
        popups.clearAllNotifications()
    }

    return {
        clock,
        settings,
        theme,
        breadcrumbs,
        notifications,
        popups,
        ui,
        instances,
        accounts,
        downloads,
        processes,
        servers,
        screenshots,
        domains,
        dispose,
    }
}

export type PreviewState = ReturnType<typeof createPreviewState>

export const [injectPreviewState, providePreviewState] = createContext<PreviewState>(
    'LauncherPreview',
    'launcher-preview-state',
)

let currentState: PreviewState | null = null

export function usePreview(): PreviewState {
    return injectPreviewState()
}

/**
 * The window's state outside a component's setup, for the ported app modules
 * that call their backend from plain functions (the desktop app's `invoke`).
 * There is one launcher window per page.
 */
export function currentPreview(): PreviewState {
    if (!currentState) throw new Error('The launcher preview is not open.')
    return currentState
}

export function setCurrentPreview(state: PreviewState | null) {
    currentState = state
}

export interface PreviewDomain<T> {
    value: T
    dispose?: () => void
}

/**
 * Returns the state of a feature area, creating it on first use.
 *
 * Feature screens keep their state in their own chunk and attach it here, so it
 * survives navigating away and back (as the desktop app's stores do) without
 * growing the preview's entry chunk. The domain is disposed with the window.
 */
export function usePreviewDomain<T>(
    key: string,
    factory: (state: PreviewState) => PreviewDomain<T>,
): T {
    return previewDomain(injectPreviewState(), key, factory)
}

/** `usePreviewDomain` for code that holds the state, such as ported backend modules. */
export function previewDomain<T>(
    state: PreviewState,
    key: string,
    factory: (state: PreviewState) => PreviewDomain<T>,
): T {
    let domain = state.domains.get(key) as PreviewDomain<T> | undefined
    if (!domain) {
        domain = factory(state)
        state.domains.set(key, domain)
    }
    return domain.value
}

export { reactive }
