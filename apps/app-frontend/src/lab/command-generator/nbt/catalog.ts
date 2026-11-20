/**
 * 目录数据的载入与索引。
 *
 * 三个数据文件由 wiki 抽取生成（见各自头部的 `sources`），运行时只读。
 * JSON 用静态 import，打包器会把它当作资源处理，不需要运行时请求。
 */

import blockEntityCatalog from '../data/nbt/block-entity.json' with { type: 'json' }
import entityCatalog from '../data/nbt/entity.json' with { type: 'json' }
import itemCatalog from '../data/nbt/item.json' with { type: 'json' }
import {
    availableFields,
    fieldAppliesTo,
    type NbtAppliesTo,
    type NbtCatalogFile,
    type NbtContext,
    type NbtDomain,
    type NbtFieldSpec,
} from './schema.ts'

const CATALOGS: Record<NbtDomain, NbtCatalogFile> = {
    entity: entityCatalog as unknown as NbtCatalogFile,
    item: itemCatalog as unknown as NbtCatalogFile,
    block_entity: blockEntityCatalog as unknown as NbtCatalogFile,
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

/** 按路径（`listener.event.distance`）取字段，用于测试与组编辑器。 */
export function findFieldByPath(domain: NbtDomain, path: string): NbtFieldSpec | undefined {
    const [first, ...rest] = path.split('.')
    let current = fieldsFor(domain).find((field) => field.key === first)
    for (const key of rest) {
        current = current?.children?.find((child) => child.key === key)
    }
    return current
}

/**
 * 走到某个复合标签，返回它这一层可选的子字段。
 *
 * 按目标挑同名字段：`item` 在物品展示实体上是展示物品、在箭上是消耗掉的物品，
 * 子字段并不相同，只看键名会拿到别的母项的结构。
 * 这一层没有目录可用的子字段时返回 `undefined`，调用方不应该退回根字段——
 * 根字段是母项那一层的东西，塞进子层只会误导。
 */
export function childrenAtPath(
    domain: NbtDomain,
    keys: string[],
    targetId?: string,
): NbtFieldSpec[] | undefined {
    if (keys.length === 0) return fieldsFor(domain)
    const groups = groupsFor(domain)
    let level = fieldsFor(domain)
    let current: NbtFieldSpec | undefined
    for (const key of keys) {
        current = level.find(
            (field) =>
                field.key === key && (!targetId || fieldAppliesTo(field, { targetId }, groups)),
        )
        if (!current) return undefined
        level = current.children ?? []
    }
    return current?.children?.length ? current.children : undefined
}
