import { invoke } from '@tauri-apps/api/core'

/**
 * Typed wrapper for the `lab` plugin's trial-chamber search.
 *
 * The search itself is the TrialSpawnerFinder program: it assembles trial
 * chambers from Minecraft's structure data and counts the trial spawners inside
 * each one, which is far too much work to reimplement here. The launcher ships
 * the UI and runs the program; it needs a Java 25+ runtime, which most 26.x
 * players already have.
 */

const PLUGIN = 'lab'

/** One trial chamber the search accepted. */
export interface TrialChamberHit {
    rank: number
    centerX: number
    centerZ: number
    chamberCount: number
    spawnerCount: number
    /** Chamber block coordinates, `[x, z]` pairs. */
    positions: [number, number][]
}

export interface TrialChamberScan {
    seed: string
    radius: number
    hits: TrialChamberHit[]
    elapsedMs: number
    /** Highest spawner count among the hits. */
    bestSpawners: number | null
}

export async function findTrialChambers(
    seed: string,
    radius: number,
    threads: number,
): Promise<TrialChamberScan> {
    return invoke<TrialChamberScan>(`plugin:${PLUGIN}|trial_chamber_find`, {
        seed,
        radius,
        threads,
    })
}

/**
 * Whether a suitable Java runtime is installed. The command only answers for the
 * runtime, not for the program itself; a missing program surfaces when scanning.
 */
export async function trialChamberAvailable(): Promise<boolean> {
    return invoke<boolean>(`plugin:${PLUGIN}|trial_chamber_available`)
}
