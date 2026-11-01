import { text } from './messages.js'

// DOM helpers. Text always goes through text nodes; nothing here parses HTML.
export const element = (id) => document.getElementById(id)

export function h(tag, props = {}, children = []) {
    const result = document.createElement(tag)
    for (const [key, value] of Object.entries(props)) {
        if (value === undefined || value === null || value === false) continue
        if (key === 'class') result.className = value
        else if (key === 'text') result.textContent = value
        else if (key === 'data') Object.assign(result.dataset, value)
        else if (key === 'label') {
            result.setAttribute('aria-label', value)
            result.dataset.tip = value
        } else if (key === 'hidden') result.hidden = true
        else result.setAttribute(key, value === true ? '' : value)
    }
    append(result, children)
    return result
}

export function append(parent, children) {
    for (const child of Array.isArray(children) ? children : [children]) {
        if (child === null || child === undefined || child === false) continue
        parent.append(typeof child === 'string' ? document.createTextNode(child) : child)
    }
    return parent
}

export function icon(name, className = '') {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
    svg.setAttribute('aria-hidden', 'true')
    if (className) svg.setAttribute('class', className)
    const use = document.createElementNS('http://www.w3.org/2000/svg', 'use')
    use.setAttribute('href', `#i-${name}`)
    svg.append(use)
    return svg
}

export function button(props = {}, children = []) {
    return h('button', { type: 'button', ...props }, children)
}

// Splits agent Markdown into blocks: paragraphs, headings, lists, quotes,
// rules and fenced code. An unclosed fence (the answer is still streaming) runs
// to the end. Each block keeps its source so a re-render can reuse it.
export function markdownBlocks(source) {
    return parseMarkdown(source).blocks
}

// The same blocks, parsed again only where the text could have changed them.
// Streamed text only ever grows at the end, and a block depends on nothing but
// the lines it read: its own, and the line that ended it. Blocks that never
// read the last line of `before` are the same in the longer text, so parsing
// starts again at the first block that did. `kept` counts the blocks reused.
export function parseMarkdown(source, before = null) {
    if (!before || !source.startsWith(before.source))
        return { source, ...parseFrom(source, 0), kept: 0 }
    const end = before.source.length
    const blocks = before.blocks.slice()
    const reads = before.reads.slice()
    // Where each block stops reading only grows, so the blocks that read the
    // last line are the ones at the end.
    let kept = blocks.length
    while (kept && reads[kept - 1].to >= end) kept--
    const from = kept < blocks.length ? reads[kept].from : before.source.lastIndexOf('\n') + 1
    const tail = parseFrom(source.slice(from), from)
    blocks.length = reads.length = kept
    blocks.push(...tail.blocks)
    reads.push(...tail.reads)
    return { source, blocks, reads, kept }
}

// Parses `source`, which starts at `offset` in the whole text. Each block's
// read is where its first line starts and where the last line it read ends.
function parseFrom(source, offset) {
    const lines = source.split('\n')
    const starts = []
    for (let line = 0, at = offset; line < lines.length; at += lines[line++].length + 1)
        starts.push(at)
    const blocks = []
    const reads = []
    let index = 0
    const take = (test) => {
        const taken = []
        while (index < lines.length && test(lines[index])) taken.push(lines[index++])
        return taken
    }
    const add = (block, first, last) => {
        blocks.push(block)
        reads.push({ from: starts[first], to: starts[last] + lines[last].length })
    }
    // A run that stops at a line has read it; one that ran out read the last.
    const stopped = () => Math.min(index, lines.length - 1)
    while (index < lines.length) {
        const first = index
        const line = lines[index]
        if (!line.trim()) {
            index++
            continue
        }
        const fence = line.match(/^\s*```\s*([\w+-]*)/)
        if (fence) {
            index++
            const code = take((entry) => !/^\s*```/.test(entry))
            const last = stopped()
            const closed = index < lines.length
            if (closed) index++
            add(
                {
                    type: 'code',
                    lang: fence[1],
                    text: code.join('\n'),
                    source: `${line}\n${code.join('\n')}${closed ? '\n```' : ''}`,
                },
                first,
                last,
            )
            continue
        }
        const heading = line.match(/^(#{1,3})\s+(.*)$/)
        if (heading) {
            index++
            add(
                {
                    type: 'heading',
                    level: heading[1].length,
                    text: heading[2],
                    source: line,
                },
                first,
                first,
            )
            continue
        }
        if (/^\s*(-{3,}|\*{3,})\s*$/.test(line)) {
            index++
            add({ type: 'rule', source: line }, first, first)
            continue
        }
        if (/^\s*>/.test(line)) {
            const quote = take((entry) => /^\s*>/.test(entry))
            add(
                {
                    type: 'quote',
                    text: quote.map((entry) => entry.replace(/^\s*>\s?/, '')).join('\n'),
                    source: quote.join('\n'),
                },
                first,
                stopped(),
            )
            continue
        }
        const bullet = /^\s*[-*]\s+/
        const number = /^\s*\d+[.)]\s+/
        if (bullet.test(line) || number.test(line)) {
            const ordered = number.test(line)
            const marker = ordered ? number : bullet
            const items = take((entry) => marker.test(entry))
            add(
                {
                    type: ordered ? 'ol' : 'ul',
                    items: items.map((entry) => entry.replace(marker, '')),
                    start: ordered ? Number.parseInt(line, 10) || 1 : 1,
                    source: items.join('\n'),
                },
                first,
                stopped(),
            )
            continue
        }
        const paragraph = take(
            (entry) =>
                entry.trim() &&
                !/^\s*```/.test(entry) &&
                !/^#{1,3}\s/.test(entry) &&
                !/^\s*>/.test(entry) &&
                !bullet.test(entry) &&
                !number.test(entry),
        )
        add(
            { type: 'p', text: paragraph.join('\n'), source: paragraph.join('\n') },
            first,
            stopped(),
        )
    }
    return { blocks, reads }
}

// Logs read faster with their levels told apart: errors and exceptions red,
// warnings amber, stack frames dimmed. Every line stays a plain text node.
const LOG_LANGS = new Set(['log', 'logs', 'crash', 'stacktrace'])
const looksLikeLog = (source) => /^\s+at [\w$.]+\(/m.test(source) && /Exception|Error/.test(source)
const isLog = (block) => LOG_LANGS.has(block.lang) || (!block.lang && looksLikeLog(block.text))

export function logLines(source) {
    const lines = source.split('\n')
    return lines.map((line, index) => {
        const tone = /^\s+at |^\s*\.\.\. \d+ more/.test(line)
            ? 'trace'
            : /\b(ERROR|FATAL|SEVERE)\b|\w*(Exception|Error)\b|^Caused by:/.test(line)
              ? 'error'
              : /\bWARN(ING)?\b/.test(line)
                ? 'warn'
                : ''
        return h('span', {
            class: tone ? `log-line is-${tone}` : 'log-line',
            text: index < lines.length - 1 ? `${line}\n` : line,
        })
    })
}

function renderBlock(block) {
    let node
    switch (block.type) {
        case 'code':
            node = h('div', { class: 'code-block' }, [
                h('div', { class: 'code-head' }, [
                    h('span', { text: block.lang || 'text' }),
                    button(
                        { class: 'code-copy', label: text('copyCode'), data: { copyCode: '' } },
                        [icon('copy')],
                    ),
                ]),
                h('pre', {}, [
                    isLog(block)
                        ? h('code', { class: 'is-log' }, logLines(block.text))
                        : h('code', { text: block.text }),
                ]),
            ])
            break
        case 'heading':
            node = h(`h${block.level + 2}`, {}, inline(block.text))
            break
        case 'rule':
            node = h('hr')
            break
        case 'quote':
            node = h('blockquote', {}, inline(block.text))
            break
        case 'ul':
        case 'ol':
            node = h(
                block.type,
                block.type === 'ol' && block.start !== 1 ? { start: block.start } : {},
                block.items.map((item) => h('li', {}, inline(item))),
            )
            break
        default:
            node = h('p', {}, inline(block.text))
    }
    node.dataset.source = block.source
    return node
}

// What each container shows, so the next render of a streamed answer parses
// and compares only its end.
const rendered = new WeakMap()

// Renders blocks into a container, keeping every leading block whose source is
// unchanged. While an answer streams only its last block changes, and it is
// brought up to date in place: the result is the same DOM a fresh render
// builds, without rebuilding every line of a long log on each delta.
export function renderMarkdown(container, source) {
    let before = rendered.get(container)
    if (before && container.childElementCount !== before.blocks.length) before = null
    const parsed = parseMarkdown(source, before)
    const { blocks } = parsed
    const nodes = container.children
    let same = parsed.kept
    while (
        same < blocks.length &&
        same < nodes.length &&
        nodes[same].dataset.source === blocks[same].source
    )
        same++
    if (
        before &&
        same < blocks.length &&
        same < nodes.length &&
        updateBlock(nodes[same], before.blocks[same], blocks[same])
    )
        same++
    while (nodes.length > same) container.lastElementChild.remove()
    container.append(...blocks.slice(same).map(renderBlock))
    rendered.set(container, parsed)
}

// Updates a block's node to what renderBlock(block) would build, changing only
// what differs from `before`, the block it shows. Lines of code and list items
// that are the same stay and text nodes take their new text, so neither the
// script nor the browser rebuilds and restyles a long log on every delta.
// False when the block has to be built again.
function updateBlock(node, before, block) {
    if (before.type !== block.type) return false
    switch (block.type) {
        case 'code': {
            if (before.lang !== block.lang || isLog(before) !== isLog(block)) return false
            const code = node.querySelector('pre > code')
            if (!isLog(block)) {
                setText(code, block.text)
                break
            }
            // Every line but the last one shown is unchanged when text was added.
            const grown = block.text.startsWith(before.text)
            const kept = grown ? Math.max(0, code.childElementCount - 1) : 0
            const from = grown ? before.text.lastIndexOf('\n') + 1 : 0
            patchFrom(code, kept, logLines(block.text.slice(from)))
            break
        }
        case 'ul':
        case 'ol': {
            if (before.start !== block.start) return false
            let kept = 0
            while (
                kept < before.items.length &&
                kept < block.items.length &&
                before.items[kept] === block.items[kept]
            )
                kept++
            patchFrom(
                node,
                kept,
                block.items.slice(kept).map((item) => h('li', {}, inline(item))),
            )
            break
        }
        case 'rule':
            break
        case 'heading':
            if (before.level !== block.level) return false
            patchChildren(node, inline(block.text))
            break
        default:
            patchChildren(node, inline(block.text))
    }
    node.dataset.source = block.source
    return true
}

function setData(node, value) {
    if (node.data !== value) node.data = value
}

// The `text` prop of h(): one text node, or none for an empty string.
function setText(parent, value) {
    const only = parent.childNodes.length === 1 ? parent.firstChild : null
    if (value && only?.nodeType === Node.TEXT_NODE) setData(only, value)
    else parent.textContent = value
}

// Makes the element children of `parent` from index `from` on match `fresh`.
function patchFrom(parent, from, fresh) {
    fresh.forEach((node, index) => {
        const current = parent.children[from + index]
        if (current) morph(current, node)
        else parent.append(node)
    })
    while (parent.childElementCount > from + fresh.length) parent.lastElementChild.remove()
}

// Makes the child nodes of `parent` match `parts`, strings and elements as
// inline() returns them.
function patchChildren(parent, parts) {
    parts.forEach((part, index) => {
        const current = parent.childNodes[index]
        if (!current) append(parent, part)
        else if (typeof part === 'string') {
            if (current.nodeType === Node.TEXT_NODE) setData(current, part)
            else current.replaceWith(part)
        } else if (current.nodeType === Node.TEXT_NODE) current.replaceWith(part)
        else morph(current, part)
    })
    while (parent.childNodes.length > parts.length) parent.lastChild.remove()
}

// Brings an element in line with a freshly built twin. It keeps the element
// only when the attributes have the same names in the same order, so the
// markup comes out exactly as the twin's would.
function morph(current, fresh) {
    if (current.isEqualNode(fresh)) return
    const had = current.attributes
    const wants = fresh.attributes
    if (
        current.namespaceURI !== fresh.namespaceURI ||
        current.localName !== fresh.localName ||
        had.length !== wants.length ||
        [...wants].some((attribute, index) => had[index].name !== attribute.name)
    ) {
        current.replaceWith(fresh)
        return
    }
    for (let index = 0; index < wants.length; index++)
        if (had[index].value !== wants[index].value) had[index].value = wants[index].value
    patchChildren(
        current,
        [...fresh.childNodes].map((child) =>
            child.nodeType === Node.TEXT_NODE ? child.data : child,
        ),
    )
}

// Inline marks: `code`, **bold**, *italic* and [links](url). Links are shown,
// not followed: the window never navigates, so they copy their address.
function inline(source) {
    const parts = []
    const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*\s][^*]*\*|\[[^\]]+\]\([^)\s]+\))/g
    let index = 0
    for (const match of source.matchAll(pattern)) {
        if (match.index > index) parts.push(source.slice(index, match.index))
        const token = match[0]
        if (token.startsWith('`')) parts.push(h('code', { text: token.slice(1, -1) }))
        else if (token.startsWith('**')) parts.push(h('strong', { text: token.slice(2, -2) }))
        else if (token.startsWith('*')) parts.push(h('em', { text: token.slice(1, -1) }))
        else {
            const [, label, url] = token.match(/^\[([^\]]+)\]\(([^)\s]+)\)$/)
            parts.push(
                button({ class: 'link', title: url, data: { copyLink: url } }, [
                    h('span', { text: label }),
                    icon('copy'),
                ]),
            )
        }
        index = match.index + token.length
    }
    if (index < source.length) parts.push(source.slice(index))
    return parts
}

// The words of an answer without its code blocks and Markdown marks, for a
// screen reader announcement.
export function plainText(source) {
    return markdownBlocks(source)
        .filter((block) => block.type !== 'code' && block.type !== 'rule')
        .map((block) => (block.items ? block.items.join('\n') : block.text))
        .join('\n')
        .replace(/\[([^\]]+)\]\([^)\s]+\)/g, '$1')
        .replace(/\*\*([^*]+)\*\*|\*([^*\s][^*]*)\*|`([^`]+)`/g, '$1$2$3')
        .trim()
}

// A stable hue for a mod glyph, so the same mod keeps its colour everywhere.
export function hue(name) {
    let value = 0
    for (const character of name) value = (value * 31 + character.codePointAt(0)) % 360
    return value
}

// ---------- focus ----------

// Views redraw by replacing nodes, and a control that is removed, hidden or
// disabled while focused drops the focus to the page. These helpers send it on
// instead: to the redrawn twin of that control (same identifying attributes),
// else to the first control left in its step group, turn, list or popover,
// else to a fallback.
const IDENTITY = [
    'data-thread',
    'data-thread-menu',
    'data-jump',
    'data-mark-turn',
    'data-copy-turn',
    'data-edit-turn',
    'data-toggle-change',
    'data-file-preview',
    'data-remove-attachment',
    'data-remove-queued',
    'data-send-queued',
    // Named by its request, which can repeat across turns ("Why change
    // Sodium?"); the twin is looked for in the nearest scope first.
    'data-followup',
    'data-turn',
    'data-prompt',
    'data-detail',
    'data-model',
    'data-instance',
]
// These narrow an identity down but do not name a control on their own: the
// same action appears in every step group.
const QUALIFIERS = ['data-action', 'data-copy-kind', 'data-provider']
const SCOPES = '.activity, .turn, .popover, #conversation-list, #attachment-list, #queue-list'
const TABBABLE = 'button, input, textarea, select, [tabindex]'

function keyOf(node) {
    if (node.id) return `#${CSS.escape(node.id)}`
    if (!IDENTITY.some((name) => node.hasAttribute(name))) return null
    const parts = [...IDENTITY, ...QUALIFIERS]
        .filter((name) => node.hasAttribute(name))
        .map((name) => `[${name}="${CSS.escape(node.getAttribute(name))}"]`)
    return node.localName + parts.join('')
}

export function focusable(node) {
    return !!node?.isConnected && !node.disabled && !node.closest('[hidden], [inert]')
}

function firstTabbable(scope) {
    return [...scope.querySelectorAll(TABBABLE)].find(
        (node) => node.tabIndex >= 0 && focusable(node) && node.getClientRects().length,
    )
}

// Where the focus is, in a form that survives a redraw.
export function focusPlace(node = document.activeElement) {
    if (!(node instanceof HTMLElement) || node === document.body) return null
    const scopes = []
    let scope = node.parentElement?.closest(SCOPES)
    while (scope) {
        scopes.push(scope)
        scope = scope.parentElement?.closest(SCOPES)
    }
    return { node, key: keyOf(node), scopes }
}

// The redrawn twin, looked for in the nearest scope that is still there before
// the whole page, so a control repeated in several turns finds its own.
function twinOf(place) {
    for (const scope of [...place.scopes.filter((scope) => scope.isConnected), document]) {
        const twin = [...scope.querySelectorAll(place.key)].find(focusable)
        if (twin) return twin
    }
    return null
}

export function restoreFocus(place, fallback) {
    const twin = place?.key && twinOf(place)
    const target =
        (focusable(place?.node) && place.node) ||
        twin ||
        place?.scopes.map((scope) => scope.isConnected && firstTabbable(scope)).find(Boolean) ||
        fallback
    target?.focus()
}

// Watches for the focused control going away. Mutations are seen in the same
// task, before the page paints, so the focus never visibly drops.
export function guardFocus(fallback) {
    let place = null
    document.addEventListener('focusin', (event) => {
        place = focusPlace(event.target)
    })
    // Clicking empty space moves the focus to the page on purpose.
    document.addEventListener('focusout', (event) => {
        if (event.relatedTarget) return
        queueMicrotask(() => {
            if (document.activeElement === document.body && focusable(event.target)) place = null
        })
    })
    new MutationObserver(() => {
        if (!place) return
        const active = document.activeElement
        if ((active !== place.node && active !== document.body) || focusable(place.node)) return
        restoreFocus(place, fallback())
    }).observe(document.body, {
        subtree: true,
        childList: true,
        attributes: true,
        attributeFilter: ['hidden', 'inert', 'disabled'],
    })
}
