// Critical-section whitelist (spec §8.2 and §11.7 J1). Writes outside any
// critical section are allowed only at a named `WhitelistSite`, and each one
// must meet all six admission criteria. Every guarded write API must also ask
// for a `WriteAccess` (CI rule ①, narrowed to the protected write paths).
//
// Usage: node scripts/axolotl/sched-whitelist-check.mjs [--dir <inventory dir>]

import { defaultOutDir, isMain, loadPolicy, outputPaths, parseArgs, readJson, sourcesOf, workspaceCrates } from './sched-utils.mjs'

export const WHITELIST_LIMIT = 10
const CRITICAL_SECTION = 'packages/app-lib/src/install/critical_section.rs'

/** Non-test variants of `enum WhitelistSite`, each with its doc comment. */
export function whitelistVariants(source) {
    const start = source.code.search(/\benum\s+WhitelistSite\s*\{/)
    if (start < 0) return null
    const open = source.code.indexOf('{', start)
    const close = source.code.indexOf('}', open)
    const variants = []
    for (const match of source.code.slice(open + 1, close).matchAll(/\b([A-Z]\w*)\s*,/g)) {
        const line = source.code.slice(0, open + 1 + match.index).split('\n').length
        const docs = []
        for (let index = line - 2; index >= 0 && /^\s*\/\/\//.test(source.rawLines[index]); index--) docs.unshift(source.rawLines[index].replace(/^\s*\/\/\/\s?/, ''))
        variants.push({ name: match[1], line, doc: docs.join(' ') })
    }
    return variants
}

export function checkWhitelist(sources, policy, registry) {
    const failures = []
    const section = sources.find((source) => source.rel === CRITICAL_SECTION)
    const variants = section ? whitelistVariants(section) : null
    if (!variants) return [`${CRITICAL_SECTION}: enum WhitelistSite not found`]
    if (variants.length > WHITELIST_LIMIT) failures.push(`whitelist has ${variants.length} sites; the cap is ${WHITELIST_LIMIT} (spec §11.8 R1: never raise it)`)
    const entries = new Map(policy.whitelist.map((entry) => [entry.site, entry]))
    const registered = new Set(registry.items.map((item) => item.ident))
    for (const variant of variants) {
        const entry = entries.get(variant.name)
        if (!entry) {
            failures.push(`WhitelistSite::${variant.name} has no entry in the whitelist table`)
            continue
        }
        if (!variant.doc.trim()) failures.push(`WhitelistSite::${variant.name} has no doc comment naming its protector`)
        const uses = sources.filter((source) => source.rel !== CRITICAL_SECTION).flatMap((source) => [...source.code.matchAll(new RegExp(`WhitelistSite::${variant.name}\\b`, 'g'))].map(() => source.rel))
        if (uses.length !== 1 || uses[0] !== entry.file) failures.push(`WhitelistSite::${variant.name} must be used exactly once, in ${entry.file}; found ${uses.join(', ') || 'none'}`)
        const file = sources.find((source) => source.rel === entry.file)
        // ① no cancelable scope in the file itself.
        if (file && /\brun_cancelable\s*\(/.test(file.code)) failures.push(`${variant.name} (criterion 1): ${entry.file} runs a cancelable scope`)
        // ② only the known idempotent or atomic variants.
        if (!policy.whitelistVariants.includes(entry.variant)) failures.push(`${variant.name} (criterion 2): ${entry.variant} is not a known whitelist variant`)
        // ③ a registered protector.
        if (!registered.has(entry.bearer)) failures.push(`${variant.name} (criterion 3): protector ${entry.bearer} is not a registered resource`)
        // ④ the variant is an existing guarded write, not a new write entry.
        if (!policy.guardedWrites.some((write) => entry.variant.endsWith(write.fn))) failures.push(`${variant.name} (criterion 4): ${entry.variant} is not one of the guarded write APIs`)
        // ⑤ an observable worst case.
        if (!entry.worstCase?.trim()) failures.push(`${variant.name} (criterion 5): no worst case`)
        // ⑥ linked to a declared limitation.
        if (!Number.isInteger(entry.limitation)) failures.push(`${variant.name} (criterion 6): no declared-limitation number`)
    }
    for (const entry of policy.whitelist) if (!variants.some((variant) => variant.name === entry.site)) failures.push(`whitelist table entry ${entry.site} has no WhitelistSite variant`)
    for (const write of policy.guardedWrites) {
        const source = sources.find((candidate) => candidate.rel === write.file)
        const signature = source?.code.match(new RegExp(`\\bfn\\s+${write.fn}\\s*(?:<[^>]*>)?\\s*\\(([^{]*?)\\)\\s*->`, 's'))
        if (!signature) failures.push(`${write.file}: fn ${write.fn} not found`)
        else if (!/\bWriteAccess\b/.test(signature[1])) failures.push(`${write.file}: fn ${write.fn} writes without asking for WriteAccess`)
    }
    return failures
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const policy = loadPolicy()
    const registry = readJson(outputPaths(args.dir ?? defaultOutDir).registry)
    const failures = checkWhitelist(sourcesOf(workspaceCrates()), policy, registry)
    console.log(`whitelist: ${policy.whitelist.length} site(s), cap ${WHITELIST_LIMIT}; guarded write APIs: ${policy.guardedWrites.length}`)
    if (failures.length > 0) {
        for (const failure of failures) console.error(failure)
        process.exit(1)
    }
    console.log('every whitelist site meets the six admission criteria')
}
