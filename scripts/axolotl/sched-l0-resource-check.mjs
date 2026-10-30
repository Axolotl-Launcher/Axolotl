// Fourth invariant of the L0 supervisor (spec §5.1, §6.3.3): code in the L0
// module holds no resource of the total order. Leaf primitives are the only
// locks L0 may take; everything else must be dispatched to another level.
//
// Usage: node scripts/axolotl/sched-l0-resource-check.mjs [--dir <inventory dir>] [--root <dir to scan instead>]

import { existsSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { defaultOutDir, isMain, loadPolicy, loadSource, outputPaths, parseArgs, readJson, repoRoot, scanMethodHits, sourcesOf, workspaceCrates } from './sched-utils.mjs'

function filesUnder(dir) {
    const absolute = path.resolve(repoRoot, dir)
    if (!existsSync(absolute)) return []
    return readdirSync(absolute, { recursive: true })
        .filter((name) => name.endsWith('.rs'))
        .map((name) => `${dir}/${name.split(path.sep).join('/')}`)
}

export function l0Violations(sources, registry, policy) {
    const leaves = new Set(policy.leafPrimitives.map((leaf) => leaf.ident).filter(Boolean))
    const ordered = new Map()
    for (const item of registry.items) {
        if (leaves.has(item.ident)) continue
        const name = item.ident.split(/::|\./).at(-1)
        ordered.set(name, [...(ordered.get(name) ?? []), item.ident])
    }
    return scanMethodHits(sources)
        .filter((hit) => hit.receiver && ordered.has(hit.receiver) && !['const_new', 'Semaphore::new'].includes(hit.method))
        .map((hit) => ({ ...hit, idents: ordered.get(hit.receiver) }))
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const policy = loadPolicy()
    const registry = readJson(outputPaths(args.dir ?? defaultOutDir).registry)
    const root = args.root ?? policy.l0Module
    const sources = args.root
        ? filesUnder(root).map((rel) => ({ crate: { dir: 'packages/app-lib' }, ...loadSource(rel) }))
        : sourcesOf(workspaceCrates()).filter((source) => source.rel.startsWith(`${root}/`))
    console.log(`L0 module ${root}: ${sources.length} file(s) scanned`)
    const violations = l0Violations(sources, registry, policy)
    if (violations.length > 0) {
        for (const hit of violations) console.error(`${hit.file}:${hit.line} L0 acquires total-order resource ${hit.idents.join(' | ')}`)
        process.exit(1)
    }
    console.log('no total-order acquisition in the L0 module')
}
