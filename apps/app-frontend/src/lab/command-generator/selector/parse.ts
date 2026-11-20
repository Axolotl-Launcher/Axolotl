/**
 * 目标选择器的读取。
 *
 * 解析是「尽力而为」的：能认出来的参数变成结构化数据，认不出来的原样保留，
 * 括号不配对也不会抛异常——用户在输入框里打字时，半截的选择器是常态。
 */

import {
    bedrockEquivalent,
    SELECTOR_VARIABLES,
    selectorArgumentSpec,
    type SelectorVariable,
} from './arguments.ts'

export interface ParsedSelectorArgument {
    key: string
    /** 原始值，含 `!` 前缀。 */
    value: string
    /** 值以 `!` 开头（排除语义）。 */
    negated: boolean
    /** 在原始文本里的区间，用于就地替换。 */
    start: number
    end: number
}

export interface ParsedSelector {
    variable: SelectorVariable
    arguments: ParsedSelectorArgument[]
    /** 变量名不是 Java 版六个之一（`@c` / `@v` / `@initiator`）。 */
    unknownVariable?: string
    /** 结构性问题；有值时 `arguments` 仍是能读到的部分。 */
    error?: 'not-a-selector' | 'unclosed'
}

/** 按分隔符切分，但跳过 `{}` / `[]` / 引号内部。 */
export function splitTopLevel(
    text: string,
    separator: string,
): Array<{ text: string; start: number; end: number }> {
    const parts: Array<{ text: string; start: number; end: number }> = []
    let depth = 0
    let quote: string | null = null
    let start = 0
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
        if (char === '{' || char === '[') depth += 1
        else if (char === '}' || char === ']') depth -= 1
        else if (char === separator && depth === 0) {
            parts.push({ text: text.slice(start, index), start, end: index })
            start = index + 1
        }
    }
    parts.push({ text: text.slice(start), start, end: text.length })
    return parts
}

/** 找到与 `open` 配对的收尾括号，找不到返回 -1。 */
export function matchBracket(text: string, open: number): number {
    const opening = text[open]
    const closing = opening === '[' ? ']' : '}'
    let depth = 0
    let quote: string | null = null
    for (let index = open; index < text.length; index += 1) {
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
        if (char === opening) depth += 1
        else if (char === closing) {
            depth -= 1
            if (depth === 0) return index
        }
    }
    return -1
}

function isSelectorVariable(value: string): value is SelectorVariable {
    return (SELECTOR_VARIABLES as readonly string[]).includes(value)
}

/**
 * 解析选择器文本。`@e[type=zombie,limit=3]` 这样写完整时返回全部参数；
 * 写到一半（`@e[type=zom`）时返回已能识别的部分，并把 `unclosed` 标出来。
 */
export function parseSelector(text: string): ParsedSelector | undefined {
    const trimmed = text.trim()
    if (!trimmed.startsWith('@')) return undefined
    // 参数位置是相对原文的：前面有空格时要把偏移补回来，否则就地改值会错位
    const shift = text.length - text.trimStart().length
    const nameMatch = /^@([A-Za-z_]+)/.exec(trimmed)
    if (!nameMatch) return undefined

    const name = nameMatch[1]
    const bodyStart = nameMatch[0].length
    const rest = trimmed.slice(bodyStart)
    const open = rest.indexOf('[')

    const result: ParsedSelector = {
        variable: isSelectorVariable(name) ? name : 'e',
        arguments: [],
    }
    if (!isSelectorVariable(name)) result.unknownVariable = name
    if (open === -1) return result

    const close = matchBracket(trimmed, bodyStart + open)
    if (close === -1) result.error = 'unclosed'
    const inner = trimmed.slice(bodyStart + open + 1, close === -1 ? trimmed.length : close)
    const offset = bodyStart + open + 1

    for (const part of splitTopLevel(inner, ',')) {
        if (!part.text.trim()) continue
        const eq = part.text.indexOf('=')
        if (eq === -1) continue
        const key = part.text.slice(0, eq).trim()
        if (!key) continue
        const rawValue = part.text.slice(eq + 1).trim()
        const negated = rawValue.startsWith('!')
        result.arguments.push({
            key,
            value: rawValue,
            negated,
            start:
                shift +
                offset +
                part.start +
                eq +
                1 +
                (part.text.length - part.text.trimStart().length),
            end: shift + offset + part.end,
        })
    }
    return result
}

// ---------------------------------------------------------------- 值解析

export interface NumericRange {
    min?: number
    max?: number
}

/**
 * 解析数字区间：`10`、`8..16`、`..10`、`10..`。
 * 单值时 `min === max`。`!` 前缀会被忽略（调用方看 `negated`）。
 */
export function parseNumericRange(value: string): NumericRange | undefined {
    const text = value.replace(/^!/, '').trim()
    if (!text) return undefined
    if (text.includes('..')) {
        const [left, right] = text.split('..')
        const min = left.trim() === '' ? undefined : Number(left)
        const max = right.trim() === '' ? undefined : Number(right)
        if (min !== undefined && !Number.isFinite(min)) return undefined
        if (max !== undefined && !Number.isFinite(max)) return undefined
        if (min === undefined && max === undefined) return undefined
        return { min, max }
    }
    const exact = Number(text)
    if (!Number.isFinite(exact)) return undefined
    return { min: exact, max: exact }
}

export function formatNumericRange(range: NumericRange): string {
    if (range.min === undefined && range.max === undefined) return ''
    if (range.min !== undefined && range.min === range.max) return String(range.min)
    return `${range.min ?? ''}..${range.max ?? ''}`
}

/** `{objective=10,other=1..5}` → 键值对。 */
export function parseScores(value: string): Record<string, string> {
    const inner = unwrapBraces(value)
    if (inner === undefined) return {}
    const out: Record<string, string> = {}
    for (const part of splitTopLevel(inner, ',')) {
        const eq = part.text.indexOf('=')
        if (eq === -1) continue
        const key = part.text.slice(0, eq).trim()
        if (!key) continue
        out[key] = part.text.slice(eq + 1).trim()
    }
    return out
}

/** `{story/smelt_iron=true}` 或 `{adv={criterion=true}}`。 */
export function parseAdvancements(
    value: string,
): Record<string, boolean | Record<string, boolean>> {
    const inner = unwrapBraces(value)
    if (inner === undefined) return {}
    const out: Record<string, boolean | Record<string, boolean>> = {}
    for (const part of splitTopLevel(inner, ',')) {
        const eq = part.text.indexOf('=')
        if (eq === -1) continue
        const key = part.text.slice(0, eq).trim()
        if (!key) continue
        const rawValue = part.text.slice(eq + 1).trim()
        if (rawValue.startsWith('{')) {
            const nested = unwrapBraces(rawValue) ?? ''
            const criteria: Record<string, boolean> = {}
            for (const entry of splitTopLevel(nested, ',')) {
                const innerEq = entry.text.indexOf('=')
                if (innerEq === -1) continue
                criteria[entry.text.slice(0, innerEq).trim()] =
                    entry.text.slice(innerEq + 1).trim() === 'true'
            }
            out[key] = criteria
            continue
        }
        out[key] = rawValue === 'true'
    }
    return out
}

/** 去掉最外层的一对花括号；没有花括号时返回原文本。 */
export function unwrapBraces(value: string): string | undefined {
    const trimmed = value.trim()
    if (!trimmed.startsWith('{')) return trimmed
    const close = matchBracket(trimmed, 0)
    if (close === -1) return trimmed.slice(1)
    return trimmed.slice(1, close)
}

// ---------------------------------------------------------------- 补全

export interface CompletionContext {
    variable: SelectorVariable
    /** 光标是否在 `[...]` 内。 */
    insideBrackets: boolean
    /** 当前正在输入的参数名（不含值）。 */
    prefix: string
    /** 光标前已写好的参数名，用于去重。 */
    usedKeys: string[]
    /** 已写了 `key=` 但还没写值。 */
    expectsValueFor?: string
}

/**
 * 光标位置决定补全内容：
 * - `@e[` → 列出全部参数；
 * - `@e[t` → 只剩 `type`；
 * - `@e[type=zombie,` → 跳过已用的不可重复参数。
 */
export function completionContext(text: string, caret: number): CompletionContext | undefined {
    const before = text.slice(0, Math.max(0, Math.min(caret, text.length)))
    const parsed = parseSelector(before)
    if (!parsed) return undefined

    const open = before.indexOf('[')
    if (open === -1 || before.length <= open) {
        return {
            variable: parsed.variable,
            insideBrackets: false,
            prefix: '',
            usedKeys: [],
        }
    }

    const inner = before.slice(open + 1)
    const parts = splitTopLevel(inner, ',')
    const last = parts[parts.length - 1]
    const usedKeys = parts
        .slice(0, -1)
        .map((part) => part.text.split('=')[0]?.trim())
        .filter(Boolean)
    const eq = last.text.indexOf('=')

    if (eq !== -1) {
        return {
            variable: parsed.variable,
            insideBrackets: true,
            prefix: '',
            usedKeys,
            expectsValueFor: last.text.slice(0, eq).trim(),
        }
    }

    return {
        variable: parsed.variable,
        insideBrackets: true,
        prefix: last.text.trim(),
        usedKeys,
    }
}

// ---------------------------------------------------------------- 诊断

export interface SelectorIssue {
    /** `error` 阻断生成，`warning` 只是提醒。 */
    level: 'error' | 'warning'
    message: string
    argument?: string
}

/**
 * 检查选择器是否合法。规则来自 wiki：
 * - `type` 不能用在 `@a` / `@p` / `@r` 上；
 * - 不可重复的参数不能出现两次（`tag`、`nbt` 可以）；
 * - `type=<id>` 只能出现一次，且不能与 `type=!<id>` 同时出现；
 * - `sort` 与 `limit` 相互影响，缺一个时说明默认行为；
 * - 基岩版参数给出对应写法。
 */
export function validateSelector(parsed: ParsedSelector): SelectorIssue[] {
    const issues: SelectorIssue[] = []

    if (parsed.unknownVariable) {
        issues.push({
            level: 'error',
            message: `@${parsed.unknownVariable} 不是 Java 版的目标变量`,
        })
    }
    if (parsed.error === 'unclosed') {
        issues.push({ level: 'error', message: '方括号没有闭合' })
    }

    const seen = new Map<string, number>()
    let positiveType = 0
    let negativeType = 0

    for (const argument of parsed.arguments) {
        const spec = selectorArgumentSpec(argument.key)
        seen.set(argument.key, (seen.get(argument.key) ?? 0) + 1)

        if (!spec) {
            const equivalent = bedrockEquivalent(argument.key)
            issues.push({
                level: equivalent ? 'error' : 'warning',
                message: equivalent
                    ? `Java 版没有 ${argument.key}=，请改用 ${equivalent}=`
                    : `未知参数 ${argument.key}=，会原样保留`,
                argument: argument.key,
            })
            continue
        }

        if (spec.forbiddenOn?.includes(parsed.variable)) {
            issues.push({
                level: 'error',
                message: `@${parsed.variable} 不能用 ${argument.key}=，改用 @e 或 @n`,
                argument: argument.key,
            })
        }

        if (argument.key === 'type') {
            if (argument.negated) negativeType += 1
            else positiveType += 1
        }

        if (spec.kind === 'unsigned-range' || spec.kind === 'range' || spec.kind === 'number') {
            const range = parseNumericRange(argument.value)
            if (!range) {
                issues.push({
                    level: 'error',
                    message: `${argument.key}= 的值不是数字或区间`,
                    argument: argument.key,
                })
                continue
            }
            if (spec.min !== undefined && (range.min ?? 0) < spec.min) {
                issues.push({
                    level: 'error',
                    message: `${argument.key}= 不能小于 ${spec.min}`,
                    argument: argument.key,
                })
            }
            if (spec.max !== undefined && (range.max ?? 0) > spec.max) {
                issues.push({
                    level: 'error',
                    message: `${argument.key}= 不能大于 ${spec.max}`,
                    argument: argument.key,
                })
            }
        }

        if (spec.kind === 'enum' && !argument.negated) {
            const value = argument.value.trim()
            if (spec.enumValues && !spec.enumValues.includes(value)) {
                issues.push({
                    level: 'error',
                    message: `${argument.key}= 只能是 ${spec.enumValues.join(' / ')}`,
                    argument: argument.key,
                })
            }
        }
    }

    if (positiveType > 1) {
        issues.push({ level: 'error', message: 'type= 只能写一次', argument: 'type' })
    }
    if (positiveType > 0 && negativeType > 0) {
        issues.push({
            level: 'error',
            message: 'type=<实体> 与 type=!<实体> 不能同时出现',
            argument: 'type',
        })
    }

    for (const [key, count] of seen) {
        const spec = selectorArgumentSpec(key)
        if (!spec || spec.repeatable || count <= 1) continue
        if (key === 'type') continue
        issues.push({ level: 'error', message: `${key}= 不能重复`, argument: key })
    }

    if (seen.has('sort') && !seen.has('limit')) {
        issues.push({
            level: 'warning',
            message: '没有 limit= 时 sort= 不影响选中的数量',
            argument: 'sort',
        })
    }
    if (
        seen.has('limit') &&
        !seen.has('sort') &&
        parsed.variable !== 'p' &&
        parsed.variable !== 'r'
    ) {
        issues.push({
            level: 'warning',
            message: '没有 sort= 时按 arbitrary 取前几个目标',
            argument: 'limit',
        })
    }

    return issues
}
