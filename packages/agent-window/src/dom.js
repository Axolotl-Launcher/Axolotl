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
    const lines = source.split('\n')
    const blocks = []
    let index = 0
    const take = (test) => {
        const taken = []
        while (index < lines.length && test(lines[index])) taken.push(lines[index++])
        return taken
    }
    while (index < lines.length) {
        const line = lines[index]
        if (!line.trim()) {
            index++
            continue
        }
        const fence = line.match(/^\s*```\s*([\w+-]*)/)
        if (fence) {
            index++
            const code = take((entry) => !/^\s*```/.test(entry))
            const closed = index < lines.length
            if (closed) index++
            blocks.push({
                type: 'code',
                lang: fence[1],
                text: code.join('\n'),
                source: `${line}\n${code.join('\n')}${closed ? '\n```' : ''}`,
            })
            continue
        }
        const heading = line.match(/^(#{1,3})\s+(.*)$/)
        if (heading) {
            index++
            blocks.push({
                type: 'heading',
                level: heading[1].length,
                text: heading[2],
                source: line,
            })
            continue
        }
        if (/^\s*(-{3,}|\*{3,})\s*$/.test(line)) {
            index++
            blocks.push({ type: 'rule', source: line })
            continue
        }
        if (/^\s*>/.test(line)) {
            const quote = take((entry) => /^\s*>/.test(entry))
            blocks.push({
                type: 'quote',
                text: quote.map((entry) => entry.replace(/^\s*>\s?/, '')).join('\n'),
                source: quote.join('\n'),
            })
            continue
        }
        const bullet = /^\s*[-*]\s+/
        const number = /^\s*\d+[.)]\s+/
        if (bullet.test(line) || number.test(line)) {
            const ordered = number.test(line)
            const marker = ordered ? number : bullet
            const items = take((entry) => marker.test(entry))
            blocks.push({
                type: ordered ? 'ol' : 'ul',
                items: items.map((entry) => entry.replace(marker, '')),
                start: ordered ? Number.parseInt(line, 10) || 1 : 1,
                source: items.join('\n'),
            })
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
        blocks.push({ type: 'p', text: paragraph.join('\n'), source: paragraph.join('\n') })
    }
    return blocks
}

export function renderBlock(block) {
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
                h('pre', {}, [h('code', { text: block.text })]),
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

// Renders blocks into a container, keeping every leading block whose source is
// unchanged. While an answer streams only its last block is rebuilt.
export function renderMarkdown(container, source) {
    const blocks = markdownBlocks(source)
    const nodes = [...container.children]
    let same = 0
    while (
        same < blocks.length &&
        same < nodes.length &&
        nodes[same].dataset.source === blocks[same].source
    )
        same++
    for (const node of nodes.slice(same)) node.remove()
    container.append(...blocks.slice(same).map(renderBlock))
}

export function markdown(source) {
    const fragment = document.createDocumentFragment()
    fragment.append(...markdownBlocks(source).map(renderBlock))
    return fragment
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

// A stable hue for a mod glyph, so the same mod keeps its colour everywhere.
export function hue(name) {
    let value = 0
    for (const character of name) value = (value * 31 + character.codePointAt(0)) % 360
    return value
}
