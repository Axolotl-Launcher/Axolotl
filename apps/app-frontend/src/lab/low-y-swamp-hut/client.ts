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

/** Everything the search program accepts on its `scan` subcommand. */
export interface SwampHutScanArgs {
    seed: string
    regions: number
    threads: number
    /** `26.2` | `1.21` | `1.20.1` | `1.19.2` | `1.18.2`; omitted means the default. */
    version?: string
    /** `NORMAL` | `LARGE_BIOMES` | `SINGLE_BIOME`. */
    preset?: string
    /** Phase-1 max height; the program clamps -50/-54 to -50 internally. */
    maxY?: number
    /** Final Y threshold for phase 2; defaults to `maxY` on the program side. */
    phase2MaxY?: number
}

export async function scanLowYSwampHut(args: SwampHutScanArgs): Promise<SwampHutScan> {
    return invoke<SwampHutScan>(`plugin:${PLUGIN}|low_y_swamp_hut_scan`, {
        seed: args.seed,
        regions: args.regions,
        threads: args.threads,
        version: args.version,
        preset: args.preset,
        maxY: args.maxY,
        phase2MaxY: args.phase2MaxY,
    })
}

/** Whether the external search program is available on this machine. */
export async function lowYSwampHutAvailable(): Promise<boolean> {
    return invoke<boolean>(`plugin:${PLUGIN}|low_y_swamp_hut_available`)
}
