/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { invoke } from '@/helpers/invoke'

export interface LoadingBarType {
    type?: string
    stage?: string
    completed?: number
    total?: number
    version?: string
    instance_id?: string
    instance_name?: string
    pack_name?: string
    icon?: string | null
}

export interface LoadingBar {
    id?: string | number
    loading_bar_uuid?: string | number
    title?: string
    message?: string
    current?: number
    total?: number
    bar_type?: LoadingBarType
}

export type OpeningCommandEvent =
    | 'RunMRPack'
    | 'OpenSchematic'
    | 'InstallServer'
    | 'InstallVersion'
    | 'InstallMod'
    | 'InstallModpack'
    | string

export interface OpeningCommand {
    event: OpeningCommandEvent
    id?: string
    path?: string
}

function startupMark(name: string): number | undefined {
    const entry = performance.getEntriesByName(name, 'mark').at(-1)
    return entry ? Math.round(entry.startTime) : undefined
}

export async function initialize_state() {
    const navigation = performance.getEntriesByType('navigation')[0] as
        PerformanceNavigationTiming | undefined

    return await invoke<void>('initialize_state', {
        frontendTiming: {
            navigationStartUnixMs: Math.round(performance.timeOrigin),
            documentResponseStartMs: Math.round(navigation?.responseStart ?? 0) || undefined,
            documentResponseEndMs: Math.round(navigation?.responseEnd ?? 0) || undefined,
            domInteractiveMs: Math.round(navigation?.domInteractive ?? 0) || undefined,
            entryEvaluatedMs: startupMark('axolotl:entry-evaluated'),
            mountStartedMs: startupMark('axolotl:mount-started'),
            mountFinishedMs: startupMark('axolotl:mount-finished'),
            initializeSentMs: Math.round(performance.now()),
        },
    })
}

export async function set_discord_activity(activity: string) {
    return await invoke<void>('set_discord_activity', { activity })
}

// Gets active progress bars
export async function progress_bars_list() {
    return await invoke<Record<string, LoadingBar>>('plugin:utils|progress_bars_list')
}

// Get opening command
// For example, if a user clicks on an .mrpack to open the app.
// This should be called once and only when the app is done booting up and ready to receive a command
// Returns a Command struct- see events.js
export async function get_opening_command() {
    return await invoke<OpeningCommand | null>('plugin:utils|get_opening_command')
}
