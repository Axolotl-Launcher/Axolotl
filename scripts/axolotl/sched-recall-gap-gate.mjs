// Size gate for the recall gap C \ G (spec §11.6 N13, checklist K0.27). The
// total order only constrains registered resources, so a large gap, or any
// unregistered quota/permit or throttle (S5/S9) resource, switches the design
// to per-family orders ("回退分族"). The fallback is automatic (spec §11.8 R5).
//
// Exit 0: within the gate. Exit 1: fallback triggered. Exit 2: unreadable input.
// Usage: node scripts/axolotl/sched-recall-gap-gate.mjs [--dir <inventory dir>] [--ratio 0.1]

import { defaultOutDir, isMain, outputPaths, parseArgs, readJson } from './sched-utils.mjs'

export const DEFAULT_RATIO = 0.1

export function evaluateGap(inventory, recallGap, ratio = DEFAULT_RATIO) {
    const limit = ratio * inventory.items.length
    const absolute = recallGap.items.filter((gap) => gap.classHint === 'S5/S9')
    const oversized = recallGap.items.length > limit
    return { size: recallGap.items.length, limit, absolute, fallback: oversized || absolute.length > 0, oversized }
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const paths = outputPaths(args.dir ?? defaultOutDir)
    let result
    try {
        result = evaluateGap(readJson(paths.inventory), readJson(paths.recallGap), args.ratio ? Number(args.ratio) : DEFAULT_RATIO)
    } catch (error) {
        console.error(`sched-recall-gap-gate: ${error.message}`)
        process.exit(2)
    }
    console.log(`|C \\ G| = ${result.size}, N = ${args.ratio ?? DEFAULT_RATIO} x |G| = ${result.limit}`)
    for (const gap of result.absolute) console.log(`S5/S9 gap item: ${gap.key} (${gap.detail ?? gap.reason}) at ${gap.sites.join(', ')}`)
    if (result.fallback) {
        console.log(`fallback to per-family orders triggered: ${[result.oversized && 'gap exceeds N', result.absolute.length > 0 && 'gap holds S5/S9 items'].filter(Boolean).join('; ')}`)
        process.exit(1)
    }
    console.log('gap within the gate; the single total order stands')
}
