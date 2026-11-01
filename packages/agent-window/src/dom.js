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
            result.title = value
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

// A small spinner drawn as an arc; CSS rotates it.
export function spinner(className = '') {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
    svg.setAttribute('class', `spinner ${className}`.trim())
    svg.setAttribute('viewBox', '0 0 24 24')
    svg.setAttribute('aria-hidden', 'true')
    const track = document.createElementNS('http://www.w3.org/2000/svg', 'circle')
    track.setAttribute('class', 'spinner-track')
    const arc = document.createElementNS('http://www.w3.org/2000/svg', 'circle')
    arc.setAttribute('class', 'spinner-arc')
    for (const circle of [track, arc]) {
        circle.setAttribute('cx', '12')
        circle.setAttribute('cy', '12')
        circle.setAttribute('r', '9')
    }
    svg.append(track, arc)
    return svg
}

// Renders the small Markdown subset agent answers use: paragraphs, "-" lists,
// **bold** and `code`.
export function markdown(source) {
    const fragment = document.createDocumentFragment()
    const blocks = source.split(/\n{2,}/)
    for (const block of blocks) {
        const lines = block.split('\n').filter((line) => line.trim())
        if (!lines.length) continue
        if (lines.every((line) => /^\s*[-*] /.test(line))) {
            fragment.append(
                h(
                    'ul',
                    {},
                    lines.map((line) => h('li', {}, inline(line.replace(/^\s*[-*] /, '')))),
                ),
            )
        } else fragment.append(h('p', {}, inline(lines.join('\n'))))
    }
    return fragment
}

function inline(source) {
    const parts = []
    const pattern = /(\*\*[^*]+\*\*|`[^`]+`)/g
    let index = 0
    for (const match of source.matchAll(pattern)) {
        if (match.index > index) parts.push(source.slice(index, match.index))
        const token = match[0]
        parts.push(
            token.startsWith('**')
                ? h('strong', { text: token.slice(2, -2) })
                : h('code', { text: token.slice(1, -1) }),
        )
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
