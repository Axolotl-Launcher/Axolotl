/**
 * One swamp hut the search found, in world coordinates.
 *
 * `y` is the average height of the hut's footprint, which is what the search
 * program reports. The whole point of the tool is finding the hut that sits
 * lowest for a given seed, so compare huts rather than treating `y` as a block
 * coordinate.
 */
export interface SwampHutHit {
    x: number
    z: number
    y: number
    /** Distance from world origin, for sorting by "closest to spawn". */
    distance: number
}

/** What one search run reports back. */
export interface SwampHutScan {
    seed: string
    /** Regions scanned, as the CLI counts them. */
    regions: number
    hits: SwampHutHit[]
    /** Wall-clock milliseconds the scan took. */
    elapsedMs: number
    /** Lowest Y among the hits, which is what the tool is named for. */
    lowestY: number | null
}
