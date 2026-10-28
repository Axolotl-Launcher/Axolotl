import { invoke } from '@tauri-apps/api/core'

import type { SwampHutScan } from './types'

/**
 * Typed wrapper for the `lab` plugin's swamp-hut search.
 *
 * The search itself is the upstream LowYSwampHut program: it replays Minecraft's
 * terrain generation to find the lowest swamp hut a seed produces, which is far
 * too much work to reimplement here. The launcher only ships the UI and calls the
 * program; when it is missing, the command reports that instead of guessing.
 */

const PLUGIN = 'lab'

export async function scanLowYSwampHut(
    seed: string,
    regions: number,
    threads: number,
): Promise<SwampHutScan> {
    return invoke<SwampHutScan>(`plugin:${PLUGIN}|low_y_swamp_hut_scan`, {
        seed,
        regions,
        threads,
    })
}

/** Whether the external search program is available on this machine. */
export async function lowYSwampHutAvailable(): Promise<boolean> {
    return invoke<boolean>(`plugin:${PLUGIN}|low_y_swamp_hut_available`)
}
