// Shared scanning for the scheduler gates (`sched-*.mjs`). The scans are
// lexical: they read Rust source with comments and literals blanked out, so
// every result is a lower bound of what the code really contains.

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync, realpathSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const repoRoot = path.resolve(import.meta.dirname, '..', '..')
export const policyPath = 'scripts/axolotl/sched-policy.json'
export const defaultOutDir = '.agents/plans'

/** Synchronization primitives that make a declaration a managed resource. */
export const PRIMITIVES = ['Semaphore', 'Mutex', 'RwLock', 'AsyncMutex', 'AsyncRwLock', 'DashMap', 'DashSet']
/** Types that are not acquire-hold-release resources (exclusion rule D3). */
export const NON_HOLDING = ['Notify', 'CancellationToken', 'watch::Sender', 'watch::Receiver']
export const EXCLUSION_RULES = ['D1', 'D2', 'D3', 'D4']
export const SCOPE_CLASSES = ['S1', 'S2', 'S3', 'S4', 'S5', 'S6', 'S7', 'S8', 'S9', 'S10']

export function isMain(moduleUrl) {
    return Boolean(process.argv[1]) && realpathSync(fileURLToPath(moduleUrl)) === realpathSync(process.argv[1])
}

export function readJson(file) {
    return JSON.parse(readFileSync(path.resolve(repoRoot, file), 'utf8'))
}

/** Writes JSON deterministically so two runs over the same input are byte-identical. */
export function writeJson(file, value) {
    writeFileSync(path.resolve(repoRoot, file), `${JSON.stringify(value, null, 2)}\n`)
}

export function sha256(text) {
    return createHash('sha256').update(text).digest('hex')
}

export function loadPolicy(file = policyPath) {
    return readJson(file)
}

/** Workspace members as reported by `cargo metadata --no-deps`, not the hand-written `members` list. */
export function workspaceCrates() {
    const output = execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
        cwd: repoRoot,
        encoding: 'utf8',
        maxBuffer: 64 * 1024 * 1024,
        stdio: ['ignore', 'pipe', 'pipe'],
    })
    const metadata = JSON.parse(output)
    const members = new Set(metadata.workspace_members)
    return metadata.packages
        .filter((pkg) => members.has(pkg.id))
        .map((pkg) => {
            const dir = toRepoPath(path.dirname(pkg.manifest_path))
            const root = pkg.targets.find((target) => target.kind.includes('lib')) ?? pkg.targets.find((target) => target.kind.includes('bin'))
            return { name: pkg.name, dir, ident: (root?.name ?? pkg.name).replaceAll('-', '_') }
        })
        .sort((a, b) => a.dir.localeCompare(b.dir))
}

export function toRepoPath(file) {
    return path.relative(repoRoot, path.resolve(repoRoot, file)).split(path.sep).join('/')
}

/**
 * Production source files of a crate, found by walking `mod name;` declarations
 * from the crate root (honouring `#[path]`). Modules declared under
 * `#[cfg(test)]` are blanked before the walk, so test-only files never appear.
 */
export function crateModules(crate) {
    const rootFile = ['lib.rs', 'main.rs'].map((name) => `${crate.dir}/src/${name}`).find((rel) => existsSync(path.resolve(repoRoot, rel)))
    const modules = []
    const seen = new Set()
    const visit = (rel, module, ownsDir) => {
        if (seen.has(rel) || !existsSync(path.resolve(repoRoot, rel))) return
        seen.add(rel)
        const source = loadSource(rel)
        modules.push({ ...source, module })
        const dir = path.posix.dirname(rel)
        const childDir = ownsDir ? dir : `${dir}/${path.posix.basename(rel, '.rs')}`
        for (const match of source.code.matchAll(/\bmod\s+([a-z_]\w*)\s*;/g)) {
            const attributes = source.raw.slice(Math.max(0, match.index - 200), match.index)
            const custom = attributes.match(/#\[path\s*=\s*"([^"]+)"\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?$/)
            const name = match[1]
            const child = custom
                ? path.posix.normalize(`${dir}/${custom[1]}`)
                : [`${childDir}/${name}.rs`, `${childDir}/${name}/mod.rs`].find((candidate) => existsSync(path.resolve(repoRoot, candidate)))
            if (child) visit(child, `${module}::${name}`, child.endsWith('/mod.rs'))
        }
    }
    if (rootFile) visit(rootFile, crate.ident, true)
    return modules.sort((a, b) => a.rel.localeCompare(b.rel))
}

/**
 * Blanks comments, string and char literals with spaces, keeping offsets and
 * newlines, so brace matching and regexes only see code.
 */
export function maskRust(text) {
    const out = text.split('')
    const blank = (from, to) => {
        for (let i = from; i < to; i++) if (out[i] !== '\n' && out[i] !== '\r') out[i] = ' '
    }
    let i = 0
    while (i < text.length) {
        const c = text[i]
        const next = text[i + 1]
        if (c === '/' && next === '/') {
            const end = text.indexOf('\n', i)
            const stop = end === -1 ? text.length : end
            blank(i, stop)
            i = stop
        } else if (c === '/' && next === '*') {
            let depth = 1
            let j = i + 2
            while (j < text.length && depth > 0) {
                if (text[j] === '/' && text[j + 1] === '*') {
                    depth++
                    j += 2
                } else if (text[j] === '*' && text[j + 1] === '/') {
                    depth--
                    j += 2
                } else j++
            }
            blank(i, j)
            i = j
        } else if ((c === 'r' || (c === 'b' && next === 'r')) && /^b?r#*"/.test(text.slice(i, i + 12)) && !/\w/.test(text[i - 1] ?? '')) {
            const head = text.slice(i).match(/^b?r(#*)"/)
            const close = `"${head[1]}`
            const end = text.indexOf(close, i + head[0].length)
            const stop = end === -1 ? text.length : end + close.length
            blank(i, stop)
            i = stop
        } else if (c === '"') {
            let j = i + 1
            while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1
            blank(i, j + 1)
            i = j + 1
        } else if (c === "'") {
            const literal = text.slice(i).match(/^'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'])'/)
            if (literal) {
                blank(i, i + literal[0].length)
                i += literal[0].length
            } else i++
        } else i++
    }
    return out.join('')
}

export function matchingBrace(masked, open) {
    let depth = 0
    for (let i = open; i < masked.length; i++) {
        if (masked[i] === '{') depth++
        else if (masked[i] === '}' && --depth === 0) return i
    }
    return masked.length - 1
}

/** Blanks every item annotated `#[cfg(test)]` (or `cfg(all(test, ...))`). */
export function stripTestItems(masked) {
    const out = masked.split('')
    const attribute = /#\[cfg\((all\()?test\b[^\]]*\]/g
    let match
    while ((match = attribute.exec(masked))) {
        const start = match.index
        let j = start + match[0].length
        let end = masked.length - 1
        let depth = 0
        // An item ends at `;` or its body; a field, variant or argument ends at a top-level `,`.
        for (; j < masked.length; j++) {
            const ch = masked[j]
            if ('([<'.includes(ch)) depth++
            else if (')]'.includes(ch) || (ch === '>' && !'-='.includes(masked[j - 1]))) {
                if (--depth < 0) {
                    end = j - 1
                    break
                }
            } else if (depth === 0 && (ch === ';' || ch === ',')) {
                end = j
                break
            } else if (ch === '{') {
                if (depth === 0) {
                    end = matchingBrace(masked, j)
                    break
                }
                j = matchingBrace(masked, j)
            } else if (ch === '}' && depth === 0) {
                end = j - 1
                break
            }
        }
        for (let k = start; k <= end; k++) if (out[k] !== '\n') out[k] = ' '
        attribute.lastIndex = end + 1
    }
    return out.join('')
}

export function lineStarts(text) {
    const starts = [0]
    for (let i = 0; i < text.length; i++) if (text[i] === '\n') starts.push(i + 1)
    return starts
}

export function lineOf(starts, offset) {
    let lo = 0
    let hi = starts.length - 1
    while (lo < hi) {
        const mid = (lo + hi + 1) >> 1
        if (starts[mid] <= offset) lo = mid
        else hi = mid - 1
    }
    return lo + 1
}

/** Loads one source file: raw text, masked code without test items, and line offsets. */
export function loadSource(rel) {
    const raw = readFileSync(path.resolve(repoRoot, rel), 'utf8').replaceAll('\r\n', '\n')
    const code = stripTestItems(maskRust(raw))
    return { rel, raw, code, starts: lineStarts(code), lines: code.split('\n'), rawLines: raw.split('\n') }
}

/** Production sources of every crate, each tagged with its crate and module path. */
export function sourcesOf(crates) {
    const sources = []
    for (const crate of crates) for (const source of crateModules(crate)) sources.push({ crate, ...source })
    return sources
}

/** Type text that starts on `line` (1-based) after `column`, joined over at most 3 more lines. */
function typeText(lines, line, column, stop) {
    let text = lines[line - 1].slice(column)
    for (let k = 0; k < 3 && !stop.test(text); k++) text += ` ${lines[line + k] ?? ''}`
    const cut = text.search(stop)
    return (cut === -1 ? text : text.slice(0, cut)).replace(/\s+/g, ' ').trim()
}

function wordPattern(names) {
    return new RegExp(`(^|[^\\w])(${names.map((name) => name.replaceAll(':', '\\:')).join('|')})(?![\\w])`)
}

/**
 * Newtypes and aliases that wrap a primitive (`struct FetchSemaphore(pub Semaphore)`,
 * `type Locks = DashMap<..>`) count as primitives, iterated to a fixpoint.
 */
export function primitiveNames(sources) {
    const names = new Set(PRIMITIVES)
    let grew = true
    while (grew) {
        grew = false
        const pattern = wordPattern([...names])
        for (const source of sources) {
            for (const match of source.code.matchAll(/\b(?:struct\s+([A-Z]\w*)\s*(?:<[^>{;]*>)?\s*\(([^;{]*)\)\s*;|type\s+([A-Z]\w*)\s*(?:<[^>=]*>)?\s*=([^;]*);)/g)) {
                const name = match[1] ?? match[3]
                if (!names.has(name) && pattern.test(match[2] ?? match[4])) {
                    names.add(name)
                    grew = true
                }
            }
        }
    }
    return names
}

/**
 * Generator side of the inventory: `static` declarations and struct fields whose
 * type mentions a primitive (or only a D3 non-holding type), multi-line aware.
 */
export function scanDeclarations(sources, primitives) {
    const holding = wordPattern([...primitives])
    const nonHolding = wordPattern(NON_HOLDING)
    const items = []
    const push = (source, ident, host, line, type) => {
        const holds = holding.test(type)
        if (!holds && !nonHolding.test(type)) return
        const module = source.module
        const name = host ? `${host}.${ident}` : ident
        items.push({
            ident: `${module}::${name}`,
            crate: source.crate.dir,
            module,
            name,
            kind: host ? 'field' : 'static',
            file: source.rel,
            line,
            type,
            holding: holds,
        })
    }
    for (const source of sources) {
        const { code, lines, starts } = source
        for (const match of code.matchAll(/^[ \t]*(?:pub(?:\([^)]*\))?\s+)?static\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:/gm)) {
            const line = lineOf(starts, match.index)
            const column = match.index + match[0].length - starts[line - 1]
            push(source, match[1], null, line, typeText(lines, line, column, /=|;/))
        }
        for (const match of code.matchAll(/\bstruct\s+([A-Z]\w*)[^;{(]*\{/g)) {
            const open = match.index + match[0].length - 1
            const close = matchingBrace(code, open)
            let depth = 0
            for (let i = open + 1; i < close; i++) {
                const ch = code[i]
                if ('{([<'.includes(ch)) depth++
                else if ('})]>'.includes(ch) && code[i - 1] !== '-') depth--
                else if (depth === 0 && /[a-z_]/.test(ch) && /[\s,{]/.test(code[i - 1])) {
                    const field = code.slice(i, close).match(/^(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:(?!:)/)
                    if (!field) continue
                    const start = i + field[0].length
                    let end = start
                    let inner = 0
                    for (; end < close; end++) {
                        if ('{([<'.includes(code[end])) inner++
                        else if ('})]>'.includes(code[end]) && code[end - 1] !== '-') inner--
                        else if (code[end] === ',' && inner === 0) break
                    }
                    push(source, field[1], match[1], lineOf(starts, i), code.slice(start, end).replace(/\s+/g, ' ').trim())
                    i = end
                }
            }
        }
    }
    return items.sort(compareItems)
}

/**
 * Struct and enum names reachable from long-lived roots: every `static`, every
 * field of `theseus::state::State`, Tauri-managed state (`.manage(..)`) and the
 * policy's extra roots. Fields of unreachable hosts are per-operation (rule D2).
 */
export function longLivedTypes(sources, extraRoots = []) {
    const edges = new Map()
    const roots = new Set(extraRoots)
    const mention = (text, into) => {
        for (const word of text.matchAll(/\b[A-Z]\w*/g)) into.add(word[0])
    }
    for (const source of sources) {
        const { code } = source
        for (const match of code.matchAll(/\b(struct|enum)\s+([A-Z]\w*)[^;{(]*([{(])/g)) {
            const open = match.index + match[0].length - 1
            const close = match[3] === '{' ? matchingBrace(code, open) : code.indexOf(';', open)
            const body = code.slice(open + 1, close)
            const targets = edges.get(match[2]) ?? new Set()
            mention(body, targets)
            edges.set(match[2], targets)
            if (match[2] === 'State' && source.rel === 'packages/app-lib/src/state/mod.rs') mention(body, roots)
        }
        for (const match of code.matchAll(/^[ \t]*(?:pub(?:\([^)]*\))?\s+)?static\s+(?:mut\s+)?[A-Z_][A-Z0-9_]*\s*:([^=;]*)/gm)) mention(match[1], roots)
        for (const match of code.matchAll(/\.manage\(\s*([^;]*?)\)\s*;?/g)) mention(match[1], roots)
    }
    const reachable = new Set()
    const queue = [...roots]
    while (queue.length > 0) {
        const name = queue.pop()
        if (reachable.has(name)) continue
        reachable.add(name)
        for (const next of edges.get(name) ?? []) queue.push(next)
    }
    return reachable
}

/**
 * Construction sites of a struct within one crate: `Name { .. }` literals and
 * `Self { .. }` inside its impl blocks, each flagged by whether it sits in a
 * function body. Rule D2 needs every site in a function body (spec §11.7 J2).
 */
export function constructionSites(sources, crateDir, name) {
    const sites = []
    for (const source of sources.filter((candidate) => candidate.crate.dir === crateDir)) {
        const { code, starts } = source
        const bodies = functionBodies(source)
        const inFunction = (offset) => {
            const line = lineOf(starts, offset)
            return bodies.some((body) => body.from < line && line <= body.to) || bodies.some((body) => body.from === line && code.lastIndexOf('{', offset - 1) > code.lastIndexOf('fn', offset))
        }
        const push = (offset) => sites.push({ file: source.rel, line: lineOf(starts, offset), inFunction: inFunction(offset) })
        const literal = new RegExp(`(?<![\\w:])(?:\\w+::)*${name}\\s*\\{`, 'g')
        for (const match of code.matchAll(literal)) {
            const before = code.slice(Math.max(0, match.index - 40), match.index)
            if (/\b(struct|enum|impl|for|union)\s+$|\bimpl\s*<[^>]*>\s*$|->\s*$/.test(before)) continue
            push(match.index)
        }
        for (const block of code.matchAll(new RegExp(`\\bimpl\\b[^{;]*?\\b${name}\\b(?:<[^{;]*>)?\\s*(?:where[^{]*)?\\{`, 'g'))) {
            const open = block.index + block[0].length - 1
            const close = matchingBrace(code, open)
            for (const match of code.slice(open, close).matchAll(/\bSelf\s*\{/g)) push(open + match.index)
        }
    }
    return sites
}

export function compareItems(a, b) {
    return a.ident.localeCompare(b.ident) || a.file.localeCompare(b.file) || a.line - b.line
}

/** Calls that hand back the same resource, so `x.clone().acquire_owned()` acquires `x`. */
const TRANSPARENT_CALLS = new Set(['clone', 'as_ref', 'deref', 'borrow'])

/**
 * Reads the receiver chain that ends right before a method call: the last
 * plain segment names the resource; a leading function call names nothing.
 */
function resolveReceiver(before) {
    const segments = []
    let rest = before.trimEnd()
    for (;;) {
        const segment = rest.match(/([A-Za-z_]\w*)\s*(\(\s*\))?\s*$/)
        if (!segment) break
        segments.unshift({ name: segment[1], call: Boolean(segment[2]) })
        rest = rest.slice(0, segment.index).trimEnd()
        if (!rest.endsWith('.') || rest.endsWith('..')) break
        rest = rest.slice(0, -1).trimEnd()
    }
    while (segments.length > 1 && segments.at(-1).call && TRANSPARENT_CALLS.has(segments.at(-1).name)) segments.pop()
    const last = segments.at(-1)
    if (!last || last.call) return { receiver: null, via: 'call' }
    if (/^[A-Z][A-Z0-9_]*$/.test(last.name)) return { receiver: last.name, via: 'static' }
    return { receiver: last.name, via: segments.length > 1 ? 'field' : 'binding' }
}

/** Method-name hits (search ①), with the receiver identifier where it is a static or a field access. */
export const METHOD_PATTERN =
    /\.(acquire|acquire_many|try_acquire|acquire_owned|acquire_many_owned|try_acquire_owned|lock|read|write|lock_owned|read_owned|write_owned|try_lock_owned|try_read_owned|try_write_owned)\(|\b(const_new|Semaphore::new)\(/g

export function scanMethodHits(sources) {
    const hits = []
    for (const source of sources) {
        const { code, starts } = source
        for (const match of code.matchAll(METHOD_PATTERN)) {
            const method = match[1] ?? match[2]
            const after = code.slice(match.index + match[0].length).match(/^\s*\)/)
            // `read`/`write`/`lock` with arguments are I/O calls, not lock guards.
            if (['lock', 'read', 'write'].includes(method) && !after) continue
            const line = lineOf(starts, match.index)
            const before = code.slice(Math.max(0, match.index - 160), match.index)
            let receiver = null
            let via = null
            if (match[1]) {
                ;({ receiver, via } = resolveReceiver(before))
            } else {
                // `static X: T = Semaphore::new(..)`, `field: Newtype(Semaphore::new(..))` or `let field = ..`.
                const target = before.match(
                    /(?:static\s+([A-Z_][A-Z0-9_]*)\s*:[^;]*|\blet\s+(?:mut\s+)?([a-z_]\w*)\s*(?::[^=;]*)?=|\b([a-z_][a-z0-9_]*)\s*(?<!:):(?!:))\s*(?:Arc::new\(\s*)?(?:[A-Z]\w*\(\s*)?(?:[\w:]+::)?$/,
                )
                if (target) [receiver, via] = [target[1] ?? target[2] ?? target[3], target[1] ? 'static' : target[2] ? 'binding' : 'field']
            }
            const semaphore = method.startsWith('acquire') || method.startsWith('try_acquire') || /Semaphore\s*::\s*$|Semaphore::new/.test(before + match[0])
            hits.push({ crate: source.crate.dir, file: source.rel, line, method, receiver, via, semaphore })
        }
    }
    return hits
}

/**
 * Declaration side of the code-side set `C` (search ②), using the line-based
 * patterns of the design literally so it stays independent of `scanDeclarations`.
 */
export function scanDeclarationLines(sources) {
    const staticLine = /^\s*(pub(\(crate\))? )?static ([A-Z_]+):/
    const fieldLine = /^\s*(pub(\(crate\))? )?([a-z_]+): (Arc<)?(tokio::sync::)?(Async)?(Mutex|RwLock|Semaphore|DashMap|DashSet)/
    const lockWord = wordPattern(PRIMITIVES)
    const found = []
    for (const source of sources) {
        source.lines.forEach((text, index) => {
            const stat = text.match(staticLine)
            if (stat) {
                const joined = source.lines.slice(index, index + 4).join(' ')
                if (lockWord.test(joined.split('=')[0])) found.push({ crate: source.crate.dir, file: source.rel, line: index + 1, name: stat[3], via: 'static-decl' })
            }
            const field = text.match(fieldLine)
            if (field) found.push({ crate: source.crate.dir, file: source.rel, line: index + 1, name: field[3], via: 'field-decl' })
        })
    }
    return found
}

/** Bare name of an inventory item: `theseus::state::State.fetch_semaphore` → `fetch_semaphore`. */
export function bareName(item) {
    return item.name.split('.').at(-1)
}

/** Assigns S1–S10 from the policy, falling back to a lexical guess that the policy can override. */
export function classify(item, policy) {
    const curated = policy.classes[item.ident]
    if (curated) return { scopeClass: curated, classSource: 'policy' }
    const name = bareName(item).toLowerCase()
    const semaphore = /semaphore|budget|permit/i.test(item.type) && !/dashmap|hashmap/i.test(item.type)
    let scopeClass = 'S8'
    if (semaphore) scopeClass = /db|job|permit|quota/.test(name) ? 'S5' : 'S9'
    else if (/budget/.test(name)) scopeClass = 'S9'
    else if (/cancel|request|task|flight|operation|session|starting/.test(name)) scopeClass = 'S6'
    else if (/event|status|snapshot|state/.test(name)) scopeClass = 'S10'
    else if (/^(Async)?(Mutex|RwLock)<\s*\(\s*\)\s*>$|<\s*(Async)?Mutex<\s*\(\s*\)\s*>\s*>$/.test(item.type.replace(/\w+::/g, ''))) scopeClass = 'S7'
    return { scopeClass, classSource: 'lexical' }
}

/** Rank key `(scope_class, crate, module_path, identifier)`; class precedence comes from the policy. */
export function assignRanks(registered, policy) {
    const order = policy.classOrder
    const sorted = [...registered].sort(
        (a, b) =>
            order.indexOf(a.scopeClass) - order.indexOf(b.scopeClass) ||
            a.crate.localeCompare(b.crate) ||
            a.module.localeCompare(b.module) ||
            a.name.localeCompare(b.name),
    )
    sorted.forEach((item, index) => {
        item.rank = index + 1
    })
    return sorted
}

/** Fingerprint of the ident → rank mapping; annotations bind to idents, so any rank move changes it. */
export function rankFingerprint(registry) {
    return sha256(registry.map((item) => `${item.ident}\t${item.rank}\n`).join(''))
}

/** Ranges of `fn` bodies in masked code, innermost last, for grouping acquisitions by function. */
export function functionBodies(source) {
    const bodies = []
    for (const match of source.code.matchAll(/\bfn\s+([A-Za-z_]\w*)[^;{]*\{/g)) {
        const open = match.index + match[0].length - 1
        bodies.push({ name: match[1], from: lineOf(source.starts, open), to: lineOf(source.starts, matchingBrace(source.code, open)) })
    }
    return bodies
}

/** `// sched-order: <module_path::IDENT>` annotations on acquisition points (design v8 §4.9). */
export function scanOrderAnnotations(sources) {
    const annotations = []
    for (const source of sources) {
        const bodies = functionBodies(source)
        source.rawLines.forEach((text, index) => {
            const match = text.match(/\/\/\s*sched-order:\s*(\S+)/)
            if (!match) return
            const line = index + 1
            const body = bodies.filter((candidate) => candidate.from <= line && line <= candidate.to).at(-1)
            annotations.push({ file: source.rel, line, ident: match[1], fn: body ? `${body.name}@${body.from}` : null })
        })
    }
    return annotations
}

/**
 * Checks annotations against the registry: each ident must be registered, and
 * within one function the annotated acquisitions must ascend in rank.
 */
export function checkAnnotations(annotations, registry) {
    const ranks = new Map(registry.items.map((item) => [item.ident, item.rank]))
    const unreachable = annotations.filter((annotation) => !ranks.has(annotation.ident))
    const descending = []
    const byFunction = new Map()
    for (const annotation of annotations.filter((candidate) => ranks.has(candidate.ident) && candidate.fn)) {
        const key = `${annotation.file}\0${annotation.fn}`
        if (!byFunction.has(key)) byFunction.set(key, [])
        byFunction.get(key).push(annotation)
    }
    for (const sequence of byFunction.values()) {
        for (let i = 1; i < sequence.length; i++) {
            if (ranks.get(sequence[i].ident) <= ranks.get(sequence[i - 1].ident)) descending.push({ held: sequence[i - 1], acquired: sequence[i] })
        }
    }
    return { unreachable, descending }
}

export function parseArgs(argv) {
    const args = {}
    for (let i = 0; i < argv.length; i++) {
        const key = argv[i]
        if (key.startsWith('--')) args[key.slice(2)] = argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : true
    }
    return args
}

export function outputPaths(outDir = defaultOutDir) {
    return {
        inventory: `${outDir}/sched-resource-inventory.json`,
        registry: `${outDir}/sched-resource-registry.json`,
        exclusions: `${outDir}/sched-resource-exclusions.json`,
        getpoints: `${outDir}/sched-resource-getpoints.json`,
        recallGap: `${outDir}/sched-resource-recall-gap.json`,
    }
}
