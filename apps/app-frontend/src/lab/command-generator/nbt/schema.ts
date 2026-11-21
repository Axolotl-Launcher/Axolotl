/**
 * NBT 字段目录：把「某个实体 / 物品 / 方块实体在这个版本上有哪些字段」变成数据。
 *
 * 数据来自 Minecraft Wiki（`Entity format`、`Data component format`、
 * `Block entity format`），每个数据文件头部记录了页面与修订号。
 * 这里只做查询与版本门控，不写死任何游戏知识。
 *
 * 一个字段往往有**多种合法写法**：`banner_patterns[].pattern` 既可以写图案 ID
 * 字符串，也可以写 `{asset_id, translation_key}` 定义；`potion_contents` 既可以
 * 写药水 ID，也可以写复合标签。所以字段的形状不是单一 `type`，而是 `forms`
 * ——按版本筛选后第一个就是新建时的默认写法。
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

/** 数组 / 列表的长度约束。`exactly` 是定长（`Pos` 是 3），其余是范围。 */
export interface NbtLength {
    exactly?: number
    min?: number
    max?: number
}

/**
 * 一种合法写法。
 *
 * - 标量：`{ type: 'string' }`
 * - 数组：`{ type: 'int_array', length: { exactly: 4 } }`
 * - 复合：`{ type: 'compound', fields: [...] }`
 * - 列表：`{ type: 'list', element: { type: 'compound', fields: [...] } }`
 */
export interface NbtValueForm {
    type: NbtTagType
    /** 列表元素写法。 */
    element?: NbtValueForm
    /** 复合（或列表元素为复合）的子字段。 */
    fields?: NbtFieldSpec[]
    length?: NbtLength
    /** 游戏用 byte 表示布尔，界面应给开关而不是数字框。 */
    valueKind?: 'boolean'
    /** 枚举来源 id（见 `values.ts`），例如 `dye_color`、`banner_pattern`。 */
    values?: string
    /** SNBT 文本形式的默认值，例如 `1b`、`80`、`"minecraft:cross"`。 */
    default?: string
    /** 这一种写法自己的说明（wiki 常把两种写法分开描述）。 */
    desc?: string
    since?: string
    until?: string
    /** 结构化编辑器标识，例如 `enchantments`。 */
    editor?: string
}

export interface NbtFieldSpec {
    key: string
    category: NbtCategory
    advanced?: boolean
    since?: string
    until?: string
    /** 旧版本用过的键名，按 `until` 判定。 */
    legacyKeys?: NbtLegacyKey[]
    appliesTo?: NbtAppliesTo
    /** 中文名，用于中文搜索与辅助显示。 */
    zh?: string
    desc?: string
    /** 合法写法；第一个是新建时的默认写法。 */
    forms: NbtValueForm[]
}

/** 目录 JSON 里的字段：v1 用 `type` + `children`，v2 直接写 `forms`。 */
export interface NbtFieldInput extends Omit<NbtFieldSpec, 'forms'> {
    type?: NbtTagType
    elementType?: NbtTagType
    length?: number | NbtLength
    valueKind?: 'boolean'
    default?: string
    editor?: string
    children?: NbtFieldInput[]
    forms?: NbtFormInput[]
}

export interface NbtFormInput extends Omit<NbtValueForm, 'element' | 'fields' | 'length'> {
    element?: NbtFormInput
    elementType?: NbtTagType
    fields?: NbtFieldInput[]
    children?: NbtFieldInput[]
    length?: number | NbtLength
}

export interface NbtCatalogFile {
    schemaVersion: number
    domain: NbtDomain
    retrievedAt: string
    sources: Array<{ page: string; revision: number }>
    groups?: Record<string, NbtAppliesTo>
    fields: NbtFieldSpec[]
}

export interface NbtCatalogInput extends Omit<NbtCatalogFile, 'fields'> {
    fields: NbtFieldInput[]
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
    spec: Pick<NbtValueForm, 'since' | 'until'>,
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

function normalizeLength(length: number | NbtLength | undefined): NbtLength | undefined {
    if (length === undefined) return undefined
    if (typeof length === 'number') return { exactly: length }
    return length
}

function normalizeForm(input: NbtFormInput): NbtValueForm {
    const form: NbtValueForm = { type: input.type }
    if (input.values) form.values = input.values
    if (input.valueKind) form.valueKind = input.valueKind
    if (input.default !== undefined) form.default = input.default
    if (input.desc) form.desc = input.desc
    if (input.since) form.since = input.since
    if (input.until) form.until = input.until
    if (input.editor) form.editor = input.editor

    const length = normalizeLength(input.length)
    if (length) form.length = length

    const fields = input.fields ?? input.children
    if (input.type === 'compound') {
        if (fields?.length) form.fields = fields.map(normalizeField)
        return form
    }

    if (input.type === 'list') {
        if (input.element) form.element = normalizeForm(input.element)
        else if (input.elementType === 'compound') {
            form.element = { type: 'compound' }
            if (fields?.length) form.element.fields = fields.map(normalizeField)
        } else if (input.elementType) form.element = { type: input.elementType }
        return form
    }

    return form
}

/**
 * 把目录里的一层归一化成 `forms`。
 *
 * 显式写了 `forms` 的字段原样展开；只写了 `type` 的旧数据归一成单一写法，
 * 这样三个目录不必一次性重写。
 */
export function normalizeField(input: NbtFieldInput): NbtFieldSpec {
    const field: NbtFieldSpec = {
        key: input.key,
        category: input.category,
        forms: [],
    }
    if (input.advanced) field.advanced = input.advanced
    if (input.since) field.since = input.since
    if (input.until) field.until = input.until
    if (input.legacyKeys?.length) field.legacyKeys = input.legacyKeys
    if (input.appliesTo) field.appliesTo = input.appliesTo
    if (input.zh) field.zh = input.zh
    if (input.desc) field.desc = input.desc

    if (input.forms?.length) {
        field.forms = input.forms.map(normalizeForm)
        return field
    }

    if (!input.type) {
        throw new Error(`NBT 目录字段 ${input.key} 既没有 forms 也没有 type`)
    }

    field.forms = [
        normalizeForm({
            type: input.type,
            elementType: input.elementType,
            children: input.children,
            length: input.length,
            valueKind: input.valueKind,
            default: input.default,
            editor: input.editor,
        }),
    ]
    return field
}

export function normalizeCatalog(input: NbtCatalogInput): NbtCatalogFile {
    return {
        schemaVersion: 2,
        domain: input.domain,
        retrievedAt: input.retrievedAt,
        sources: input.sources,
        groups: input.groups,
        fields: input.fields.map(normalizeField),
    }
}

/** 该版本下这个字段还剩下哪些写法；空数组表示这个字段在版本里不存在。 */
export function formsFor(spec: NbtFieldSpec, version: string): NbtValueForm[] {
    if (!isVersionInRange(spec, version)) return []
    return spec.forms.filter((form) => isVersionInRange(form, version))
}

/**
 * 取一种写法。`preferred` 是节点当前的标签类型：用户已经写成字符串的
 * `pattern` 不该被当成复合标签处理。
 */
export function formFor(
    spec: NbtFieldSpec,
    version: string,
    preferred?: NbtTagType,
): NbtValueForm | undefined {
    const forms = formsFor(spec, version)
    if (preferred) {
        const match = forms.find((form) => form.type === preferred)
        if (match) return match
    }
    return forms[0]
}

/** 写法里的子字段：复合取 `fields`，列表取元素的 `fields`。 */
export function childrenOf(form: NbtValueForm | undefined): NbtFieldSpec[] {
    if (!form) return []
    if (form.type === 'list') return form.element?.fields ?? []
    return form.fields ?? []
}

/** 列表的元素写法；非列表返回 `undefined`。 */
export function elementOf(form: NbtValueForm | undefined): NbtValueForm | undefined {
    return form?.type === 'list' ? form.element : undefined
}

/** 定长约束；没有约束返回 `undefined`。 */
export function exactLengthOf(form: NbtValueForm | undefined): number | undefined {
    return form?.length?.exactly
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
    return dedupeFields(
        fields.filter(
            (spec) =>
                formsFor(spec, context.version).length > 0 &&
                (!onlyApplicable ||
                    !context.targetId ||
                    fieldAppliesTo(spec, context, options.groups)),
        ),
    )
}

/** 形态签名：形状真的不同的同名键要分开保留，形状相同的才算重复。 */
function formSignature(form: NbtValueForm): string {
    if (form.type === 'list') {
        return `list<${form.element ? formSignature(form.element) : ''}>`
    }
    if (form.type === 'compound') {
        return `compound(${(form.fields ?? []).map((field) => field.key).join('+')})`
    }
    return form.type
}

/**
 * 去掉同名同形的重复字段。
 *
 * 目录按 `appliesTo` 存多条：`Item` 在 12 个投掷物实体下各有一条，`Items` 在
 * 驴 / 羊驼 / 骡 / 行商羊驼和容器实体下共 15 条。知道具体目标时会被过滤掉，
 * 不知道目标时（`/data` 的坐标、没有 `type=` 的选择器）它们除了隐藏的适用范围
 * 之外完全一样，全列出来就是同一行重复十几遍。
 */
export function dedupeFields(fields: NbtFieldSpec[]): NbtFieldSpec[] {
    const seen = new Set<string>()
    const result: NbtFieldSpec[] = []
    for (const field of fields) {
        const signature = `${field.key}|${field.forms.map(formSignature).join(',')}`
        if (seen.has(signature)) continue
        seen.add(signature)
        result.push(field)
    }
    return result
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
        if (formsFor(spec, version).length === 0) continue
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
