/**
 * 目标选择器的拼装与编辑。
 *
 * 选择器参数的顺序不影响语义，但影响可读性。这里固定成 wiki 汇总表的顺序
 * （空间 → 计分板 → 物种 → 实体数据 → 玩家数据 → 数量），
 * 用户手写的未知参数排到最后并保持原有先后。
 */

import {
    argumentsForVariable,
    SELECTOR_ARGUMENTS,
    selectorArgumentSpec,
    type SelectorVariable,
} from './arguments.ts'
import { formatNumericRange, type NumericRange, parseSelector } from './parse.ts'

export interface SelectorArgument {
    key: string
    /** 值，含 `!` 前缀。 */
    value: string
}

export interface SelectorDraft {
    variable: SelectorVariable
    arguments: SelectorArgument[]
}

const ORDER = new Map(SELECTOR_ARGUMENTS.map((spec, index) => [spec.key, index]))

function orderOf(key: string): number {
    return ORDER.get(key) ?? SELECTOR_ARGUMENTS.length
}

/** 把草稿拼成 `@e[type=zombie,limit=3]`。没有参数时只输出变量。 */
export function serializeSelector(draft: SelectorDraft): string {
    const args = draft.arguments.filter((argument) => argument.key.trim())
    if (args.length === 0) return `@${draft.variable}`
    const ordered = args
        .map((argument, index) => ({ argument, index }))
        .sort(
            (left, right) =>
                orderOf(left.argument.key) - orderOf(right.argument.key) ||
                left.index - right.index,
        )
        .map(({ argument }) => argument)
    const body = ordered.map((argument) => `${argument.key}=${argument.value}`).join(',')
    return `@${draft.variable}[${body}]`
}

/** 从已有文本得到可编辑的草稿；认不出的参数原样带过来。 */
export function draftFromSelector(text: string): SelectorDraft | undefined {
    const parsed = parseSelector(text)
    if (!parsed) return undefined
    return {
        variable: parsed.variable,
        arguments: parsed.arguments.map((argument) => ({
            key: argument.key,
            value: argument.value,
        })),
    }
}

/** 设置一个参数：已有同名参数时替换第一个，否则追加。 */
export function setArgument(draft: SelectorDraft, key: string, value: string): SelectorDraft {
    const existing = draft.arguments.findIndex((argument) => argument.key === key)
    if (existing === -1) {
        return { ...draft, arguments: [...draft.arguments, { key, value }] }
    }
    const next = [...draft.arguments]
    next[existing] = { key, value }
    return { ...draft, arguments: next }
}

export function removeArgument(draft: SelectorDraft, key: string, index?: number): SelectorDraft {
    if (index === undefined) {
        return { ...draft, arguments: draft.arguments.filter((argument) => argument.key !== key) }
    }
    return { ...draft, arguments: draft.arguments.filter((_, position) => position !== index) }
}

export function toggleNegated(value: string): string {
    return value.startsWith('!') ? value.slice(1) : `!${value}`
}

/** 区间值：`10`、`8..16`、`..10`、`10..`。 */
export function rangeValue(range: NumericRange, negated = false): string {
    const text = formatNumericRange(range)
    return negated ? `!${text}` : text
}

/** 该变量下允许出现的参数名，用于在卡片里隐藏不合法的项。 */
export function allowedKeys(variable: SelectorVariable): Set<string> {
    return new Set(argumentsForVariable(variable).map((spec) => spec.key))
}

/**
 * 从选择器推导 NBT 面板的实体上下文。
 *
 * 只有 `type=<单个实体>` 能确定字段集合；`type=!zombie`、实体类型标签
 * （`#minecraft:undead`）和 `@a` 这类只看玩家的变量都退回通用字段。
 */
export function nbtTargetFromSelector(text: string): string | undefined {
    const parsed = parseSelector(text)
    if (!parsed) return undefined
    if (parsed.variable === 'a' || parsed.variable === 'p' || parsed.variable === 'r') {
        return 'player'
    }
    const types = parsed.arguments.filter((argument) => argument.key === 'type')
    if (types.length !== 1) return undefined
    const [type] = types
    if (type.negated) return undefined
    const value = type.value.trim()
    if (!value || value.startsWith('#') || value.includes(',')) return undefined
    return value.replace(/^minecraft:/, '')
}

/** 玩家专属参数会让非玩家目标被排除，UI 用它提示「这会把 @e 收窄成玩家」。 */
export function filtersToPlayers(draft: SelectorDraft): boolean {
    return draft.arguments.some((argument) => {
        const spec = selectorArgumentSpec(argument.key)
        return spec?.playersOnly === true
    })
}
