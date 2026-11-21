/**
 * SNBT 补全的候选来源：把 NBT 目录、版本与目标物品 / 实体接进 `@modrinth/ui`
 * 的补全管线。
 *
 * 语言本身（词法、括号、缩进、悬浮列表）在 UI 包里，这里只回答「这个位置有
 * 哪些合法写法」。所以同一套补全可以给命令生成器用，也可以给 Studio 用，
 * 差别只在传进来的目录与 ID 表。
 */

import type { SnbtCandidate, SnbtCompletionSource, SnbtScan, SnbtSprite } from '@modrinth/ui'

import potionsData from '../../shared/minecraft/assets/vanilla/potions.json' with { type: 'json' }
import { resolveRecipeItemName } from '../../shared/minecraft/item-names.ts'
import { loadItems, TEXTURE_ATLAS, TEXTURE_ATLAS_SIZE } from '../../shared/minecraft/resources.ts'
import type { JavaVersionId } from '../../shared/minecraft/types.ts'
import { compareMinecraftVersions, JAVA_VERSIONS } from '../../shared/minecraft/versions.ts'
import effectsData from '../data/effects.json' with { type: 'json' }
import enchantmentsData from '../data/enchantments.json' with { type: 'json' }
import entitiesData from '../data/entities.json' with { type: 'json' }
import { groupsFor, levelAtPath, type NbtLevel, type NbtLookup, rootFieldsFor } from './catalog.ts'
import {
    fieldDescription,
    fieldLabel,
    type MessageFormatter,
    NBT_TYPE_NAME,
    NBT_TYPE_SHORT,
} from './labels.ts'
import type { NbtTagType } from './model.ts'
import {
    availableFields,
    formFor,
    formsFor,
    humanizeKey,
    keyForVersion,
    type NbtFieldSpec,
    type NbtValueForm,
} from './schema.ts'
import { quoteSnbt } from './snbt.ts'
import { valueOptions } from './values.ts'

export interface NbtCompletionContext {
    /** 没有目录可查时（例如 `/data` 的 storage）为 `null`，只给形态候选。 */
    lookup: NbtLookup | null
    version: string
    locale: string
    formatMessage: MessageFormatter
}

/** 没有目录时也能写的容器：让用户先打出骨架，再往里填。 */
const GENERIC_SHAPES: SnbtCandidate[] = [
    {
        label: '{ … }',
        insertText: '{$0}',
        snippet: true,
        kind: 'snippet',
        detail: 'Compound',
        score: 40,
    },
    {
        label: '[ … ]',
        insertText: '[$0]',
        snippet: true,
        kind: 'snippet',
        detail: 'List',
        score: 38,
    },
    { label: '""', insertText: '""', kind: 'value', detail: 'String', score: 36 },
]

const SCALAR_LITERALS: Partial<Record<NbtTagType, string>> = {
    byte: '0b',
    short: '0s',
    int: '0',
    long: '0L',
    float: '0.0f',
    double: '0.0d',
    string: '""',
}

const ARRAY_LITERALS: Partial<Record<NbtTagType, string>> = {
    byte_array: '[B;$0]',
    int_array: '[I;$0]',
    long_array: '[L;$0]',
}

type IdSource = 'item' | 'entity' | 'potion' | 'enchantment' | 'effect'

const ITEM_PARENTS = new Set(['Items', 'Item', 'item', 'equipment', 'Inventory', 'item_stack'])
const EFFECT_PARENTS = new Set(['Effects', 'effects', 'active_effects', 'custom_effects'])

/** 版本号落到最近的资源快照上（物品清单只按大版本打包）。 */
function resolveResourceVersion(version: string): JavaVersionId {
    const match = JAVA_VERSIONS.find((meta) => compareMinecraftVersions(meta.id, version) <= 0)
    return (match ?? JAVA_VERSIONS[JAVA_VERSIONS.length - 1]).id
}

function spriteFor(texture: string | null | undefined): SnbtSprite | undefined {
    if (!texture) return undefined
    const region = TEXTURE_ATLAS.layout[texture]
    if (!region) return undefined
    return {
        url: TEXTURE_ATLAS.url,
        x: region[0],
        y: region[1],
        width: region[2],
        height: region[3],
        sheetWidth: TEXTURE_ATLAS_SIZE.width,
        sheetHeight: TEXTURE_ATLAS_SIZE.height,
    }
}

function isChinese(locale: string): boolean {
    return locale.toLowerCase().startsWith('zh')
}

/**
 * 物品栈永远是 `{id, count}` 的形状：`Items`、`bundle_contents`、
 * `charged_projectiles`、`equipment.mainhand` 全都长这样。按这个形状判断，
 * 比一个个枚举父键名可靠，也不会把效果 / 方块状态里的 `id` 认成物品。
 *
 * 根层不算：实体根层就有叫 `count` 的标签（那是生物自己的字段），而根层的
 * `id` 是实体 / 方块实体的类型。
 */
function isItemStack(path: string[], level: NbtLevel | undefined, key: string): boolean {
    if (key !== 'id' || !path.length || !level) return false
    return level.fields.some((field) => field.key === 'count' || field.key === 'Count')
}

/**
 * 这个键的值是不是某一类 ID。
 *
 * 判据是键名加上父级路径：`Items` 里的 `id` 是物品，根层的 `id` 是实体，
 * 附魔表里的键名本身就是附魔 ID。
 */
function idSourceFor(
    domain: string | undefined,
    path: string[],
    key: string,
    level: NbtLevel | undefined,
): IdSource | undefined {
    if (isItemStack(path, level, key)) return 'item'
    // 这个组件也能直接写成实体 ID 字符串。
    if (key === 'entity_data') return 'entity'
    const parent = path[path.length - 1]
    if (parent === 'Enchantments') return 'enchantment'
    if (parent === 'levels' && path[path.length - 2] === 'enchantments') return 'enchantment'
    if (parent && EFFECT_PARENTS.has(parent)) {
        return key === 'id' || key === 'effect' ? 'effect' : undefined
    }
    if (key === 'potion' || key === 'Potion' || key === 'base_potion') return 'potion'
    if (key === 'effect' || key === 'primary_effect' || key === 'secondary_effect') return 'effect'
    if (key === 'Item' || key === 'item_id') return 'item'
    if (key !== 'id') return undefined
    if (parent && ITEM_PARENTS.has(parent)) return 'item'
    if (parent === 'entity' || parent === 'entity_data' || parent === 'EntityTag') return 'entity'
    if (!parent && domain === 'entity') return 'entity'
    return undefined
}

function findSpec(fields: NbtFieldSpec[], key: string, version: string): NbtFieldSpec | undefined {
    return fields.find(
        (spec) =>
            keyForVersion(spec, version) === key ||
            spec.key === key ||
            (spec.legacyKeys ?? []).some((legacy) => legacy.key === key),
    )
}

function shapeCandidate(form: NbtValueForm): SnbtCandidate {
    if (form.type === 'compound') {
        return {
            label: '{ … }',
            insertText: '{$0}',
            snippet: true,
            kind: 'snippet',
            detail: NBT_TYPE_NAME.compound,
            score: 60,
        }
    }
    if (form.type === 'list') {
        return {
            label: '[ … ]',
            insertText: '[$0]',
            snippet: true,
            kind: 'snippet',
            detail: NBT_TYPE_NAME.list,
            score: 58,
        }
    }
    const array = ARRAY_LITERALS[form.type]
    if (array) {
        return {
            label: array.replace('$0', ' … '),
            insertText: array,
            snippet: true,
            kind: 'snippet',
            detail: NBT_TYPE_NAME[form.type],
            score: 56,
        }
    }
    const literal = form.default ?? SCALAR_LITERALS[form.type] ?? '""'
    return {
        label: literal,
        insertText: literal,
        kind: 'value',
        detail: NBT_TYPE_NAME[form.type],
        score: 55,
    }
}

/**
 * 括号类写法要连括号一起补：只给 `banner_patterns=` 的话，用户还得自己敲
 * `[]` 再把光标挪进去，位置一错就补成了别的类型。字符串同理，直接给一对
 * 引号，光标停在中间，紧接着的候选会填进去（扫描器认得出光标在引号里）。
 * 数字、布尔没有括号，只补分隔符，值交给后面的候选。
 */
function keySuffix(form: NbtValueForm | undefined): string | undefined {
    if (!form) return undefined
    if (form.type === 'compound') return '{$0}'
    if (form.type === 'list') return '[$0]'
    const array = ARRAY_LITERALS[form.type]
    if (array) return array
    if (form.values || form.type === 'string') return '"$0"'
    return undefined
}

function valueCandidatesFor(form: NbtValueForm, version: string): SnbtCandidate[] {
    if (form.values) {
        return valueOptions(form.values, version).map((option) => ({
            label: option.label,
            insertText: quoteSnbt(option.value),
            kind: 'enum',
            detail: NBT_TYPE_SHORT.string,
            score: 70,
        }))
    }
    if (form.valueKind === 'boolean') {
        return [
            { label: 'true', insertText: '1b', kind: 'value', detail: 'Byte', score: 68 },
            { label: 'false', insertText: '0b', kind: 'value', detail: 'Byte', score: 67 },
        ]
    }
    return [shapeCandidate(form)]
}

function keyScore(key: string, prefix: string, spec: NbtFieldSpec, exists: boolean): number {
    let score = 50
    if (prefix) {
        const lower = key.toLowerCase()
        if (lower === prefix) score += 40
        else if (lower.startsWith(prefix)) score += 30
        else if (lower.includes(prefix)) score += 10
    }
    if (spec.advanced) score -= 15
    if (exists) score -= 20
    return Math.max(0, Math.min(100, score))
}

/** 显示用的短 ID：`minecraft:water` 写成 `water`。 */
function shortId(id: string): string {
    return id.replace(/^minecraft:/, '')
}

async function itemCandidates(version: JavaVersionId, locale: string): Promise<SnbtCandidate[]> {
    const items = await loadItems(version)
    return items.map((item) => ({
        label: item.id,
        detail: resolveRecipeItemName(item.id, locale, item.name),
        kind: 'item' as const,
        icon: spriteFor(item.texture),
    }))
}

async function effectCandidates(version: JavaVersionId, locale: string): Promise<SnbtCandidate[]> {
    const items = await loadItems(version)
    const byId = new Map(items.map((item) => [item.id, item]))
    return effectsData.map((effect) => {
        const icon = byId.get(`minecraft:${effect.icon}`)
        return {
            label: `minecraft:${effect.id}`,
            detail: isChinese(locale) ? undefined : humanizeKey(effect.id),
            kind: 'effect' as const,
            icon: spriteFor(icon?.texture),
        }
    })
}

async function entityCandidates(version: JavaVersionId, locale: string): Promise<SnbtCandidate[]> {
    const items = await loadItems(version)
    const byId = new Map(items.map((item) => [item.id, item]))
    return entitiesData.entities.map((entity) => ({
        label: `minecraft:${entity.id}`,
        detail: isChinese(locale) ? entity.zh : entity.en,
        kind: 'entity' as const,
        // 生物没有贴图，用刷怪蛋代替；没有刷怪蛋就退回 codicon。
        icon: spriteFor(byId.get(`minecraft:${entity.id}_spawn_egg`)?.texture),
    }))
}

function enchantmentCandidates(locale: string): SnbtCandidate[] {
    return enchantmentsData.enchantments.map((enchantment) => ({
        label: `minecraft:${enchantment.id}`,
        detail: isChinese(locale) ? enchantment.zh : enchantment.en,
        kind: 'enchantment' as const,
    }))
}

function potionCandidates(locale: string): SnbtCandidate[] {
    return potionsData.map((id) => ({
        label: id,
        detail: isChinese(locale) ? undefined : humanizeKey(shortId(id)),
        kind: 'potion' as const,
    }))
}

async function idCandidates(
    context: NbtCompletionContext,
    scan: SnbtScan,
    level: NbtLevel | undefined,
): Promise<SnbtCandidate[]> {
    const source = idSourceFor(context.lookup?.domain, scan.keyPath, scan.key, level)
    if (!source) return []
    const version = resolveResourceVersion(context.version)
    switch (source) {
        case 'item':
            return itemCandidates(version, context.locale)
        case 'effect':
            return effectCandidates(version, context.locale)
        case 'entity':
            return entityCandidates(version, context.locale)
        case 'enchantment':
            return enchantmentCandidates(context.locale)
        case 'potion':
            return potionCandidates(context.locale)
    }
}

function keyCandidates(context: NbtCompletionContext, scan: SnbtScan): SnbtCandidate[] {
    const lookup = context.lookup
    if (!lookup) return []
    const level = scan.keyPath.length
        ? levelAtPath(lookup, scan.keyPath)
        : { fields: rootFieldsFor(lookup) }
    if (!level) return []

    const fields = availableFields(
        level.fields,
        { domain: lookup.domain, version: lookup.version, targetId: lookup.targetId },
        { onlyApplicable: Boolean(lookup.targetId), groups: groupsFor(lookup.domain) },
    )
    const existing = new Set(scan.container?.keys ?? [])
    const prefix = scan.word.toLowerCase()
    const component = scan.expect === 'component'

    return fields.map((spec) => {
        const key = keyForVersion(spec, lookup.version)
        const form = formFor(spec, lookup.version)
        const suffix = keySuffix(form)
        const description = fieldDescription(spec, lookup.domain, context.formatMessage)
        const type = form ? NBT_TYPE_SHORT[form.type] : ''
        const label = fieldLabel(spec, context.locale)
        const separator = component ? '=' : ':'
        // 后面已经跟着分隔符时只补键名，不然会出现 `display::`。
        const typed = scan.next === separator
        return {
            label: key,
            insertText: typed ? key : `${key}${separator}${suffix ?? ''}`,
            snippet: !typed && Boolean(suffix),
            detail: type && label ? `${type} · ${label}` : type || label,
            documentation: description || undefined,
            kind: 'field' as const,
            score: keyScore(key, prefix, spec, existing.has(key)),
        }
    })
}

async function valueCandidates(
    context: NbtCompletionContext,
    scan: SnbtScan,
): Promise<SnbtCandidate[]> {
    const lookup = context.lookup
    const level: NbtLevel | undefined = lookup ? levelAtPath(lookup, scan.keyPath) : undefined
    const forms: NbtValueForm[] = []
    if (level) {
        if (scan.expect === 'element') {
            if (level.element) forms.push(level.element)
        } else {
            const spec = findSpec(level.fields, scan.key, context.version)
            if (spec) forms.push(...formsFor(spec, context.version))
        }
    }

    const ids = await idCandidates(context, scan, level)
    const base = forms.length
        ? forms.flatMap((form) => valueCandidatesFor(form, context.version))
        : GENERIC_SHAPES
    // 光标已经在引号里时，`""` 补进去只会得到一对空引号；有 ID 列表时它也只是
    // 噪音。两种情况都不该把占位摆出来。
    const noisy = scan.quoted || ids.length > 0
    const candidates = base.filter((item) => !noisy || item.insertText !== '""')
    return [...ids, ...candidates]
}

/** 一个补全来源：按光标上下文给出候选，数据全部来自当前目录与版本。 */
export function createNbtCompletionSource(read: () => NbtCompletionContext): SnbtCompletionSource {
    return {
        async candidates(scan) {
            if (scan.expect === 'after') return []
            const context = read()
            // 物品在 1.20.5+ 的根是 `[id=…]`。刚敲下 `[` 时扫描器还认不出这是
            // 组件列表（没有 `=`），按元素位置给候选只会得到空列表。
            const rootList =
                scan.expect === 'element' && scan.frames.length === 1 && !scan.keyPath.length
            if (rootList && context.lookup?.components) {
                return keyCandidates(context, { ...scan, expect: 'component' })
            }
            if (scan.expect === 'key' || scan.expect === 'component') {
                return keyCandidates(context, scan)
            }
            return valueCandidates(context, scan)
        },
    }
}
