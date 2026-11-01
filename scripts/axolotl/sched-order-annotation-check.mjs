// Resource-order check (spec §6.3, design v8 §4.9). The recall-gap gate has
// switched the design to per-family orders (spec §6.3.4, accepted as R5), so
// ranks are compared only within one scope family. `// sched-order:`
// annotations must name registered idents, the rank table must match the
// reviewed fingerprint, and while a guard is held each later acquisition of
// its family must ascend in rank, unless the pair is a listed exception and
// the nested pairs stay acyclic.
//
// Exit 0: per-family order holds on every annotated path. Exit 1: an in-family
// descending pair (V_struct) that is neither fixed nor a listed exception.
// Exit 2: annotation, cycle, coverage (with --require-annotations) or
// fingerprint error (never fixed by regenerating ranks, spec §11.8 R2).
// Usage: node scripts/axolotl/sched-order-annotation-check.mjs [--dir <inventory dir>] [--require-annotations]

import path from 'node:path'
import {
    checkAnnotations,
    defaultOutDir,
    isMain,
    loadPolicy,
    orderCoverage,
    outputPaths,
    parseArgs,
    rankFingerprint,
    readJson,
    scanOrderAnnotations,
    sourcesOf,
    workspaceCrates,
    writeJson,
} from './sched-utils.mjs'

export function evaluateOrder(registry, policy, annotations) {
    const errors = []
    const exceptions = policy.orderExceptions ?? []
    const result = checkAnnotations(annotations, registry, exceptions)
    for (const annotation of result.unreachable) errors.push(`${annotation.file}:${annotation.line} names unregistered ${annotation.ident}`)
    for (const cycle of result.cycles) errors.push(`nested acquisitions form a cycle: ${cycle.join(' -> ')}`)
    if (rankFingerprint(registry.items) !== policy.rankFingerprint) errors.push('rank table differs from the reviewed fingerprint')
    const items = new Map(registry.items.map((item) => [item.ident, item]))
    const sameFamily = (violation) => items.get(violation.acquired)?.scopeClass === items.get(violation.held)?.scopeClass
    const recorded = policy.orderViolations.filter((violation) => sameFamily(violation) && (items.get(violation.acquired)?.rank ?? 0) <= (items.get(violation.held)?.rank ?? 0))
    const stale = policy.orderViolations.filter((violation) => !recorded.includes(violation))
    // Exceptions found only by the instrumented run are invisible to this static check.
    const unusedExceptions = exceptions.filter(
        (exception) => !exception.observedBy && !result.exempted.some((edge) => edge.held.ident === exception.held && edge.acquired.ident === exception.acquired),
    )
    return { ...result, errors, recorded, stale, unusedExceptions }
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const dir = args.dir ?? defaultOutDir
    const paths = outputPaths(dir)
    let registry
    let getpoints
    try {
        registry = readJson(paths.registry)
        getpoints = readJson(paths.getpoints)
    } catch (error) {
        // A missing inventory is an error, never the exit-1 fallback.
        console.error(`sched-order-annotation-check: ${error.message}`)
        process.exit(2)
    }
    const sources = sourcesOf(workspaceCrates())
    const annotations = scanOrderAnnotations(sources)
    const result = evaluateOrder(registry, loadPolicy(), annotations)
    const coverage = orderCoverage(getpoints, registry, annotations, sources)
    writeJson(path.join(dir, 'sched-order-coverage.json'), {
        constrained: coverage.constrained.length,
        uncovered: coverage.uncovered,
        exempt: coverage.exempt.map(({ file, line, method, idents, rule }) => ({ file, line, method, idents, rule })),
    })
    console.log(
        `annotations: ${annotations.length}; acquisition points needing one: ${coverage.constrained.length} (${coverage.uncovered.length} without); ` +
            `exempt: ${coverage.exempt.filter((point) => point.rule === 'F0').length} F0 constructors, ${coverage.exempt.filter((point) => point.rule === 'F1').length} F1 alone in their family`,
    )
    console.log(`nested same-family pairs: ${result.edges.length} (${result.exempted.length} listed exceptions); cross-family pairs left unordered (R5): ${result.crossFamily}`)
    if (args['require-annotations']) {
        for (const point of coverage.uncovered) result.errors.push(`${point.file}:${point.line} acquires ${point.idents.join(' | ')} without a sched-order annotation`)
    }
    for (const error of result.errors) console.error(error)
    if (result.errors.length > 0) process.exit(2)
    for (const exception of result.unusedExceptions) console.log(`exception no longer observed: ${exception.held} -> ${exception.acquired}`)
    for (const violation of result.stale) console.log(`no longer descending within one family: ${violation.site}`)
    for (const { held, acquired } of result.descending) console.log(`V_struct: ${acquired.file}:${acquired.statement} acquires ${acquired.ident} while holding ${held.ident}`)
    for (const violation of result.recorded) console.log(`V_struct (coarse screen): ${violation.site} acquires ${violation.acquired} while holding ${violation.held}`)
    if (result.descending.length + result.recorded.length > 0) process.exit(1)
    console.log('per-family order holds for every annotated and recorded path')
}
