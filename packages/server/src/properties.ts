export type PropertiesEntry =
    | { type: 'comment'; text: string }
    | { type: 'blank' }
    | { type: 'pair'; key: string; value: string; separator: string }

const COMMENT_RE = /^\s*[#!]/
const ESCAPE_RE = /\\(.)/g

function unescapeValue(value: string): string {
    return value.replace(ESCAPE_RE, (_, char: string) => {
        switch (char) {
            case 'n':
                return '\n'
            case 't':
                return '\t'
            case 'r':
                return '\r'
            case 'f':
                return '\f'
            default:
                return char
        }
    })
}

function escapeValue(value: string): string {
    return value
        .replace(/\\/g, '\\\\')
        .replace(/\n/g, '\\n')
        .replace(/\t/g, '\\t')
        .replace(/\r/g, '\\r')
}

/** Finds the first unescaped `=` or `:` separator, falling back to whitespace. */
function findSeparator(line: string): number {
    for (let i = 0; i < line.length; i++) {
        const char = line[i]
        if (char === '\\') {
            i++
            continue
        }
        if (char === '=' || char === ':') return i
        if (/\s/.test(char)) {
            let j = i
            while (j < line.length && /\s/.test(line[j])) j++
            if (line[j] === '=' || line[j] === ':') return j
            return i
        }
    }
    return -1
}

function unescapeKey(key: string): string {
    return unescapeValue(key).replace(/\\([ =:])/g, '$1')
}

/**
 * Parses a Java `.properties` document, preserving comment lines, blank lines,
 * key order, and the original `=`/`:` separators so it round-trips safely.
 */
export function parseProperties(text: string): PropertiesEntry[] {
    const entries: PropertiesEntry[] = []
    let pendingContinuation: {
        type: 'pair'
        key: string
        value: string
        separator: string
    } | null = null

    for (const rawLine of text.split(/\r?\n/)) {
        const line: string = pendingContinuation ? rawLine : rawLine.trim()
        if (pendingContinuation) {
            const continued = line.replace(/\\\s*$/, '')
            pendingContinuation.value += continued
            if (/\\\s*$/.test(rawLine)) continue
            entries.push(pendingContinuation)
            pendingContinuation = null
            continue
        }
        if (line === '') {
            entries.push({ type: 'blank' })
            continue
        }
        if (COMMENT_RE.test(line)) {
            entries.push({ type: 'comment', text: line })
            continue
        }
        const separatorIndex = findSeparator(line)
        if (separatorIndex === -1) {
            entries.push({ type: 'pair', key: unescapeKey(line), value: '', separator: '=' })
            continue
        }
        const key = unescapeKey(line.slice(0, separatorIndex).trimEnd())
        const separator = line[separatorIndex]
        const value: string = line.slice(separatorIndex + 1).trim()
        if (/\\\s*$/.test(value)) {
            pendingContinuation = {
                type: 'pair',
                key,
                value: value.replace(/\\\s*$/, ''),
                separator,
            }
            continue
        }
        entries.push({ type: 'pair', key, value: unescapeValue(value), separator })
    }
    if (pendingContinuation) entries.push(pendingContinuation)

    return entries
}

export function serializeProperties(entries: PropertiesEntry[]): string {
    return entries
        .map((entry) => {
            if (entry.type === 'blank') return ''
            if (entry.type === 'comment') return entry.text
            return `${entry.key.replace(/([ =:])/g, '\\$1')}${entry.separator}${escapeValue(entry.value)}`
        })
        .join('\n')
}

export function getProperty(entries: PropertiesEntry[], key: string): string | undefined {
    const entry = entries.find(
        (entry): entry is Extract<PropertiesEntry, { type: 'pair' }> =>
            entry.type === 'pair' && entry.key === key,
    )
    return entry?.value
}

export function setProperty(
    entries: PropertiesEntry[],
    key: string,
    value: string,
): PropertiesEntry[] {
    let updated = false
    const next = entries.map((entry) => {
        if (entry.type === 'pair' && entry.key === key) {
            updated = true
            return { ...entry, value }
        }
        return entry
    })
    if (!updated) next.push({ type: 'pair', key, value, separator: '=' })
    return next
}
