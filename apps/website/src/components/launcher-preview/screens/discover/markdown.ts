/**
 * Markdown rendering of project descriptions and changelogs, following the launcher's
 * `renderHighlightedString` (packages/utils): markdown-it's default preset with raw HTML,
 * protocol-only linkify and no soft-break conversion, `rel="noopener nofollow ugc"` on links to
 * other sites, then the `configuredXss` tag and attribute whitelist. Remote media is not
 * loaded inside the preview: images fall back to their alt text and embeds are left out.
 */

interface LinkReference {
    href: string
    title?: string
}

interface RenderContext {
    refs: Map<string, LinkReference>
}

const PUNCTUATION = /[!-/:-@[-`{-~\u2000-\u206f\u2e00-\u2e7f\\'!"#$%&()*+,\-./:;<=>?@[\]^_`{|}~]/

function escapeHtml(text: string) {
    return text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
}

function escapeText(text: string) {
    // Keep valid entity references, escape everything else.
    return text
        .replace(/&(?!#\d+;|#x[0-9a-fA-F]+;|[a-zA-Z][a-zA-Z0-9]*;)/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
}

function normalizeLabel(label: string) {
    return label.trim().replace(/\s+/g, ' ').toLowerCase()
}

function isBlank(line: string) {
    return /^[ \t]*$/.test(line)
}

function leadingSpaces(line: string) {
    let count = 0
    for (const char of line) {
        if (char === ' ') count++
        else if (char === '\t') count += 4 - (count % 4)
        else break
    }
    return count
}

function stripIndent(line: string, amount: number) {
    let removed = 0
    let index = 0
    while (removed < amount && index < line.length) {
        const char = line[index]
        if (char === ' ') removed++
        else if (char === '\t') removed += 4 - (removed % 4)
        else break
        index++
    }
    return line.slice(index)
}

// Block level ------------------------------------------------------------------------------------

const FENCE = /^( {0,3})(`{3,}|~{3,})\s*([^`]*)$/
const ATX = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/
const HR = /^ {0,3}((?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$/
const SETEXT = /^ {0,3}(=+|-+)[ \t]*$/
const QUOTE = /^ {0,3}> ?/
const BULLET = /^( {0,3})([*+-])([ \t]+|$)/
const ORDERED = /^( {0,3})(\d{1,9})([.)])([ \t]+|$)/
const REF_DEF =
    /^ {0,3}\[([^\]]+)\]:[ \t]*<?([^\s>]+)>?(?:[ \t]+(?:"([^"]*)"|'([^']*)'|\(([^)]*)\)))?[ \t]*$/
const TABLE_DELIMITER = /^ {0,3}\|?[ \t]*:?-+:?[ \t]*(\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/
const HTML_BLOCK_6 =
    /^ {0,3}<\/?(address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h1|h2|h3|h4|h5|h6|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(\s|\/?>|$)/i
const HTML_BLOCK_7 =
    /^ {0,3}(?:<[A-Za-z][A-Za-z0-9-]*(?:\s+[a-zA-Z_:][a-zA-Z0-9_.:-]*(?:\s*=\s*(?:[^"'=<>`\s]+|'[^']*'|"[^"]*"))?)*\s*\/?>|<\/[A-Za-z][A-Za-z0-9-]*\s*>)\s*$/

function htmlBlockKind(line: string, interruptsParagraph: boolean): number {
    if (/^ {0,3}<(script|pre|style|textarea)(\s|>|$)/i.test(line)) return 1
    if (/^ {0,3}<!--/.test(line)) return 2
    if (/^ {0,3}<\?/.test(line)) return 3
    if (/^ {0,3}<![A-Za-z]/.test(line)) return 4
    if (/^ {0,3}<!\[CDATA\[/.test(line)) return 5
    if (HTML_BLOCK_6.test(line)) return 6
    if (!interruptsParagraph && HTML_BLOCK_7.test(line)) return 7
    return 0
}

function htmlBlockEnds(kind: number, line: string) {
    switch (kind) {
        case 1:
            return /<\/(script|pre|style|textarea)>/i.test(line)
        case 2:
            return line.includes('-->')
        case 3:
            return line.includes('?>')
        case 4:
            return line.includes('>')
        case 5:
            return line.includes(']]>')
        default:
            return false
    }
}

function splitTableRow(line: string): string[] {
    let text = line.trim()
    if (text.startsWith('|')) text = text.slice(1)
    if (text.endsWith('|') && !text.endsWith('\\|')) text = text.slice(0, -1)
    const cells: string[] = []
    let current = ''
    let inCode = false
    for (let index = 0; index < text.length; index++) {
        const char = text[index]
        if (char === '\\' && text[index + 1] === '|') {
            current += '|'
            index++
            continue
        }
        if (char === '`') inCode = !inCode
        if (char === '|' && !inCode) {
            cells.push(current.trim())
            current = ''
            continue
        }
        current += char
    }
    cells.push(current.trim())
    return cells
}

function listMarker(line: string) {
    const bullet = BULLET.exec(line)
    if (bullet) {
        return {
            ordered: false,
            marker: bullet[2],
            start: 1,
            indent: bullet[1].length,
            width: 1,
            spacing: bullet[3],
            rest: line.slice(bullet[0].length),
        }
    }
    const ordered = ORDERED.exec(line)
    if (ordered) {
        return {
            ordered: true,
            marker: ordered[3],
            start: Number(ordered[2]),
            indent: ordered[1].length,
            width: ordered[2].length + 1,
            spacing: ordered[4],
            rest: line.slice(ordered[0].length),
        }
    }
    return null
}

function startsBlock(line: string) {
    return (
        FENCE.test(line) ||
        ATX.test(line) ||
        HR.test(line) ||
        QUOTE.test(line) ||
        !!listMarker(line) ||
        htmlBlockKind(line, true) > 0
    )
}

function renderBlocks(lines: string[], ctx: RenderContext, tight = false): string {
    const out: string[] = []
    let paragraph: string[] = []
    let index = 0

    function flushParagraph() {
        if (!paragraph.length) return
        const text = paragraph
            .map((line) => line.replace(/^[ \t]+/, ''))
            .join('\n')
            .replace(/[ \t]+$/, '')
        paragraph = []
        if (!text) return
        const html = renderInline(text, ctx)
        out.push(tight ? html : `<p>${html}</p>`)
    }

    while (index < lines.length) {
        const line = lines[index]

        if (isBlank(line)) {
            flushParagraph()
            index++
            continue
        }

        const indent = leadingSpaces(line)
        if (indent >= 4) {
            if (paragraph.length) {
                paragraph.push(line)
                index++
                continue
            }
            const code: string[] = []
            while (
                index < lines.length &&
                (leadingSpaces(lines[index]) >= 4 || isBlank(lines[index]))
            ) {
                code.push(stripIndent(lines[index], 4))
                index++
            }
            while (code.length && isBlank(code[code.length - 1])) code.pop()
            out.push(`<pre><code>${escapeHtml(code.join('\n'))}\n</code></pre>`)
            continue
        }

        const fence = FENCE.exec(line)
        if (fence) {
            flushParagraph()
            const fenceIndent = fence[1].length
            const fenceChars = fence[2]
            const info = fence[3].trim().split(/\s+/)[0] ?? ''
            const code: string[] = []
            index++
            while (index < lines.length) {
                const current = lines[index]
                const closing = new RegExp(
                    `^ {0,3}${fenceChars[0] === '`' ? '`' : '~'}{${fenceChars.length},}[ \\t]*$`,
                )
                if (closing.test(current)) {
                    index++
                    break
                }
                code.push(stripIndent(current, fenceIndent))
                index++
            }
            const language = info ? ` class="language-${escapeHtml(info)}"` : ''
            out.push(
                `<pre><code${language}>${escapeHtml(code.join('\n'))}${code.length ? '\n' : ''}</code></pre>`,
            )
            continue
        }

        const atx = ATX.exec(line)
        if (atx) {
            flushParagraph()
            const level = atx[1].length
            out.push(`<h${level}>${renderInline((atx[2] ?? '').trim(), ctx)}</h${level}>`)
            index++
            continue
        }

        if (paragraph.length && SETEXT.test(line)) {
            const level = line.trim().startsWith('=') ? 1 : 2
            const text = paragraph.map((entry) => entry.trim()).join('\n')
            paragraph = []
            out.push(`<h${level}>${renderInline(text, ctx)}</h${level}>`)
            index++
            continue
        }

        if (HR.test(line)) {
            flushParagraph()
            out.push('<hr>')
            index++
            continue
        }

        const htmlKind = htmlBlockKind(line, paragraph.length > 0)
        if (htmlKind) {
            flushParagraph()
            const block: string[] = []
            while (index < lines.length) {
                const current = lines[index]
                if (htmlKind >= 6 && isBlank(current)) break
                block.push(current)
                index++
                if (htmlKind < 6 && htmlBlockEnds(htmlKind, current)) break
            }
            out.push(block.join('\n'))
            continue
        }

        if (QUOTE.test(line)) {
            flushParagraph()
            const quoted: string[] = []
            while (index < lines.length) {
                const current = lines[index]
                if (QUOTE.test(current)) {
                    quoted.push(current.replace(QUOTE, ''))
                    index++
                    continue
                }
                if (
                    !isBlank(current) &&
                    quoted.length &&
                    !isBlank(quoted[quoted.length - 1]) &&
                    !startsBlock(current)
                ) {
                    quoted.push(current)
                    index++
                    continue
                }
                break
            }
            out.push(`<blockquote>\n${renderBlocks(quoted, ctx)}\n</blockquote>`)
            continue
        }

        const marker = listMarker(line)
        if (
            marker &&
            (!paragraph.length || ((!marker.ordered || marker.start === 1) && marker.rest.trim()))
        ) {
            flushParagraph()
            const items: string[][] = []
            let loose = false
            let previousEndedWithBlank = false
            while (index < lines.length) {
                const current = listMarker(lines[index])
                if (
                    !current ||
                    current.ordered !== marker.ordered ||
                    current.marker !== marker.marker
                )
                    break
                if (items.length && previousEndedWithBlank) loose = true
                const spacing = current.spacing.replace(/\t/g, '    ').length
                const contentIndent =
                    current.indent + current.width + (spacing >= 1 && spacing <= 4 ? spacing : 1)
                const itemLines: string[] = [
                    spacing > 4 ? `${' '.repeat(spacing - 1)}${current.rest}` : current.rest,
                ]
                index++
                while (index < lines.length) {
                    const next = lines[index]
                    if (isBlank(next)) {
                        itemLines.push('')
                        index++
                        continue
                    }
                    if (leadingSpaces(next) >= contentIndent) {
                        itemLines.push(stripIndent(next, contentIndent))
                        index++
                        continue
                    }
                    const lastLine = itemLines[itemLines.length - 1]
                    if (lastLine !== undefined && !isBlank(lastLine) && !startsBlock(next)) {
                        itemLines.push(next.trim())
                        index++
                        continue
                    }
                    break
                }
                previousEndedWithBlank = false
                while (itemLines.length > 1 && isBlank(itemLines[itemLines.length - 1])) {
                    itemLines.pop()
                    previousEndedWithBlank = true
                }
                const innerBlank = itemLines.findIndex(
                    (entry, position) => position > 0 && isBlank(entry),
                )
                if (innerBlank > 0 && innerBlank < itemLines.length - 1) loose = true
                items.push(itemLines)
            }
            const tag = marker.ordered ? 'ol' : 'ul'
            const start = marker.ordered && marker.start !== 1 ? ` start="${marker.start}"` : ''
            const rendered = items
                .map((item) => `<li>${renderBlocks(item, ctx, !loose)}</li>`)
                .join('\n')
            out.push(`<${tag}${start}>\n${rendered}\n</${tag}>`)
            continue
        }

        if (
            line.includes('|') &&
            index + 1 < lines.length &&
            TABLE_DELIMITER.test(lines[index + 1])
        ) {
            const header = splitTableRow(line)
            const delimiter = splitTableRow(lines[index + 1])
            if (header.length === delimiter.length) {
                flushParagraph()
                const aligns = delimiter.map((cell) => {
                    const left = cell.startsWith(':')
                    const right = cell.endsWith(':')
                    return left && right ? 'center' : right ? 'right' : left ? 'left' : ''
                })
                const cell = (tag: 'th' | 'td', content: string, column: number) =>
                    `<${tag}${aligns[column] ? ` style="text-align:${aligns[column]}"` : ''}>${renderInline(content, ctx)}</${tag}>`
                const head = `<thead>\n<tr>\n${header.map((content, column) => cell('th', content, column)).join('\n')}\n</tr>\n</thead>`
                index += 2
                const body: string[] = []
                while (
                    index < lines.length &&
                    !isBlank(lines[index]) &&
                    leadingSpaces(lines[index]) < 4
                ) {
                    const cells = splitTableRow(lines[index])
                    body.push(
                        `<tr>\n${header
                            .map((_, column) => cell('td', cells[column] ?? '', column))
                            .join('\n')}\n</tr>`,
                    )
                    index++
                }
                out.push(
                    `<table>\n${head}\n${body.length ? `<tbody>\n${body.join('\n')}\n</tbody>\n` : ''}</table>`,
                )
                continue
            }
        }

        if (!paragraph.length) {
            const reference = REF_DEF.exec(line)
            if (reference) {
                const label = normalizeLabel(reference[1])
                if (!ctx.refs.has(label)) {
                    ctx.refs.set(label, {
                        href: reference[2],
                        title: reference[3] ?? reference[4] ?? reference[5],
                    })
                }
                index++
                continue
            }
        }

        paragraph.push(line)
        index++
    }
    flushParagraph()
    return out.join('\n')
}

// Inline level -----------------------------------------------------------------------------------

type InlineNode =
    | { type: 'text'; text: string }
    | { type: 'html'; html: string }
    | {
          type: 'delim'
          char: string
          length: number
          canOpen: boolean
          canClose: boolean
          active: boolean
      }

const AUTOLINK = /^<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^<>\s]*)>/
const EMAIL_AUTOLINK =
    /^<([a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*)>/
const INLINE_HTML =
    /^(?:<[A-Za-z][A-Za-z0-9-]*(?:\s+[a-zA-Z_:][a-zA-Z0-9_.:-]*(?:\s*=\s*(?:[^"'=<>`\s]+|'[^']*'|"[^"]*"))?)*\s*\/?>|<\/[A-Za-z][A-Za-z0-9-]*\s*>|<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<![A-Za-z][^>]*>|<!\[CDATA\[[\s\S]*?\]\]>)/
const ENTITY = /^&(?:#\d{1,7}|#[xX][0-9a-fA-F]{1,6}|[a-zA-Z][a-zA-Z0-9]{1,31});/
const LINKIFY = /(?:https?:\/\/|ftp:\/\/|mailto:)[^\s<>"]+/g

function isWhitespace(char: string | undefined) {
    return char === undefined || /\s/.test(char)
}

function isPunctuation(char: string | undefined) {
    return char !== undefined && PUNCTUATION.test(char)
}

function findClosingBracket(text: string, open: number) {
    let depth = 0
    for (let index = open; index < text.length; index++) {
        const char = text[index]
        if (char === '\\') {
            index++
            continue
        }
        if (char === '`') {
            const run = /^`+/.exec(text.slice(index))![0]
            const close = text.indexOf(run, index + run.length)
            if (close !== -1) index = close + run.length - 1
            continue
        }
        if (char === '[') depth++
        else if (char === ']') {
            depth--
            if (depth === 0) return index
        }
    }
    return -1
}

function parseLinkDestination(text: string, start: number) {
    // `start` points just after "(".
    let index = start
    while (index < text.length && /[ \t\n]/.test(text[index])) index++
    let href = ''
    if (text[index] === '<') {
        const close = text.indexOf('>', index)
        if (close === -1) return null
        href = text.slice(index + 1, close)
        index = close + 1
    } else {
        let depth = 0
        const begin = index
        while (index < text.length) {
            const char = text[index]
            if (char === '\\') {
                index += 2
                continue
            }
            if (/\s/.test(char)) break
            if (char === '(') depth++
            if (char === ')') {
                if (depth === 0) break
                depth--
            }
            index++
        }
        href = text.slice(begin, index)
    }
    while (index < text.length && /[ \t\n]/.test(text[index])) index++
    let title: string | undefined
    const quote = text[index]
    if (quote === '"' || quote === "'" || quote === '(') {
        const closeChar = quote === '(' ? ')' : quote
        const close = text.indexOf(closeChar, index + 1)
        if (close === -1) return null
        title = text.slice(index + 1, close)
        index = close + 1
        while (index < text.length && /[ \t\n]/.test(text[index])) index++
    }
    if (text[index] !== ')') return null
    return { href: href.replace(/\\(.)/g, '$1'), title, end: index + 1 }
}

function plainText(html: string) {
    return html.replace(/<[^>]*>/g, '')
}

function linkify(text: string) {
    let result = ''
    let last = 0
    for (const match of text.matchAll(LINKIFY)) {
        let url = match[0]
        while (/[.,;:!?'"*_~]$/.test(url)) url = url.slice(0, -1)
        while (
            url.endsWith(')') &&
            (url.match(/\(/g)?.length ?? 0) < (url.match(/\)/g)?.length ?? 0)
        )
            url = url.slice(0, -1)
        const position = match.index ?? 0
        result += escapeText(text.slice(last, position))
        result += `<a href="${escapeHtml(url)}">${escapeText(url)}</a>`
        last = position + url.length
    }
    return result + escapeText(text.slice(last))
}

function renderInline(text: string, ctx: RenderContext, inLink = false): string {
    const nodes: InlineNode[] = []
    let buffer = ''
    const flush = () => {
        if (buffer) nodes.push({ type: 'text', text: buffer })
        buffer = ''
    }
    let index = 0
    while (index < text.length) {
        const char = text[index]

        if (char === '\\') {
            const next = text[index + 1]
            if (next === '\n') {
                flush()
                nodes.push({ type: 'html', html: '<br>\n' })
                index += 2
                continue
            }
            if (next !== undefined && isPunctuation(next)) {
                flush()
                nodes.push({ type: 'html', html: escapeHtml(next) })
                index += 2
                continue
            }
        }

        if (char === '`') {
            const run = /^`+/.exec(text.slice(index))![0]
            const close = text.indexOf(run, index + run.length)
            let validClose = close
            while (validClose !== -1 && text[validClose + run.length] === '`') {
                validClose = text.indexOf(run, validClose + run.length + 1)
            }
            if (validClose !== -1) {
                flush()
                let code = text.slice(index + run.length, validClose).replace(/\n/g, ' ')
                if (/^ .* $/.test(code) && code.trim()) code = code.slice(1, -1)
                nodes.push({ type: 'html', html: `<code>${escapeHtml(code)}</code>` })
                index = validClose + run.length
                continue
            }
            buffer += run
            index += run.length
            continue
        }

        if (char === '<') {
            const rest = text.slice(index)
            const autolink = AUTOLINK.exec(rest)
            if (autolink) {
                flush()
                nodes.push({
                    type: 'html',
                    html: `<a href="${escapeHtml(autolink[1])}">${escapeText(autolink[1])}</a>`,
                })
                index += autolink[0].length
                continue
            }
            const email = EMAIL_AUTOLINK.exec(rest)
            if (email) {
                flush()
                nodes.push({
                    type: 'html',
                    html: `<a href="mailto:${escapeHtml(email[1])}">${escapeText(email[1])}</a>`,
                })
                index += email[0].length
                continue
            }
            const html = INLINE_HTML.exec(rest)
            if (html) {
                flush()
                nodes.push({ type: 'html', html: html[0] })
                index += html[0].length
                continue
            }
        }

        if (char === '&') {
            const entity = ENTITY.exec(text.slice(index))
            if (entity) {
                flush()
                nodes.push({ type: 'html', html: entity[0] })
                index += entity[0].length
                continue
            }
        }

        if ((char === '!' && text[index + 1] === '[') || (char === '[' && !inLink)) {
            const open = char === '!' ? index + 1 : index
            const close = findClosingBracket(text, open)
            if (close !== -1) {
                const label = text.slice(open + 1, close)
                let target: { href: string; title?: string; end: number } | null = null
                if (text[close + 1] === '(') {
                    target = parseLinkDestination(text, close + 2)
                } else {
                    const refMatch = /^\[([^\]]*)\]/.exec(text.slice(close + 1))
                    const refLabel = refMatch && refMatch[1] ? refMatch[1] : label
                    const reference = ctx.refs.get(normalizeLabel(refLabel))
                    if (reference) {
                        target = {
                            href: reference.href,
                            title: reference.title,
                            end: close + 1 + (refMatch ? refMatch[0].length : 0),
                        }
                    }
                }
                if (target) {
                    flush()
                    const title = target.title ? ` title="${escapeHtml(target.title)}"` : ''
                    if (char === '!') {
                        const alt = plainText(renderInline(label, ctx, true))
                        nodes.push({
                            type: 'html',
                            html: `<img src="${escapeHtml(target.href)}" alt="${escapeHtml(alt)}"${title}>`,
                        })
                    } else {
                        nodes.push({
                            type: 'html',
                            html: `<a href="${escapeHtml(target.href)}"${title}>${renderInline(label, ctx, true)}</a>`,
                        })
                    }
                    index = target.end
                    continue
                }
            }
        }

        if (char === '*' || char === '_' || char === '~') {
            const run = new RegExp(`^\\${char}+`).exec(text.slice(index))![0]
            const before = text[index - 1]
            const after = text[index + run.length]
            const leftFlanking =
                !isWhitespace(after) &&
                (!isPunctuation(after) || isWhitespace(before) || isPunctuation(before))
            const rightFlanking =
                !isWhitespace(before) &&
                (!isPunctuation(before) || isWhitespace(after) || isPunctuation(after))
            if (char === '~' && run.length !== 2) {
                buffer += run
                index += run.length
                continue
            }
            flush()
            nodes.push({
                type: 'delim',
                char,
                length: run.length,
                canOpen:
                    char === '_'
                        ? leftFlanking && (!rightFlanking || isPunctuation(before))
                        : leftFlanking,
                canClose:
                    char === '_'
                        ? rightFlanking && (!leftFlanking || isPunctuation(after))
                        : rightFlanking,
                active: true,
            })
            index += run.length
            continue
        }

        if (char === '\n') {
            const trailing = /( {2,})$/.exec(buffer)
            if (trailing) {
                buffer = buffer.slice(0, -trailing[1].length)
                flush()
                nodes.push({ type: 'html', html: '<br>\n' })
            } else {
                buffer = buffer.replace(/ +$/, '')
                buffer += '\n'
            }
            index++
            continue
        }

        buffer += char
        index++
    }
    flush()

    // Emphasis, strong and strikethrough (CommonMark delimiter processing).
    const wraps = new Map<InlineNode, { before: string[]; after: string[] }>()
    const wrap = (node: InlineNode) => {
        let entry = wraps.get(node)
        if (!entry) {
            entry = { before: [], after: [] }
            wraps.set(node, entry)
        }
        return entry
    }
    for (let closerIndex = 0; closerIndex < nodes.length; closerIndex++) {
        const closer = nodes[closerIndex]
        if (closer.type !== 'delim' || !closer.canClose || !closer.active) continue
        while (closer.length > 0) {
            let openerIndex = -1
            for (let candidate = closerIndex - 1; candidate >= 0; candidate--) {
                const opener = nodes[candidate]
                if (
                    opener.type !== 'delim' ||
                    !opener.active ||
                    opener.char !== closer.char ||
                    !opener.canOpen ||
                    opener.length === 0
                )
                    continue
                if (
                    closer.char !== '~' &&
                    (opener.canClose || closer.canOpen) &&
                    (opener.length + closer.length) % 3 === 0 &&
                    !(opener.length % 3 === 0 && closer.length % 3 === 0)
                )
                    continue
                openerIndex = candidate
                break
            }
            if (openerIndex === -1) break
            const opener = nodes[openerIndex] as Extract<InlineNode, { type: 'delim' }>
            let tag: string
            let used: number
            if (closer.char === '~') {
                tag = 's'
                used = 2
            } else {
                used = opener.length >= 2 && closer.length >= 2 ? 2 : 1
                tag = used === 2 ? 'strong' : 'em'
            }
            opener.length -= used
            closer.length -= used
            wrap(opener).after.unshift(`<${tag}>`)
            wrap(closer).before.push(`</${tag}>`)
            for (let between = openerIndex + 1; between < closerIndex; between++) {
                const node = nodes[between]
                if (node.type === 'delim') node.active = false
            }
        }
    }

    let html = ''
    for (const node of nodes) {
        const extra = wraps.get(node)
        if (extra) html += extra.before.join('')
        if (node.type === 'text') html += inLink ? escapeText(node.text) : linkify(node.text)
        else if (node.type === 'html') html += node.html
        else html += node.char.repeat(node.length)
        if (extra) html += extra.after.join('')
    }
    return html
}

// Sanitizing (`configuredXss`) -------------------------------------------------------------------

const ATTRIBUTE_WHITELIST: Record<string, string[]> = {
    a: ['target', 'href', 'title', 'rel'],
    abbr: ['title'],
    address: [],
    area: ['shape', 'coords', 'href', 'alt', 'target', 'title', 'rel'],
    article: [],
    aside: [],
    b: [],
    bdi: ['dir'],
    bdo: ['dir'],
    big: [],
    blockquote: ['cite'],
    br: [],
    caption: [],
    center: [],
    cite: [],
    code: [],
    col: ['align', 'valign', 'span', 'width'],
    colgroup: ['align', 'valign', 'span', 'width'],
    dd: [],
    del: ['datetime'],
    details: ['open'],
    div: ['align'],
    dl: [],
    dt: [],
    em: [],
    figcaption: [],
    figure: [],
    font: ['color', 'size', 'face'],
    footer: [],
    h1: ['id'],
    h2: ['id'],
    h3: ['id'],
    h4: ['id'],
    h5: ['id'],
    h6: ['id'],
    header: [],
    hr: [],
    i: [],
    img: ['src', 'alt', 'title', 'width', 'height', 'loading', 'usemap', 'style', 'align'],
    input: ['checked', 'disabled', 'type'],
    ins: ['datetime'],
    kbd: ['id'],
    li: [],
    map: ['name'],
    mark: [],
    nav: [],
    ol: ['start'],
    p: ['align'],
    picture: [],
    pre: [],
    s: [],
    section: [],
    small: [],
    span: [],
    strike: [],
    strong: [],
    sub: [],
    summary: [],
    sup: [],
    table: ['width', 'border', 'align', 'valign'],
    tbody: ['align', 'valign'],
    td: ['width', 'rowspan', 'colspan', 'align', 'valign', 'style'],
    tfoot: ['align', 'valign'],
    th: ['width', 'rowspan', 'colspan', 'align', 'valign', 'style'],
    thead: ['align', 'valign'],
    tr: ['rowspan', 'align', 'valign'],
    tt: [],
    u: [],
    ul: [],
}

/** Elements that would load remote media inside the window; they are left out. */
const EMBED_TAGS = new Set(['iframe', 'video', 'audio', 'source'])

const SAFE_URL = /^(?:#|\/|\.\/|\.\.\/|https?:\/\/|mailto:|tel:|ftp:\/\/|data:image\/)/i
const ATTRIBUTE = /([^\s=/>]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g
const STYLE_WHITELIST: Record<string, RegExp> = {
    'image-rendering': /^pixelated$/,
    'text-align': /^(?:center|left|right)$/,
    float: /^(?:left|right)$/,
}

function filterStyle(value: string) {
    return value
        .split(';')
        .map((declaration) => declaration.trim())
        .filter(Boolean)
        .filter((declaration) => {
            const [property, ...rest] = declaration.split(':')
            const check = STYLE_WHITELIST[property?.trim().toLowerCase() ?? '']
            return !!check && check.test(rest.join(':').trim())
        })
        .join('; ')
}

function isRemote(value: string) {
    return /^(?:https?:)?\/\//i.test(value.trim())
}

function sanitizeTag(tag: string): string {
    const closing = /^<\//.test(tag)
    const nameMatch = /^<\/?([a-zA-Z][a-zA-Z0-9-]*)/.exec(tag)
    if (!nameMatch) return escapeText(tag)
    const name = nameMatch[1].toLowerCase()
    if (EMBED_TAGS.has(name)) return ''
    const allowed = ATTRIBUTE_WHITELIST[name]
    if (!allowed) return escapeText(tag)
    if (closing) return `</${name}>`

    const attributes: string[] = []
    const source = tag.slice(nameMatch[0].length).replace(/\/?>$/, '')
    let href: string | null = null
    let alt = ''
    let remoteImage = false
    for (const match of source.matchAll(ATTRIBUTE)) {
        const attribute = match[1].toLowerCase()
        const value = match[2] ?? match[3] ?? match[4] ?? ''
        if (attribute === 'alt') alt = value
        if (name === 'pre' || name === 'code' || name === 'span') {
            if (attribute === 'class') {
                const classes = value
                    .split(/\s+/)
                    .filter((entry) => entry.startsWith('hljs-') || entry.startsWith('language-'))
                attributes.push(`class="${escapeHtml(classes.join(' '))}"`)
                continue
            }
        }
        if (!allowed.includes(attribute)) continue
        let safe = value
        if (attribute === 'href' || attribute === 'src') {
            if (!SAFE_URL.test(value.trim())) continue
            if (attribute === 'src' && name === 'img' && isRemote(value)) remoteImage = true
            if (attribute === 'href') href = value
        }
        if (attribute === 'style') {
            safe = filterStyle(value)
            if (!safe) continue
        }
        if (attribute === 'target' || attribute === 'rel') continue
        attributes.push(`${attribute}="${escapeHtml(safe)}"`)
    }
    if (name === 'img' && remoteImage) return escapeText(alt)
    if (name === 'a' && href) {
        let external = false
        try {
            const url = new URL(href)
            external = url.protocol === 'http:' || url.protocol === 'https:'
            if (url.hostname !== 'modrinth.com') attributes.push('rel="noopener nofollow ugc"')
        } catch {
            attributes.push('rel="noopener nofollow ugc"')
        }
        if (external) attributes.push('target="_blank"')
    }
    if (name === 'img') attributes.push('loading="lazy"')
    return `<${name}${attributes.length ? ` ${attributes.join(' ')}` : ''}>`
}

function sanitize(html: string): string {
    const withoutComments = html.replace(/<!--[\s\S]*?-->/g, '')
    let result = ''
    let last = 0
    const TAG =
        /<\/?[a-zA-Z][a-zA-Z0-9-]*(?:\s+[^\s=/>]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'=<>`]+))?)*\s*\/?>/g
    for (const match of withoutComments.matchAll(TAG)) {
        const position = match.index ?? 0
        result += withoutComments.slice(last, position).replace(/</g, '&lt;').replace(/>/g, '&gt;')
        result += sanitizeTag(match[0])
        last = position + match[0].length
    }
    result += withoutComments.slice(last).replace(/</g, '&lt;').replace(/>/g, '&gt;')
    return result
}

export function renderMarkdown(source: string): string {
    if (!source) return ''
    const lines = source.replace(/\r\n?/g, '\n').split('\n')
    // Link reference definitions apply to the whole document, wherever they are written.
    const refs = new Map<string, LinkReference>()
    let fenced = false
    for (const line of lines) {
        if (FENCE.test(line)) fenced = !fenced
        if (fenced) continue
        const reference = REF_DEF.exec(line)
        if (reference) {
            const label = normalizeLabel(reference[1])
            if (!refs.has(label))
                refs.set(label, {
                    href: reference[2],
                    title: reference[3] ?? reference[4] ?? reference[5],
                })
        }
    }
    return sanitize(renderBlocks(lines, { refs }))
}
