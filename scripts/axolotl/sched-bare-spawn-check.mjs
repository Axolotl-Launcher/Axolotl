// Bare spawn and blocking forms (spec D17/D25): every production site in the
// workspace crates must be on the allowlist with a reason, so a new spawn or
// blocking call fails until it is either routed through the scheduler or
// explicitly allowed. Unbounded channels are banned only inside the L0 module.
//
// Usage: node scripts/axolotl/sched-bare-spawn-check.mjs [--allowlist <file>] [--update]
// --update rewrites the counts and keeps existing reasons; new entries get an
// empty reason, which fails the check until someone writes one.

import { existsSync } from 'node:fs'
import path from 'node:path'
import { isMain, lineOf, loadPolicy, parseArgs, readJson, repoRoot, sourcesOf, workspaceCrates, writeJson } from './sched-utils.mjs'

export const allowlistPath = 'scripts/axolotl/sched-spawn-allowlist.json'
export const FORMS = {
    'tokio::spawn': /\btokio::(?:task::)?spawn\s*\(/g,
    spawn_blocking: /\bspawn_blocking\s*\(/g,
    'std::thread::spawn': /\b(?:std::)?thread::spawn\s*\(/g,
    'thread::Builder': /\b(?:std::)?thread::Builder::new\s*\(/g,
    // A builder can be stored and spawned later, so in files that build one
    // every `.spawn(` call counts; a new spawn on an old builder grows it.
    'thread::Builder spawn': /\.spawn\s*\(/g,
    'tauri::async_runtime::spawn': /\b(?:tauri::)?async_runtime::spawn\s*\(/g,
    'std::fs': /\bstd::fs::/g,
    'std::thread::sleep': /\b(?:std::)?thread::sleep\s*\(/g,
}
const UNBOUNDED = /\b(UnboundedSender|UnboundedReceiver|unbounded_channel|mpsc::unbounded)\b/g

export function countForms(sources) {
    const counts = new Map()
    for (const source of sources) {
        const buildsThreads = /\b(?:std::)?thread::Builder::new\s*\(/.test(source.code)
        for (const [form, pattern] of Object.entries(FORMS)) {
            if (form === 'thread::Builder spawn' && !buildsThreads) continue
            const lines = [...source.code.matchAll(pattern)].map((match) => lineOf(source.starts, match.index))
            if (lines.length > 0) counts.set(`${source.rel}\0${form}`, { file: source.rel, form, count: lines.length, lines })
        }
    }
    return counts
}

export function unboundedInL0(sources, l0Module) {
    return sources
        .filter((source) => source.rel.startsWith(`${l0Module}/`))
        .flatMap((source) => [...source.code.matchAll(UNBOUNDED)].map((match) => ({ file: source.rel, line: lineOf(source.starts, match.index), form: match[1] })))
}

export function checkSpawns(counts, allowlist) {
    const failures = []
    const stale = []
    const allowed = new Map(allowlist.entries.map((entry) => [`${entry.file}\0${entry.form}`, entry]))
    for (const [key, found] of counts) {
        const entry = allowed.get(key)
        if (!entry) failures.push(`${found.file}: ${found.count} unregistered ${found.form} (lines ${found.lines.join(', ')})`)
        else if (found.count > entry.count) failures.push(`${found.file}: ${found.form} grew from ${entry.count} to ${found.count} (lines ${found.lines.join(', ')})`)
        else if (found.count < entry.count) stale.push(`${found.file}: ${found.form} shrank from ${entry.count} to ${found.count}; lower the allowlist`)
    }
    for (const entry of allowlist.entries) {
        if (!entry.reason?.trim()) failures.push(`${entry.file}: allowlisted ${entry.form} has no reason`)
        if (!counts.has(`${entry.file}\0${entry.form}`)) stale.push(`${entry.file}: ${entry.form} no longer occurs; drop it from the allowlist`)
    }
    return { failures, stale }
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const crates = workspaceCrates()
    const sources = sourcesOf(crates)
    const counts = countForms(sources)
    const file = args.allowlist ?? allowlistPath
    if (args.update) {
        const previous = new Map((existsSync(path.resolve(repoRoot, file)) ? readJson(file).entries : []).map((entry) => [`${entry.file}\0${entry.form}`, entry]))
        const entries = [...counts.values()].map(({ file: rel, form, count }) => ({ file: rel, form, count, reason: previous.get(`${rel}\0${form}`)?.reason ?? '' }))
        writeJson(file, { entries: entries.sort((a, b) => a.file.localeCompare(b.file) || a.form.localeCompare(b.form)) })
    }
    const { failures, stale } = checkSpawns(counts, readJson(file))
    const unbounded = unboundedInL0(sources, loadPolicy().l0Module)
    for (const hit of unbounded) failures.push(`${hit.file}:${hit.line} ${hit.form} is banned inside the L0 module`)
    const total = [...counts.values()].reduce((sum, found) => sum + found.count, 0)
    console.log(`scope: ${crates.length} crates; ${total} spawn/blocking sites in ${new Set([...counts.values()].map((found) => found.file)).size} files`)
    for (const note of stale) console.log(`note: ${note}`)
    if (failures.length > 0) {
        for (const failure of failures) console.error(failure)
        process.exit(1)
    }
    console.log('every site is allowlisted with a reason')
}
