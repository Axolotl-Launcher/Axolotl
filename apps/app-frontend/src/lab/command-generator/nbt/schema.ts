/**
 * NBT 字段目录：把「某个实体 / 物品 / 方块实体在这个版本上有哪些字段」变成数据。
 *
 * 数据来自 Minecraft Wiki（`Entity format`、`Data component format`、
 * `Block entity format`），每个数据文件头部记录了页面与修订号。
 * 这里只做查询与版本门控，不写死任何游戏知识。
 */

import type { NbtTagType } from './model.ts'

export type NbtDomain = 'entity' | 'item' | 'block_entity'

export type NbtCategory =
    | 'basic'
    | 'appearance'
    | 'combat'
    | 'ai'
    | 'movement'
    | 'inventory'
    | 'effect'
    | 'data'
    | 'advanced'

/** 字段的适用范围：全部、按 id、按「除了这些」，或按分组（分组定义在同一文件里）。 */
export interface NbtAppliesTo {
    any?: boolean
    ids?: string[]
    except?: string[]
    group?: string
}

export interface NbtLegacyKey {
    key: string
    until: string
}

export interface NbtFieldSpec {
    key: string
    type: NbtTagType
    /** 列表元素类型；未知时由第一个子节点决定。 */
    elementType?: NbtTagType
    /** 游戏用 byte 表示布尔，界面应给开关而不是数字框。 */
    valueKind?: 'boolean'
    /** SNBT 文本形式的默认值，例如 `1b`、`80`、`""`。 */
    default?: string
    category: NbtCategory
    advanced?: boolean
    since?: string
    until?: string
    /** 旧版本用过的键名，按 `until` 判定。 */
    legacyKeys?: NbtLegacyKey[]
    appliesTo?: NbtAppliesTo
    /** 结构化编辑器标识，例如 `enchantments`。 */
    editor?: string
    desc?: string
    /** 中文名，用于中文搜索与辅助显示。 */
    zh?: string
    children?: NbtFieldSpec[]
}

export interface NbtCatalogFile {
    schemaVersion: number
    domain: NbtDomain
    retrievedAt: string
    sources: Array<{ page: string; revision: number }>
    groups?: Record<string, NbtAppliesTo>
    fields: NbtFieldSpec[]
}

/** 使用目录的上下文：知道具体实体 / 物品时才能做「仅显示适用」过滤。 */
export interface NbtContext {
    domain: NbtDomain
    /** 实体 ID、物品 ID 或方块实体 ID。 */
    targetId?: string
    version: string
}

/** 版本比较：`26.1` 要大于 `1.21`，所以必须按数字段比较。 */
export function compareVersion(left: string, right: string): number {
    return left.localeCompare(right, undefined, { numeric: true })
}

export function isVersionInRange(
    spec: Pick<NbtFieldSpec, 'since' | 'until'>,
    version: string,
): boolean {
    if (spec.since && compareVersion(version, spec.since) < 0) return false
    if (spec.until && compareVersion(version, spec.until) > 0) return false
    return true
}

/** 该版本下这个字段实际使用的键名（旧版本可能叫别的名字）。 */
export function keyForVersion(spec: NbtFieldSpec, version: string): string {
    for (const legacy of spec.legacyKeys ?? []) {
        if (compareVersion(version, legacy.until) <= 0) return legacy.key
    }
    return spec.key
}

function matchesAppliesTo(appliesTo: NbtAppliesTo, targetId: string | undefined): boolean {
    if (appliesTo.any) return true
    if (!targetId) return true
    if (appliesTo.except) return !appliesTo.except.includes(targetId)
    if (appliesTo.ids) return appliesTo.ids.includes(targetId)
    return true
}

/** 字段是否适用于当前上下文。分组会先展开成 id 集合。 */
export function fieldAppliesTo(
    spec: NbtFieldSpec,
    context: Pick<NbtContext, 'targetId'>,
    groups: Record<string, NbtAppliesTo> = {},
): boolean {
    const appliesTo = spec.appliesTo
    if (!appliesTo) return true
    if (appliesTo.group) {
        const group = groups[appliesTo.group]
        if (!group) return true
        return matchesAppliesTo(group, context.targetId)
    }
    return matchesAppliesTo(appliesTo, context.targetId)
}

/**
 * 过滤出当前版本、当前目标可用的字段。
 *
 * `onlyApplicable` 为 false 时只按版本过滤，用来实现「显示全部」。
 */
export function availableFields(
    fields: NbtFieldSpec[],
    context: NbtContext,
    options: { onlyApplicable?: boolean; groups?: Record<string, NbtAppliesTo> } = {},
): NbtFieldSpec[] {
    const onlyApplicable = options.onlyApplicable ?? true
    return fields.filter(
        (spec) =>
            isVersionInRange(spec, context.version) &&
            (!onlyApplicable || !context.targetId || fieldAppliesTo(spec, context, options.groups)),
    )
}

/** 把 `fall_distance` / `CustomName` 这类键名变成可读标签。 */
const ACRONYMS = new Map([
    ['uuid', 'UUID'],
    ['id', 'ID'],
    ['ai', 'AI'],
    ['nbt', 'NBT'],
])

export function humanizeKey(key: string): string {
    if (!key) return ''
    const acronym = ACRONYMS.get(key.toLowerCase())
    if (acronym) return acronym
    const spaced = key
        .replace(/[_-]+/g, ' ')
        .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
        .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
        .trim()
        .toLowerCase()
    if (!spaced) return key
    return spaced.charAt(0).toUpperCase() + spaced.slice(1)
}

export interface NbtSearchResult {
    spec: NbtFieldSpec
    score: number
}

/**
 * 搜索字段：键名优先，其次中文名 / 英文名，最后是说明。
 *
 * 支持空格分词，所有词都要命中（顺序不限）。
 */
export function searchFields(
    fields: NbtFieldSpec[],
    query: string,
    version: string,
): NbtSearchResult[] {
    const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean)
    const results: NbtSearchResult[] = []

    for (const spec of fields) {
        if (!isVersionInRange(spec, version)) continue
        const key = keyForVersion(spec, version).toLowerCase()
        const label = humanizeKey(spec.key).toLowerCase()
        const zh = (spec.zh ?? '').toLowerCase()
        const desc = (spec.desc ?? '').toLowerCase()

        let score = 0
        let matchedAll = true
        for (const term of terms) {
            if (key === term) score += 100
            else if (key.startsWith(term)) score += 60
            else if (key.includes(term)) score += 30
            else if (zh.startsWith(term)) score += 45
            else if (zh.includes(term)) score += 25
            else if (label.startsWith(term)) score += 40
            else if (label.includes(term)) score += 20
            else if (desc.includes(term)) score += 5
            else matchedAll = false
        }

        if (!matchedAll) continue
        if (terms.length === 0) score = 1
        results.push({ spec, score })
    }

    return results.sort(
        (left, right) => right.score - left.score || left.spec.key.localeCompare(right.spec.key),
    )
}

/** 结果按分类分组，保持目录里的原始顺序。 */
export const CATEGORY_ORDER: readonly NbtCategory[] = [
    'basic',
    'appearance',
    'combat',
    'movement',
    'ai',
    'inventory',
    'effect',
    'data',
    'advanced',
]

export function groupByCategory(
    results: NbtSearchResult[],
): Array<{ category: NbtCategory; items: NbtFieldSpec[] }> {
    const buckets = new Map<NbtCategory, NbtFieldSpec[]>()
    for (const { spec } of results) {
        const list = buckets.get(spec.category) ?? []
        list.push(spec)
        buckets.set(spec.category, list)
    }
    return CATEGORY_ORDER.filter((category) => buckets.has(category)).map((category) => ({
        category,
        items: buckets.get(category)!,
    }))
}
