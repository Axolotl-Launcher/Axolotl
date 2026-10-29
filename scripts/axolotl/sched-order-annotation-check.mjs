// Total-order check (spec §6.3, design v8 §4.9): `// sched-order:` annotations
// must name registered idents, the rank table must match the reviewed
// fingerprint, and acquisitions must ascend in rank. A structural violation
// (V_struct > 0) found here or recorded by the coarse screen in the policy
// switches the design to per-family orders automatically (spec §11.8 R5).
//
// Exit 0: total order holds. Exit 1: fallback triggered. Exit 2: annotation,
// coverage (with --require-annotations) or fingerprint error (never fixed by
// regenerating ranks, spec §11.8 R2).
// Usage: node scripts/axolotl/sched-order-annotation-check.mjs [--dir <inventory dir>] [--require-annotations]

import {
    checkAnnotations,
    defaultOutDir,
    isMain,
    loadPolicy,
    outputPaths,
    parseArgs,
    rankFingerprint,
    readJson,
    scanOrderAnnotations,
    sourcesOf,
    workspaceCrates,
} from './sched-utils.mjs'

export function evaluateOrder(registry, policy, annotations) {
    const errors = []
    const { unreachable, descending } = checkAnnotations(annotations, registry)
    for (const annotation of unreachable) errors.push(`${annotation.file}:${annotation.line} names unregistered ${annotation.ident}`)
    if (rankFingerprint(registry.items) !== policy.rankFingerprint) errors.push('rank table differs from the reviewed fingerprint')
    const ranks = new Map(registry.items.map((item) => [item.ident, item.rank]))
    const recorded = policy.orderViolations.filter((violation) => (ranks.get(violation.acquired) ?? 0) <= (ranks.get(violation.held) ?? 0))
    const stale = policy.orderViolations.filter((violation) => !recorded.includes(violation))
    return { errors, descending, recorded, stale }
}

/**
 * Acquisition points of registered resources with no `sched-order` annotation
 * on their line or the line above. The order is only judged where annotated,
 * so this is the part of the code the gate cannot see yet.
 */
export function unannotatedGetpoints(getpoints, registry, annotations) {
    const registered = new Set(registry.items.map((item) => item.ident))
    const annotated = new Set(annotations.flatMap((annotation) => [`${annotation.file}:${annotation.line}`, `${annotation.file}:${annotation.line + 1}`]))
    return getpoints.items.filter((point) => point.idents.some((ident) => registered.has(ident)) && !annotated.has(`${point.file}:${point.line}`))
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const paths = outputPaths(args.dir ?? defaultOutDir)
    const registry = readJson(paths.registry)
    const annotations = scanOrderAnnotations(sourcesOf(workspaceCrates()))
    const result = evaluateOrder(registry, loadPolicy(), annotations)
    const unannotated = unannotatedGetpoints(readJson(paths.getpoints), registry, annotations)
    console.log(`annotations: ${annotations.length}; acquisition points without one: ${unannotated.length}`)
    // Annotations arrive with the total order (scheduler phase 4); from then on
    // CI passes --require-annotations so an unannotated acquisition fails.
    if (args['require-annotations']) {
        for (const point of unannotated) result.errors.push(`${point.file}:${point.line} acquires ${point.idents.join(' | ')} without a sched-order annotation`)
    }
    for (const error of result.errors) console.error(error)
    if (result.errors.length > 0) process.exit(2)
    for (const { held, acquired } of result.descending) console.log(`V_struct: ${acquired.file}:${acquired.line} acquires ${acquired.ident} after ${held.ident}`)
    for (const violation of result.recorded) console.log(`V_struct (coarse screen): ${violation.site} acquires ${violation.acquired} while holding ${violation.held}`)
    for (const violation of result.stale) console.log(`no longer descending under the current ranks: ${violation.site}`)
    if (result.descending.length + result.recorded.length > 0) {
        console.log('fallback to per-family orders triggered (spec §6.3.4)')
        process.exit(1)
    }
    console.log('total order holds for every annotated and recorded path')
}
