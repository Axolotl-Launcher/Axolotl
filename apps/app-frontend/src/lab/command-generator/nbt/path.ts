/**
 * NBT 路径（`/data` 的 `path` 参数）的解析与校验。
 *
 * 路径不是 NBT 文档：它指向文档里的某个位置。语法依据 Minecraft Wiki
 * `NBT path format`：
 * - `Foo.Bar` 逐层取子标签；
 * - `Foo[0]` 取列表元素，`Foo[]` 取全部元素，`Foo[0..2]` 取一段；
 * - `Foo[{k:v}]` / `Foo{k:v}` 只保留匹配的复合标签；
 * - 键名含 `.` `[` `]` `{` `}` 空格等字符时必须加引号。
 */

import { matchBracket, splitTopLevel } from '../selector/parse.ts'

export type NbtPathSegment =
    | { kind: 'key'; name: string; quoted: boolean }
    | { kind: 'index'; index: number }
    | { kind: 'slice'; from?: number; to?: number }
    | { kind: 'all' }
    | { kind: 'filter'; raw: string }

export interface NbtPathIssue {
    level: 'error' | 'warning'
    message: string
    /** 出问题的位置，用于在输入框里定位。 */
    at: number
}

const BARE_KEY = /^[A-Za-z0-9_+-]+$/

function readQuoted(text: string, start: number): { value: string; end: number } | undefined {
    const quote = text[start]
    if (quote !== '"' && quote !== "'") return undefined
    let value = ''
    for (let index = start + 1; index < text.length; index += 1) {
        const char = text[index]
        if (char === '\\' && index + 1 < text.length) {
            value += text[index + 1]
            index += 1
            continue
        }
        if (char === quote) return { value, end: index + 1 }
        value += char
    }
    return undefined
}

/**
 * 把路径切成片段。无法识别时把剩下的部分当成一个 `key` 片段，保证输入过程中
 * 不会因为半截路径而抛异常。
 */
export function parseNbtPath(path: string): NbtPathSegment[] {
    const segments: NbtPathSegment[] = []
    let index = 0
    const text = path.trim()

    while (index < text.length) {
        const char = text[index]

        if (char === '.') {
            index += 1
            continue
        }

        if (char === '[') {
            const close = matchBracket(text, index)
            const inner = text.slice(index + 1, close === -1 ? text.length : close)
            const end = close === -1 ? text.length : close + 1

            if (inner === '') {
                segments.push({ kind: 'all' })
            } else if (inner.startsWith('{')) {
                segments.push({ kind: 'filter', raw: inner })
            } else if (inner.includes('..')) {
                const [from, to] = inner.split('..')
                segments.push({
                    kind: 'slice',
                    from: from.trim() === '' ? undefined : Number(from),
                    to: to.trim() === '' ? undefined : Number(to),
                })
            } else {
                segments.push({ kind: 'index', index: Number(inner) })
            }
            index = end
            continue
        }

        if (char === '{') {
            const close = matchBracket(text, index)
            const end = close === -1 ? text.length : close + 1
            segments.push({ kind: 'filter', raw: text.slice(index, end) })
            index = end
            continue
        }

        if (char === '"' || char === "'") {
            const quoted = readQuoted(text, index)
            if (!quoted) {
                // 引号还没写完：把剩下的当成半截键名，别让光标跳动
                segments.push({ kind: 'key', name: text.slice(index + 1), quoted: true })
                break
            }
            segments.push({ kind: 'key', name: quoted.value, quoted: true })
            index = quoted.end
            continue
        }

        // 裸键名读到 `.` 或 `[` 为止
        let end = index
        while (end < text.length && !'.[{'.includes(text[end])) end += 1
        const name = text.slice(index, end)
        if (name) segments.push({ kind: 'key', name, quoted: false })
        index = end
    }

    return segments
}

export function formatNbtPath(segments: NbtPathSegment[]): string {
    return segments
        .map((segment, index) => {
            switch (segment.kind) {
                case 'key': {
                    const name =
                        segment.quoted || !BARE_KEY.test(segment.name)
                            ? `"${segment.name.replace(/"/g, '\\"')}"`
                            : segment.name
                    return index === 0 ? name : `.${name}`
                }
                case 'index':
                    return `[${segment.index}]`
                case 'slice':
                    return `[${segment.from ?? ''}..${segment.to ?? ''}]`
                case 'all':
                    return '[]'
                case 'filter':
                    return segment.raw
            }
        })
        .join('')
}

/** 路径是否合法。空路径在 `get` 时表示整个标签，允许。 */
export function validateNbtPath(path: string): NbtPathIssue[] {
    const issues: NbtPathIssue[] = []
    const text = path.trim()
    if (!text) return issues

    if (text.startsWith('.')) {
        issues.push({ level: 'error', message: '路径不能以点开头', at: 0 })
    }

    let depth = 0
    let quote: string | null = null
    for (let index = 0; index < text.length; index += 1) {
        const char = text[index]
        if (quote) {
            if (char === '\\') index += 1
            else if (char === quote) quote = null
            continue
        }
        if (char === '"' || char === "'") {
            quote = char
            continue
        }
        if (char === '[' || char === '{') depth += 1
        else if (char === ']' || char === '}') {
            depth -= 1
            if (depth < 0) {
                issues.push({ level: 'error', message: '多余的右括号', at: index })
                break
            }
        }
    }
    if (depth > 0) {
        issues.push({ level: 'error', message: '括号没有闭合', at: text.length })
    }

    for (const segment of parseNbtPath(text)) {
        if (segment.kind === 'index' && !Number.isInteger(segment.index)) {
            issues.push({ level: 'error', message: '列表下标必须是整数', at: 0 })
        }
        if (segment.kind === 'slice') {
            if (segment.from !== undefined && !Number.isInteger(segment.from)) {
                issues.push({ level: 'error', message: '区间起点必须是整数', at: 0 })
            }
            if (segment.to !== undefined && !Number.isInteger(segment.to)) {
                issues.push({ level: 'error', message: '区间终点必须是整数', at: 0 })
            }
        }
        if (segment.kind === 'filter') {
            const inner = segment.raw.replace(/^\[/, '').replace(/\]$/, '')
            const body = inner.slice(1, -1)
            for (const part of splitTopLevel(body, ',')) {
                if (part.text.trim() && !part.text.includes(':')) {
                    issues.push({
                        level: 'warning',
                        message: '复合标签筛选要写成 key:value',
                        at: 0,
                    })
                }
            }
        }
    }

    return issues
}

/**
 * 当前正在输入的最后一个片段，用于补全。
 *
 * `depth` 是「用户想要第几层字段」：写完一个点之后要的是下一层，所以加一。
 */
export function pathCompletionPrefix(path: string): { prefix: string; depth: number } {
    const trimmed = path.trimEnd()
    const segments = parseNbtPath(trimmed)
    const keyCount = segments.filter((segment) => segment.kind === 'key').length
    if (trimmed.endsWith('.')) return { prefix: '', depth: keyCount + 1 }
    const last = segments[segments.length - 1]
    if (last && last.kind === 'key' && !last.quoted) {
        return { prefix: last.name, depth: keyCount }
    }
    return { prefix: '', depth: keyCount + 1 }
}

/** 把补全结果接到路径末尾。 */
export function appendPathKey(path: string, key: string): string {
    const trimmed = path.trimEnd()
    if (!trimmed) return key
    if (trimmed.endsWith('.')) return `${trimmed}${key}`
    return `${trimmed}.${key}`
}
