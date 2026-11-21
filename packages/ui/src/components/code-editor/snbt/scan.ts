/**
 * SNBT 的容错扫描：只看到光标为止，不抛异常。
 *
 * 补全要回答的是「光标这个位置现在能写什么」，而正在编辑的文本通常是残缺的
 * （少一个 `}`、字符串还没闭合）。所以这里不复用 `NbtTag.fromString`，而是自己
 * 走一遍词法，把容器栈、键名路径和期望类型算出来。
 *
 * 路径规则与 NBT 目录一致：列表元素不贡献键名，`Items[0].id` 的路径是
 * `['Items','id']`。
 */

import type { SnbtExpect, SnbtFrame, SnbtScan } from './types.ts'

// 键名不能带 `:`（`Pos:` 的冒号是分隔符），值可以是 `minecraft:stone`。
const KEY_WORD = /[-\w.+]/
const VALUE_WORD = /[-\w.:+]/
const SPACE = /\s/

function isSpace(char: string | undefined): boolean {
    return char !== undefined && SPACE.test(char)
}

function skipSpace(text: string, from: number, limit: number): number {
    let index = from
    while (index < limit && isSpace(text[index])) index += 1
    return index
}

function readWord(
    text: string,
    from: number,
    limit: number,
    pattern: RegExp,
): { word: string; end: number } {
    let index = from
    while (index < limit && pattern.test(text[index])) index += 1
    return { word: text.slice(from, index), end: index }
}

/** 未闭合的字符串返回 -1，由调用方标记 `quoted`。 */
function findStringEnd(text: string, from: number, limit: number, quote: string): number {
    let index = from + 1
    while (index < limit) {
        const char = text[index]
        if (char === '\\') {
            index += 2
            continue
        }
        if (char === quote) return index
        index += 1
    }
    return -1
}

/** 根层的 `[` 后面是 `name=…` 还是普通列表元素。 */
function looksLikeComponents(text: string, from: number, limit: number): boolean {
    const start = skipSpace(text, from, limit)
    const { word, end } = readWord(text, start, limit, VALUE_WORD)
    if (!word) return false
    const next = skipSpace(text, end, limit)
    return next < limit && text[next] === '='
}

/** 新容器属于哪个键：复合里的键名往下走一层，列表元素停在列表本身。 */
function childPath(frame: SnbtFrame | undefined, key: string): string[] {
    if (!frame) return []
    return frame.kind === 'compound' ? [...frame.keyPath, key] : frame.keyPath
}

export function scanSnbt(text: string, offset: number): SnbtScan {
    const limit = Math.max(0, Math.min(offset, text.length))
    const stack: SnbtFrame[] = []
    let expect: SnbtExpect = 'value'
    let key = ''
    let word = ''
    let quoted = false

    const top = () => stack[stack.length - 1]

    let index = 0
    while (index < limit) {
        const char = text[index]

        if (isSpace(char)) {
            index += 1
            continue
        }

        if (char === '"' || char === "'") {
            const close = findStringEnd(text, index, limit, char)
            if (close < 0) {
                quoted = true
                word = text.slice(index + 1)
                break
            }
            const content = text.slice(index + 1, close)
            const next = skipSpace(text, close + 1, limit)
            if (next < limit && text[next] === ':') {
                key = content
                expect = 'value'
                index = next + 1
            } else {
                expect = 'after'
                index = close + 1
            }
            continue
        }

        if (char === '{') {
            stack.push({ kind: 'compound', keyPath: childPath(top(), key), components: false })
            key = ''
            expect = 'key'
            index += 1
            continue
        }

        if (char === '[') {
            const root = stack.length === 0
            const components = root && looksLikeComponents(text, index + 1, limit)
            stack.push({ kind: 'list', keyPath: childPath(top(), key), components })
            key = ''
            expect = components ? 'component' : 'element'
            index += 1
            continue
        }

        if (char === '}' || char === ']') {
            stack.pop()
            key = ''
            expect = 'after'
            index += 1
            continue
        }

        if (char === ',') {
            const frame = top()
            key = ''
            expect =
                frame?.kind === 'compound' ? 'key' : frame?.components ? 'component' : 'element'
            index += 1
            continue
        }

        if (char === ':') {
            expect = 'value'
            index += 1
            continue
        }

        if (char === '=' || char === ';') {
            expect = 'value'
            index += 1
            continue
        }

        const wordPattern = expect === 'key' ? KEY_WORD : VALUE_WORD
        const { word: token, end } = readWord(text, index, limit, wordPattern)
        if (!token) {
            index += 1
            continue
        }
        // 词正好顶到光标：光标在这个词里面，期望类型保持进入这个词之前的样子。
        // 唯一的例外是数字——`{a:1|}` 里的 `1` 已经写完了，不该再提示值候选。
        if (end >= limit) {
            word = token
            if ((expect === 'value' || expect === 'element') && /^-?\d/.test(token)) {
                expect = 'after'
            }
            break
        }

        const next = skipSpace(text, end, limit)
        const nextChar = next < limit ? text[next] : ''
        if (expect === 'key' && nextChar === ':') {
            key = token
            expect = 'value'
            index = next + 1
            continue
        }
        if (expect === 'component' && nextChar === '=') {
            key = token
            expect = 'value'
            index = next + 1
            continue
        }
        if (expect === 'value' || expect === 'element') expect = 'after'
        index = end
    }

    const container = top()
    const after = skipSpace(text, limit, text.length)

    return {
        frames: stack,
        container,
        keyPath: container ? container.keyPath : [],
        expect,
        key,
        word,
        quoted,
        next: after < text.length ? text[after] : '',
    }
}
