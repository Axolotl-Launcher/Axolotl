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
    /** 值在原始文本里的区间，用于就地替换。 */
    start: number
    end: number
    /**
     * 整个 `key=value` 段在原文里的区间（首尾空白已排除），用于删除。
     * 值可以为空（`@e[distance=]`），那时 `start === end`，拿它删会什么都没删掉。
     */
    partStart: number
    partEnd: number
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
        const lead = part.text.length - part.text.trimStart().length
        const tail = part.text.length - part.text.trimEnd().length
        const partStart = shift + offset + part.start + lead
        const partEnd = shift + offset + part.end - tail
        // `eq` 是相对整段（含前导空白）的下标，和 `partStart` 不能直接相加
        const equalsIndex = shift + offset + part.start + eq
        const afterEquals = part.text.slice(eq + 1)
        const valueLead = afterEquals.length - afterEquals.trimStart().length
        result.arguments.push({
            key,
            value: rawValue,
            negated,
            // `key=   ` 这种只有空白的值：区间要收在段尾，否则 start 会越过 end
            start: Math.min(equalsIndex + 1 + valueLead, partEnd),
            end: partEnd,
            partStart,
            partEnd,
        })
    }
    return result
}

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
        const parts = text.split('..')
        // 多写一个点（`1..2..3`）不是区间：只取前两段等于悄悄接受非法值
        if (parts.length !== 2) return undefined
        const [left, right] = parts
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

export interface CompletionContext {
    variable: SelectorVariable
    /** 光标是否在 `[...]` 内。 */
    insideBrackets: boolean
    /** 当前正在输入的参数名（不含值）。 */
    prefix: string
    /** 光标前已写好的参数名，用于去重。只统计「取值相等」的出现：`name=!a` 之后仍可再写 `name=`。 */
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
        // wiki 允许 `name=!a,name=!b`：取反过的出现不占用候选
        .filter(
            (part) =>
                !part.text
                    .slice(part.text.indexOf('=') + 1)
                    .trim()
                    .startsWith('!'),
        )
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

/**
 * 诊断的稳定标识。文案在 `messages.ts` 里按 ID 登记，这里只给 ID 与参数——
 * 解析层是纯函数，不依赖 i18n 运行时，也不需要知道当前语言。
 */
export type SelectorIssueId =
    | 'not-a-selector'
    | 'unknown-variable'
    | 'unclosed-brackets'
    | 'bedrock-argument'
    | 'unknown-argument'
    | 'forbidden-on-variable'
    | 'not-a-number'
    | 'not-a-single-number'
    | 'not-negatable'
    | 'below-minimum'
    | 'above-maximum'
    | 'invalid-enum'
    | 'duplicate-type'
    | 'type-conflict'
    | 'duplicate-argument'
    | 'sort-without-limit'
    | 'limit-without-sort'

export interface SelectorIssue {
    /** `error` 阻断生成，`warning` 只是提醒。 */
    level: 'error' | 'warning'
    id: SelectorIssueId
    /** ICU 消息参数，例如 `{ key: 'distance', min: 0 }`。 */
    values?: Record<string, string | number>
    argument?: string
}

/**
 * 检查选择器是否合法。规则来自 wiki：
 * - `type` 不能用在 `@a` / `@p` / `@r` 上；
 * - 不可重复的参数不能出现两次（`tag`、`nbt`、`predicate` 可以）；
 * - 取值「相等」的同一参数只能出现一次，取「不相等」的可以重复
 *   （`@e[name=!a,name=!b]` 合法，`@e[name=a,name=b]` 不合法）；
 * - `type=<id>` 只能出现一次，且不能与 `type=!<id>` 同时出现；
 * - 数值参数的每一个写出来的边界都要落在允许区间内（`distance=..-5` 不合法）；
 * - `sort` 与 `limit` 相互影响，缺一个时说明默认行为；
 * - 基岩版参数给出对应写法。
 */
export function validateSelector(parsed: ParsedSelector): SelectorIssue[] {
    const issues: SelectorIssue[] = []

    if (parsed.unknownVariable) {
        issues.push({
            level: 'error',
            id: 'unknown-variable',
            values: { variable: parsed.unknownVariable },
        })
    }
    if (parsed.error === 'unclosed') {
        issues.push({ level: 'error', id: 'unclosed-brackets' })
    }

    /** 每个参数出现的总次数，以及其中「取值相等」的次数。 */
    const counts = new Map<string, { total: number; positive: number }>()
    let positiveType = 0
    let negativeType = 0

    for (const argument of parsed.arguments) {
        const spec = selectorArgumentSpec(argument.key)
        const count = counts.get(argument.key) ?? { total: 0, positive: 0 }
        count.total += 1
        if (!argument.negated) count.positive += 1
        counts.set(argument.key, count)

        if (!spec) {
            const equivalent = bedrockEquivalent(argument.key)
            issues.push(
                equivalent
                    ? {
                          level: 'error',
                          id: 'bedrock-argument',
                          values: { key: argument.key, equivalent },
                          argument: argument.key,
                      }
                    : {
                          level: 'warning',
                          id: 'unknown-argument',
                          values: { key: argument.key },
                          argument: argument.key,
                      },
            )
            continue
        }

        if (spec.forbiddenOn?.includes(parsed.variable)) {
            issues.push({
                level: 'error',
                id: 'forbidden-on-variable',
                values: { variable: parsed.variable, key: argument.key },
                argument: argument.key,
            })
        }

        if (argument.key === 'type') {
            if (argument.negated) negativeType += 1
            else positiveType += 1
        }

        // 只有标了 `negatable` 的参数能加 `!`：`limit=!3`、`sort=!random` 游戏会拒绝
        if (argument.negated && !spec.negatable) {
            issues.push({
                level: 'error',
                id: 'not-negatable',
                values: { key: argument.key },
                argument: argument.key,
            })
        }

        if (spec.kind === 'unsigned-range' || spec.kind === 'range' || spec.kind === 'number') {
            const range = parseNumericRange(argument.value)
            if (!range) {
                issues.push({
                    level: 'error',
                    id: 'not-a-number',
                    values: { key: argument.key },
                    argument: argument.key,
                })
                continue
            }
            // `number` 参数只吃一个数，`limit=3..5` 游戏不认
            if (spec.kind === 'number' && range.min !== range.max) {
                issues.push({
                    level: 'error',
                    id: 'not-a-single-number',
                    values: { key: argument.key },
                    argument: argument.key,
                })
                continue
            }
            // 区间只写了一边时，另一边就是全部范围：用已定义的那一边判断越界
            const lowest = range.min ?? range.max
            const highest = range.max ?? range.min
            if (spec.min !== undefined && lowest !== undefined && lowest < spec.min) {
                issues.push({
                    level: 'error',
                    id: 'below-minimum',
                    values: { key: argument.key, min: spec.min },
                    argument: argument.key,
                })
            }
            if (spec.max !== undefined && highest !== undefined && highest > spec.max) {
                issues.push({
                    level: 'error',
                    id: 'above-maximum',
                    values: { key: argument.key, max: spec.max },
                    argument: argument.key,
                })
            }
        }

        if (spec.kind === 'enum') {
            // 取反的枚举（`type=!zombie`）去掉 `!` 再比对，不能整个跳过检查
            const value = argument.value.trim().replace(/^!/, '')
            if (spec.enumValues && !spec.enumValues.includes(value)) {
                issues.push({
                    level: 'error',
                    id: 'invalid-enum',
                    values: { key: argument.key, values: spec.enumValues.join(' / ') },
                    argument: argument.key,
                })
            }
        }
    }

    if (positiveType > 1) {
        issues.push({ level: 'error', id: 'duplicate-type', argument: 'type' })
    }
    if (positiveType > 0 && negativeType > 0) {
        issues.push({ level: 'error', id: 'type-conflict', argument: 'type' })
    }

    for (const [key, count] of counts) {
        const spec = selectorArgumentSpec(key)
        if (!spec || spec.repeatable || count.total <= 1) continue
        if (key === 'type') continue
        // wiki：测试「不相等」的参数可以重复，测试「相等」的只能出现一次
        if (count.positive <= 1) continue
        issues.push({
            level: 'error',
            id: 'duplicate-argument',
            values: { key },
            argument: key,
        })
    }

    if (counts.has('sort') && !counts.has('limit')) {
        issues.push({ level: 'warning', id: 'sort-without-limit', argument: 'sort' })
    }
    if (
        counts.has('limit') &&
        !counts.has('sort') &&
        parsed.variable !== 'p' &&
        parsed.variable !== 'r'
    ) {
        issues.push({ level: 'warning', id: 'limit-without-sort', argument: 'limit' })
    }

    return issues
}
