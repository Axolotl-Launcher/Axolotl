/**
 * SNBT 的排版：把压成一行（或本来就乱）的文本展开成缩进好的多行。
 *
 * 这里不解析——只走一遍词法，所以残缺的文本也能排版，而且原样的写法（数字
 * 后缀、引号风格、`minecraft:` 前缀）全都保留。语法是否正确由解析器负责，
 * 这个函数只关心换行和缩进。
 */

/** 与编辑器的 `tabSize` 对齐。 */
const INDENT = '    '
/** 一出现就要单独处理的字符；其余的连成一段原样搬走。 */
const STRUCTURAL = new Set(['{', '}', '[', ']', ',', '"', "'"])
/** 数组前缀 `[B;`、`[I;`、`[L;` 里的元素不换行。 */
const ARRAY_PREFIX = /^[BIL];/

interface Frame {
    array: boolean
}

function isSpace(char: string | undefined): boolean {
    return char !== undefined && /\s/.test(char)
}

/** 跳过空白，返回下一个非空白字符的下标（越界就返回长度）。 */
function nextSignificant(source: string, from: number): number {
    let index = from
    while (index < source.length && isSpace(source[index])) index += 1
    return index
}

/** 字符串字面量的结束位置（含右引号）；没闭合就到末尾。 */
function stringEnd(source: string, from: number): number {
    const quote = source[from]
    let index = from + 1
    while (index < source.length) {
        const char = source[index]
        if (char === '\\') {
            index += 2
            continue
        }
        if (char === quote) return index + 1
        index += 1
    }
    return source.length
}

export function formatSnbt(source: string, indent = INDENT): string {
    const out: string[] = []
    const stack: Frame[] = []
    let depth = 0
    let lineStart = true

    const line = () => {
        out.push('\n' + indent.repeat(depth))
        lineStart = true
    }
    const push = (text: string) => {
        out.push(text)
        lineStart = false
    }

    let index = 0
    while (index < source.length) {
        const char = source[index]

        if (isSpace(char)) {
            index += 1
            continue
        }

        if (char === '"' || char === "'") {
            const end = stringEnd(source, index)
            push(source.slice(index, end))
            index = end
            continue
        }

        if (char === '{' || char === '[') {
            const close = char === '{' ? '}' : ']'
            const inner = nextSignificant(source, index + 1)
            // 空容器保持一行：`{}`、`[]`。
            if (source[inner] === close) {
                push(char + close)
                index = inner + 1
                continue
            }
            if (char === '[') {
                const prefix = ARRAY_PREFIX.exec(source.slice(inner, inner + 2))
                if (prefix) {
                    push(`[${prefix[0]}`)
                    stack.push({ array: true })
                    index = inner + 2
                    continue
                }
            }
            push(char)
            stack.push({ array: false })
            depth += 1
            line()
            index += 1
            continue
        }

        if (char === '}' || char === ']') {
            const frame = stack.pop()
            depth = Math.max(0, depth - 1)
            if (!frame?.array) {
                // 尾逗号已经换过行了，别再空出一行。
                if (lineStart) out.pop()
                line()
            }
            push(char)
            index += 1
            continue
        }

        if (char === ',') {
            push(char)
            if (!stack[stack.length - 1]?.array) line()
            index += 1
            continue
        }

        let end = index + 1
        while (end < source.length && !isSpace(source[end]) && !STRUCTURAL.has(source[end])) {
            end += 1
        }
        push(source.slice(index, end))
        index = end
    }

    return out.join('')
}
