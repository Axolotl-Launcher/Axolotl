// Fourth invariant of the L0 supervisor (spec §5.1, §6.3.3): code in the L0
// module holds no resource of the total order. Leaf primitives are the only
// locks L0 may take; everything else must be dispatched to another level.
//
// Usage: node scripts/axolotl/sched-l0-resource-check.mjs [--dir <inventory dir>] [--root <dir to scan instead>]

import { existsSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { defaultOutDir, functionBodies, isMain, loadPolicy, loadSource, outputPaths, parseArgs, readJson, repoRoot, scanMethodHits, sourcesOf, workspaceCrates } from './sched-utils.mjs'

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

/**
 * L0 functions outside the L0 module (the detectors left in their original
 * files) may only await the callees the policy allows and may not touch the
 * reactions that belong to other levels: the database, events, spawns,
 * blocking file I/O and child processes.
 */
export function l0FunctionViolations(sources, policy) {
    const violations = []
    for (const entry of policy.l0Functions ?? []) {
        const source = sources.find((candidate) => candidate.rel === entry.file)
        const body = source && functionBodies(source).find((candidate) => candidate.name === entry.fn)
        if (!body) {
            violations.push(`${entry.file}: L0 function ${entry.fn} not found`)
            continue
        }
        const code = source.lines.slice(body.from - 1, body.to).join('\n')
        for (const pattern of policy.l0ForbiddenPatterns) {
            for (const match of code.matchAll(new RegExp(pattern, 'g'))) {
                violations.push(`${entry.file}:${body.from + code.slice(0, match.index).split('\n').length - 1} L0 function ${entry.fn} uses ${match[0].trim()}`)
            }
        }
        for (const match of code.matchAll(/\.await\b/g)) {
            const statement = code.slice(0, match.index).split(/[;{}]/).at(-1)
            if (!entry.allowedAwaits.some((allowed) => new RegExp(allowed).test(statement))) {
                violations.push(`${entry.file}:${body.from + code.slice(0, match.index).split('\n').length - 1} L0 function ${entry.fn} awaits ${statement.trim().replace(/\s+/g, ' ')}`)
            }
        }
    }
    return violations
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const policy = loadPolicy()
    const registry = readJson(outputPaths(args.dir ?? defaultOutDir).registry)
    const root = args.root ?? policy.l0Module
    const all = args.root ? [] : sourcesOf(workspaceCrates())
    const sources = args.root
        ? filesUnder(root).map((rel) => ({ crate: { dir: 'packages/app-lib' }, ...loadSource(rel) }))
        : all.filter((source) => source.rel.startsWith(`${root}/`))
    console.log(`L0 module ${root}: ${sources.length} file(s) scanned; L0 functions elsewhere: ${(policy.l0Functions ?? []).length}`)
    const violations = l0Violations(sources, registry, policy)
    const functionViolations = args.root ? [] : l0FunctionViolations(all, policy)
    for (const hit of violations) console.error(`${hit.file}:${hit.line} L0 acquires total-order resource ${hit.idents.join(' | ')}`)
    for (const violation of functionViolations) console.error(violation)
    if (violations.length + functionViolations.length > 0) process.exit(1)
    console.log('no total-order acquisition and no dispatched reaction inside L0')
}
