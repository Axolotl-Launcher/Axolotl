// Bare spawn and blocking forms (spec D17/D25): every production site in the
// workspace crates must be on the allowlist with a reason, so a new spawn or
// blocking call fails until it is either routed through the scheduler or
// explicitly allowed. Unbounded channels are banned only inside the L0 module.
//
// Each entry records its sites as `<enclosing fn>: <source line>`, so replacing
// one site with another of the same form fails even when the count holds.
//
// Usage: node scripts/axolotl/sched-bare-spawn-check.mjs [--allowlist <file>] [--update]
// --update rewrites the counts and sites and keeps existing reasons; new
// entries get an empty reason, which fails the check until someone writes one.

import { existsSync } from 'node:fs'
import path from 'node:path'
import { functionBodies, isMain, lineOf, loadPolicy, parseArgs, readJson, repoRoot, sourcesOf, workspaceCrates, writeJson } from './sched-utils.mjs'

export const allowlistPath = 'scripts/axolotl/sched-spawn-allowlist.json'
export const FORMS = {
    'tokio::spawn': /\btokio::(?:task::)?spawn\s*\(/g,
    spawn_blocking: /\bspawn_blocking\s*\(/g,
    'std::thread::spawn': /\b(?:std::)?thread::spawn\s*\(/g,
    'thread::Builder': /\b(?:std::)?thread::Builder::new\s*\(/g,
    // A builder can be stored and spawned later, so in files that build one
    // every `.spawn(` call counts; a new spawn on an old builder grows it.
    'thread::Builder spawn': /\.spawn\s*\(/g,
    // Scoped threads spawn through the scope handle, so in files that open a
    // scope every `.spawn(` call counts too.
    'thread::scope spawn': /\.spawn\s*\(/g,
    'tauri::async_runtime::spawn': /\b(?:tauri::)?async_runtime::spawn\s*\(/g,
    'std::fs': /\bstd::fs::/g,
    'std::thread::sleep': /\b(?:std::)?thread::sleep\s*\(/g,
}
/** Forms counted only in files that also match the gate pattern. */
const GATED = {
    'thread::Builder spawn': /\b(?:std::)?thread::Builder::new\s*\(/,
    'thread::scope spawn': /\b(?:std::)?thread::scope\s*\(/,
}
const FS_IMPORT = 'std::fs import'
const UNBOUNDED = /\b(UnboundedSender|UnboundedReceiver|unbounded_channel|mpsc::unbounded)\b/g

/** Splits a use-tree list on the commas outside nested braces. */
function splitTopLevel(text) {
    const parts = []
    let depth = 0
    let start = 0
    for (let i = 0; i < text.length; i++) {
        if (text[i] === '{') depth++
        else if (text[i] === '}') depth--
        else if (text[i] === ',' && depth === 0) {
            parts.push(text.slice(start, i))
            start = i + 1
        }
    }
    parts.push(text.slice(start))
    return parts.map((part) => part.trim()).filter(Boolean)
}

/** Expands `std::{fs::{self, File as F}, io}` into one path and local name per import. */
function expandUseTree(text, prefix = []) {
    const brace = text.indexOf('{')
    if (brace === -1) {
        const parsed = text.match(/^([\w:\s]+?)(?:\s+as\s+(\w+))?$/)
        if (!parsed) return []
        const segments = [...prefix, ...parsed[1].split('::').map((segment) => segment.trim()).filter(Boolean)]
        if (segments.at(-1) === 'self') segments.pop()
        return [{ segments, local: parsed[2] ?? segments.at(-1) }]
    }
    const base = [...prefix, ...text.slice(0, brace).split('::').map((segment) => segment.trim()).filter(Boolean)]
    return splitTopLevel(text.slice(brace + 1, text.lastIndexOf('}'))).flatMap((part) => expandUseTree(part, base))
}

/**
 * Call sites of `std::fs` names brought in by `use`: `fs::read(..)` after
 * `use std::fs`, `OpenOptions::new()` after `use std::fs::OpenOptions`. The
 * literal `std::fs::` form misses these, and so would a new site in a file
 * whose only literal match is the import itself.
 */
export function fsImportSites(code) {
    const statements = [...code.matchAll(/\buse\s+(?:::)?(std\s*::[^;]*);/g)]
    const inside = (index) => statements.some((statement) => index >= statement.index && index < statement.index + statement[0].length)
    const patterns = []
    for (const statement of statements) {
        for (const { segments, local } of expandUseTree(statement[1])) {
            if (segments[0] !== 'std' || segments[1] !== 'fs' || !local || local === '*' || local === '_') continue
            // The module itself, a type (`File::open`) or a function (`read_to_string(`).
            const use = segments.length === 2 ? '::' : /^[A-Z]/.test(local) ? '\\s*::' : '\\s*\\('
            patterns.push(new RegExp(`(?<![\\w:.])${local}${use}`, 'g'))
        }
    }
    return patterns.flatMap((pattern) => [...code.matchAll(pattern)].map((match) => match.index)).filter((index) => !inside(index)).sort((a, b) => a - b)
}

/** `<innermost fn>: <trimmed source line>`, the identity a site keeps while code around it moves. */
function siteOf(source, bodies, line) {
    const owner = bodies.filter((body) => body.from <= line && line <= body.to).reduce((inner, body) => (!inner || body.from >= inner.from ? body : inner), null)
    return `${owner?.name ?? '<module>'}: ${(source.rawLines[line - 1] ?? '').trim().replace(/\s+/g, ' ')}`
}

export function countForms(sources) {
    const counts = new Map()
    for (const source of sources) {
        const bodies = functionBodies(source)
        const record = (form, offsets) => {
            if (offsets.length === 0) return
            const lines = offsets.map((offset) => lineOf(source.starts, offset))
            const sites = lines.map((line) => siteOf(source, bodies, line)).sort()
            counts.set(`${source.rel}\0${form}`, { file: source.rel, form, count: lines.length, lines, sites })
        }
        for (const [form, pattern] of Object.entries(FORMS)) {
            if (GATED[form] && !GATED[form].test(source.code)) continue
            record(form, [...source.code.matchAll(pattern)].map((match) => match.index))
        }
        record(FS_IMPORT, fsImportSites(source.code))
    }
    return counts
}

export function unboundedInL0(sources, l0Module) {
    return sources
        .filter((source) => source.rel.startsWith(`${l0Module}/`))
        .flatMap((source) => [...source.code.matchAll(UNBOUNDED)].map((match) => ({ file: source.rel, line: lineOf(source.starts, match.index), form: match[1] })))
}

/** Items of `left` not matched one-for-one in `right`. */
function without(left, right) {
    const remaining = new Map()
    for (const item of right) remaining.set(item, (remaining.get(item) ?? 0) + 1)
    return left.filter((item) => {
        const count = remaining.get(item) ?? 0
        if (count === 0) return true
        remaining.set(item, count - 1)
        return false
    })
}

export function checkSpawns(counts, allowlist) {
    const failures = []
    const stale = []
    const allowed = new Map(allowlist.entries.map((entry) => [`${entry.file}\0${entry.form}`, entry]))
    for (const [key, found] of counts) {
        const entry = allowed.get(key)
        if (!entry) {
            failures.push(`${found.file}: ${found.count} unregistered ${found.form} (lines ${found.lines.join(', ')})`)
            continue
        }
        if (!Array.isArray(entry.sites)) {
            failures.push(`${found.file}: allowlisted ${found.form} records no sites; review them and run --update`)
            continue
        }
        const added = without(found.sites, entry.sites)
        const gone = without(entry.sites, found.sites)
        for (const site of added) failures.push(`${found.file}: ${found.form} site not on the allowlist: ${site}`)
        if (gone.length > 0) stale.push(`${found.file}: ${found.form} lost ${gone.length} allowlisted site(s); lower the allowlist`)
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
        const entries = [...counts.values()].map(({ file: rel, form, count, sites }) => ({ file: rel, form, count, reason: previous.get(`${rel}\0${form}`)?.reason ?? '', sites }))
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
