// Generates the managed-resource inventory of the scheduler design (spec §6.3.1):
// the generated set G, the registry R with ranks, the exclusions E, the
// acquisition points and the recall gap C \ G. Output is deterministic and
// carries no expected counts; the gates compare sets, not sizes.
//
// Usage: node scripts/axolotl/sched-resource-inventory.mjs [--out <dir>] [--policy <file>]

import { mkdirSync } from 'node:fs'
import path from 'node:path'
import {
    assignRanks,
    bareName,
    classify,
    compareItems,
    constructionSites,
    defaultOutDir,
    EXCLUSION_RULES,
    isMain,
    loadPolicy,
    longLivedTypes,
    outputPaths,
    parseArgs,
    policyPath,
    primitiveNames,
    rankFingerprint,
    repoRoot,
    scanDeclarationLines,
    scanDeclarations,
    scanMethodHits,
    sourcesOf,
    workspaceCrates,
    writeJson,
} from './sched-utils.mjs'

export function buildInventory(policy) {
    const invalid = policy.exclusions.filter((entry) => !EXCLUSION_RULES.includes(entry.rule) || !entry.reason?.trim())
    if (invalid.length > 0) {
        const names = invalid.map((entry) => entry.ident).join(', ')
        throw new Error(`exclusions must cite one of ${EXCLUSION_RULES.join('/')} with a reason (spec §6.3.1): ${names}`)
    }

    const crates = workspaceCrates()
    const sources = sourcesOf(crates)
    const primitives = primitiveNames(sources)
    const generated = scanDeclarations(sources, primitives)
    const longLived = longLivedTypes(sources, policy.longLivedRoots)
    const curatedExclusions = new Map(policy.exclusions.map((entry) => [entry.ident, entry]))
    const hostOf = (item) => (item.kind === 'field' ? `${item.module}::${item.name.split('.')[0]}` : null)
    // Rule D2 needs proof (spec §11.7 J2): at least one construction site and
    // every site inside a function body. Without it the field stays registered.
    const constructionCache = new Map()
    const perOperation = (crate, host) => {
        const key = `${crate}\0${host}`
        if (!constructionCache.has(key)) {
            const sites = constructionSites(sources, crate, host)
            constructionCache.set(key, sites.length > 0 && sites.every((site) => site.inFunction) ? sites : null)
        }
        return constructionCache.get(key)
    }

    const registered = []
    const excluded = []
    for (const item of generated) {
        const curated = curatedExclusions.get(item.ident) ?? curatedExclusions.get(`${hostOf(item)}.*`)
        const host = item.name.split('.')[0]
        if (!item.holding) {
            excluded.push({ ...item, rule: 'D3', reason: `type holds no acquire-hold-release primitive: ${item.type}` })
        } else if (curated) {
            excluded.push({ ...item, rule: curated.rule, reason: curated.reason })
        } else if (item.kind === 'field' && !longLived.has(host) && !policy.longLivedRoots.includes(host) && perOperation(item.crate, host)) {
            const sites = perOperation(item.crate, host)
            excluded.push({
                ...item,
                rule: 'D2',
                reason: `host ${host} is unreachable from statics, State fields and Tauri-managed state, and is only constructed inside function bodies`,
                evidence: sites.map((site) => `${site.file}:${site.line} (fn body)`),
            })
        } else {
            registered.push({ ...item, ...classify(item, policy) })
        }
    }
    // Curated exclusions that name nothing generated stay in E so the
    // consistency check (E ⊆ G) reports them instead of hiding them here.
    for (const entry of policy.exclusions) {
        const host = entry.ident.endsWith('.*') ? entry.ident.slice(0, -2) : null
        if (!generated.some((item) => item.ident === entry.ident || (host && hostOf(item) === host))) excluded.push({ ident: entry.ident, rule: entry.rule, reason: entry.reason, file: null, line: null })
    }

    const registry = assignRanks(registered, policy)
    const byCrateName = new Map()
    for (const item of generated) {
        const key = `${item.crate}\0${bareName(item)}`
        if (!byCrateName.has(key)) byCrateName.set(key, [])
        byCrateName.get(key).push(item)
    }

    const getpoints = []
    const codeSide = []
    for (const hit of scanMethodHits(sources)) {
        if (!hit.receiver || hit.via === 'binding') continue
        const candidates = byCrateName.get(`${hit.crate}\0${hit.receiver}`) ?? []
        const sameFile = candidates.filter((item) => item.file === hit.file)
        const owners = sameFile.length > 0 ? sameFile : candidates
        if (owners.length > 0) {
            getpoints.push({ file: hit.file, line: hit.line, method: hit.method, receiver: hit.receiver, idents: owners.map((item) => item.ident) })
        }
        codeSide.push({ crate: hit.crate, file: hit.file, line: hit.line, name: hit.receiver, via: hit.via, method: hit.method, semaphore: hit.semaphore, matched: owners.length > 0 })
    }
    for (const decl of scanDeclarationLines(sources)) {
        const matched = byCrateName.has(`${decl.crate}\0${decl.name}`)
        codeSide.push({ ...decl, method: null, matched })
    }

    const recallGap = new Map()
    for (const entry of codeSide.filter((candidate) => !candidate.matched)) {
        const key = `${entry.crate}::${entry.name}`
        const curated = policy.recallGap[key]
        const gap = recallGap.get(key) ?? {
            key,
            crate: entry.crate,
            name: entry.name,
            sites: [],
            reason: curated?.reason ?? (entry.via.endsWith('-decl') ? 'cross-line-declaration' : 'other'),
            detail: curated?.detail ?? null,
            classHint: null,
            unconstrained: true,
        }
        gap.sites.push(`${entry.file}:${entry.line}`)
        if (entry.semaphore) gap.classHint = 'S5/S9'
        recallGap.set(key, gap)
    }

    return {
        inventory: {
            scope: crates.map((crate) => ({ dir: crate.dir, name: crate.name })),
            primitives: [...primitives].sort(),
            items: generated.map(({ holding, ...item }) => ({
                ...item,
                status: registered.some((entry) => entry.ident === item.ident && entry.file === item.file) ? 'registered' : 'excluded',
            })),
        },
        registry: {
            classOrder: policy.classOrder,
            fingerprint: rankFingerprint(registry),
            items: registry.map(({ ident, rank, scopeClass, classSource, file, line, type }) => ({ ident, rank, scopeClass, classSource, file, line, type })),
        },
        exclusions: {
            items: excluded.sort(compareItems).map(({ ident, rule, reason, evidence, file, line }) => ({ ident, rule, reason, ...(evidence ? { evidence } : {}), file, line })),
        },
        getpoints: {
            items: getpoints.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line),
        },
        recallGap: {
            items: [...recallGap.values()].sort((a, b) => a.key.localeCompare(b.key)),
        },
    }
}

if (import.meta.url === new URL(`file:///${path.resolve(process.argv[1]).replaceAll('\\', '/')}`).href) {
    const args = parseArgs(process.argv.slice(2))
    const outDir = args.out ?? defaultOutDir
    try {
        const result = buildInventory(loadPolicy(args.policy ?? policyPath))
        mkdirSync(path.resolve(repoRoot, outDir), { recursive: true })
        const paths = outputPaths(outDir)
        for (const [key, file] of Object.entries(paths)) writeJson(file, result[key])
        for (const file of Object.values(paths)) console.log(file)
    } catch (error) {
        console.error(`sched-resource-inventory: ${error.message}`)
        process.exit(1)
    }
}
