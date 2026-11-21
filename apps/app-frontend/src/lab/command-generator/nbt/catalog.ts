/**
 * 目录数据的载入与索引。
 *
 * 三个数据文件由 wiki 抽取生成（见各自头部的 `sources`），运行时只读。
 * JSON 用静态 import，打包器会把它当作资源处理，不需要运行时请求。
 *
 * 载入时统一归一化成 `forms`（见 `schema.ts`），所以运行时不存在
 * 「有的字段写 `type`、有的写 `forms`」这种半吊子状态。
 */

import blockEntityCatalog from '../data/nbt/block-entity.json' with { type: 'json' }
import entityCatalog from '../data/nbt/entity.json' with { type: 'json' }
import itemCatalog from '../data/nbt/item.json' with { type: 'json' }
import {
    availableFields,
    childrenOf,
    dedupeFields,
    elementOf,
    fieldAppliesTo,
    formFor,
    type NbtAppliesTo,
    type NbtCatalogFile,
    type NbtCatalogInput,
    type NbtContext,
    type NbtDomain,
    type NbtFieldSpec,
    type NbtValueForm,
    normalizeCatalog,
} from './schema.ts'

const CATALOGS: Record<NbtDomain, NbtCatalogFile> = {
    entity: normalizeCatalog(entityCatalog as unknown as NbtCatalogInput),
    item: normalizeCatalog(itemCatalog as unknown as NbtCatalogInput),
    block_entity: normalizeCatalog(blockEntityCatalog as unknown as NbtCatalogInput),
}

export function catalogFor(domain: NbtDomain): NbtCatalogFile {
    return CATALOGS[domain]
}

export function fieldsFor(domain: NbtDomain): NbtFieldSpec[] {
    return CATALOGS[domain].fields
}

export function groupsFor(domain: NbtDomain): Record<string, NbtAppliesTo> {
    return CATALOGS[domain].groups ?? {}
}

/** 带分组展开的可用字段查询：UI 一律走这里，避免忘记传分组表。 */
export function availableFieldsFor(
    context: NbtContext,
    options: { onlyApplicable?: boolean } = {},
): NbtFieldSpec[] {
    return availableFields(fieldsFor(context.domain), context, {
        ...options,
        groups: groupsFor(context.domain),
    })
}

/**
 * 目录查询的上下文。
 *
 * `components` 只对物品有意义：1.20.5 起 NBT 是数据组件列表，之前是
 * `{id, Count, tag}` 里的 `tag` 复合标签。两种写法的键名与结构完全不同，
 * 由页面上的「使用 1.20.5+ 数据组件」开关决定查哪一套。
 */
export interface NbtLookup {
    domain: NbtDomain
    version: string
    targetId?: string
    components?: boolean
}

function itemUsesComponents(lookup: NbtLookup): boolean {
    return lookup.domain !== 'item' || lookup.components !== false
}

/**
 * 这一层的根字段。
 *
 * 旧版物品的根不是组件，而是 `tag` 的子键（`display`、`Enchantments`…）。
 * 面板如果在这种模式下还列 `custom_name`，用户就会把组件写进旧版 tag 里。
 */
export function rootFieldsFor(lookup: NbtLookup): NbtFieldSpec[] {
    if (itemUsesComponents(lookup)) return fieldsFor(lookup.domain)
    const tag = fieldsFor('item').find((field) => field.key === 'tag')
    return tag ? childrenOf(tag.forms[0]) : []
}

/** 按路径（`listener.event.distance`）取字段，用于测试与组编辑器。 */
export function findFieldByPath(domain: NbtDomain, path: string): NbtFieldSpec | undefined {
    const [first, ...rest] = path.split('.')
    let current = fieldsFor(domain).find((field) => field.key === first)
    for (const key of rest) {
        if (!current) return undefined
        current = childrenOf(current.forms[0]).find((child) => child.key === key)
    }
    return current
}

/** 走到某一层能看到的字段，以及这一层如果是列表时的元素写法。 */
export interface NbtLevel {
    /** 这一层可选的字段：复合的子字段，或列表元素的子字段。 */
    fields: NbtFieldSpec[]
    /** 这一层是列表时，元素的写法。面板据此决定「插入的是元素」而不是命名子项。 */
    element?: NbtValueForm
    /** 这一层对应的字段本身（标题、说明用）。根层为 `undefined`。 */
    spec?: NbtFieldSpec
    /** 这一层实际用的写法。 */
    form?: NbtValueForm
}

/**
 * 按键名路径取一层。
 *
 * 列表元素不贡献键名，所以 `Items[0].id` 的路径是 `['Items','id']`；
 * 走到列表字段时返回元素的子字段与元素写法。
 *
 * 这一层没有目录可用的子字段时返回 `undefined`，调用方不应该退回根字段——
 * 根字段是母项那一层的东西，塞进子层只会误导。
 */
export function levelAtPath(lookup: NbtLookup, keys: string[]): NbtLevel | undefined {
    const groups = groupsFor(lookup.domain)
    let level = rootFieldsFor(lookup)
    let spec: NbtFieldSpec | undefined
    let form: NbtValueForm | undefined

    for (const key of keys) {
        spec = level.find(
            (field) =>
                field.key === key &&
                (!lookup.targetId || fieldAppliesTo(field, { targetId: lookup.targetId }, groups)),
        )
        if (!spec) return undefined
        form = formFor(spec, lookup.version)
        level = childrenOf(form)
    }

    if (!spec) {
        return { fields: dedupeFields(level) }
    }
    return { fields: dedupeFields(level), element: elementOf(form), spec, form }
}
