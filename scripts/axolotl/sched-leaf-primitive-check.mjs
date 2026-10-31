// Leaf primitives (spec §5.2): the six registered leaves, with the worst-case
// critical section of the two that are not bounded, and the rule that L0 code
// locks nothing outside the leaf table unless it is a recorded pending access
// awaiting phase-3 dispatch separation.
//
// Usage: node scripts/axolotl/sched-leaf-primitive-check.mjs [--dir <inventory dir>]

import { defaultOutDir, functionBodies, isMain, loadPolicy, outputPaths, parseArgs, readJson, scanMethodHits, sourcesOf, workspaceCrates } from './sched-utils.mjs'

/** Lock calls in L0 member scopes: whole files, or one function when `fn` is given. */
export function l0LockHits(sources, members) {
    const hits = []
    for (const member of members) {
        const source = sources.find((candidate) => candidate.rel === member.file)
        if (!source) continue
        const body = member.fn ? functionBodies(source).find((candidate) => candidate.name === member.fn) : null
        if (member.fn && !body) continue
        for (const hit of scanMethodHits([source])) {
            if (body && (hit.line < body.from || hit.line > body.to)) continue
            if (['const_new', 'Semaphore::new'].includes(hit.method)) continue
            hits.push({ ...hit, member: member.class })
        }
    }
    return hits
}

export function checkLeaves(policy, inventory, sources) {
    const failures = []
    const generated = new Map(inventory.items.map((item) => [item.ident, item]))
    if (policy.leafPrimitives.length !== 6) failures.push(`leaf table must list the 6 leaves of spec §5.2, found ${policy.leafPrimitives.length}`)
    for (const leaf of policy.leafPrimitives) {
        if (leaf.ident && !generated.has(leaf.ident)) failures.push(`leaf ${leaf.name}: ${leaf.ident} is not in the inventory`)
        if (!leaf.ident && !leaf.planned) failures.push(`leaf ${leaf.name}: neither present nor planned`)
        if (leaf.bounded === false && !(leaf.worstCaseMs > 0)) failures.push(`leaf ${leaf.name}: unbounded leaf needs its worst-case critical section`)
    }
    const leafNames = new Set(policy.leafPrimitives.filter((leaf) => leaf.ident).map((leaf) => leaf.ident.split(/::|\./).at(-1)))
    const pending = new Set(policy.l0PendingAccess.map((entry) => `${entry.file}\0${entry.receiver}`))
    const unregistered = []
    for (const hit of l0LockHits(sources, policy.l0Members)) {
        if (hit.receiver && (leafNames.has(hit.receiver) || pending.has(`${hit.file}\0${hit.receiver}`))) continue
        unregistered.push(hit)
        failures.push(`${hit.file}:${hit.line} .${hit.method}() on ${hit.receiver ?? 'an unnamed receiver'} in L0 (${hit.member}) is neither a leaf nor a recorded pending access`)
    }
    return { failures, unregistered }
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const policy = loadPolicy()
    const inventory = readJson(outputPaths(args.dir ?? defaultOutDir).inventory)
    const { failures } = checkLeaves(policy, inventory, sourcesOf(workspaceCrates()))
    for (const leaf of policy.leafPrimitives) {
        const bound = leaf.bounded ? 'bounded' : `unbounded, worst case <= ${leaf.worstCaseMs} ms`
        console.log(`leaf: ${leaf.name} -> ${leaf.ident ?? `planned in ${leaf.planned}`} (${bound})`)
    }
    if (failures.length > 0) {
        for (const failure of failures) console.error(failure)
        process.exit(1)
    }
    console.log(`L0 lock calls outside the leaf table: ${policy.l0PendingAccess.length} recorded pending accesses, 0 unregistered`)
}
