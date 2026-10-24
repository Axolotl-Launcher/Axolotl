import { effectScope, reactive, watch } from 'vue'

import { previewDomain, type PreviewState, usePreview, usePreviewDomain } from '../../../lib/state'

/*
 * Instance settings the preview's shared `GameInstance` does not carry, kept per
 * instance for the life of the window so they survive closing the modal and
 * navigating away (the app reads them from its database).
 */

/** `SyncedOption` of `apps/app-frontend/src/helpers/instance.ts`. */
export type SyncedOption =
    | 'game_options'
    | 'command_history'
    | 'multiplayer_servers'
    | 'creative_hotbars'
    | 'screenshots'
    | 'resource_packs'
    | 'data_packs'

export const SYNCED_OPTIONS: SyncedOption[] = [
    'command_history',
    'multiplayer_servers',
    'creative_hotbars',
    'screenshots',
    'game_options',
    'resource_packs',
    'data_packs',
]

export interface InstanceSettingsExtras {
    custom_env_vars: [string, string][] | null
    synced_options: Record<SyncedOption, boolean>
    launch_preparation_timeout: number | null
    game_dir_override: string | null
    linked_dot_minecraft: string | null
}

function defaultSyncedOptions(): Record<SyncedOption, boolean> {
    return Object.fromEntries(SYNCED_OPTIONS.map((option) => [option, false])) as Record<
        SyncedOption,
        boolean
    >
}

function createSettingsDomain(state: PreviewState) {
    const extras = reactive<Record<string, InstanceSettingsExtras>>({})
    /**
     * The app's global synced-option switches (`sync_feature_settings`), which
     * gate every instance's options. The settings table seeds them all off.
     */
    const globalSyncedOptions = reactive<Record<SyncedOption, boolean>>(defaultSyncedOptions())

    function extrasFor(instanceId: string): InstanceSettingsExtras {
        if (!extras[instanceId]) {
            extras[instanceId] = {
                custom_env_vars: null,
                synced_options: defaultSyncedOptions(),
                launch_preparation_timeout: null,
                game_dir_override: null,
                linked_dot_minecraft: null,
            }
        }
        return extras[instanceId]
    }

    // A deleted instance takes its rows with it (`ON DELETE CASCADE`). The watcher
    // belongs to the window, not to the component that first opened the modal.
    const scope = effectScope(true)
    scope.run(() => {
        watch(
            () => state.instances.instances.value.map((instance) => instance.id),
            (ids) => {
                for (const id of Object.keys(extras))
                    if (!ids.includes(id)) Reflect.deleteProperty(extras, id)
            },
        )
    })

    return { value: { extras, extrasFor, globalSyncedOptions }, dispose: () => scope.stop() }
}

export type InstanceSettingsDomain = ReturnType<typeof createSettingsDomain>['value']

export function useInstanceSettingsState(): InstanceSettingsDomain {
    return usePreviewDomain('instance-settings', createSettingsDomain)
}

export function instanceSettingsState(preview: PreviewState): InstanceSettingsDomain {
    return previewDomain(preview, 'instance-settings', createSettingsDomain)
}

/** Resolves after `ms` on the window's clock, standing in for an `invoke` round trip. */
export function useSimulatedInvoke() {
    const preview = usePreview()
    return function simulate(ms = 180): Promise<void> {
        return new Promise((resolve) => {
            preview.clock.after(ms, resolve)
        })
    }
}

/**
 * `get()` of `helpers/settings.ts`, reduced to the global launch defaults the
 * instance settings fall back to. `custom_window_title_enabled` lives with the
 * Settings page's own state; it is read from there when that page has opened.
 */
export function useGlobalSettings() {
    const preview = usePreview()
    const settingsPage = preview.domains.get('settings')?.value as
        { extras?: { custom_window_title_enabled?: boolean } } | undefined
    return {
        settings: preview.settings,
        customWindowTitleEnabled: settingsPage?.extras?.custom_window_title_enabled ?? false,
    }
}
