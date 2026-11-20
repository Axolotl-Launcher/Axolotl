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
 * 路径编辑器用：给定已经走过的键名，返回这一层可选的字段。
 *
 * 走到一半断了（键名不在目录里）就返回 `undefined`，由调用方决定退回什么。
 */
export function fieldsAtPath(domain: NbtDomain, keys: string[]): NbtFieldSpec[] | undefined {
    if (keys.length === 0) return fieldsFor(domain)
    const [first, ...rest] = keys
    let current = fieldsFor(domain).find((field) => field.key === first)
    for (const key of rest) {
        current = current?.children?.find((child) => child.key === key)
    }
    if (!current) return undefined
    if (current.children?.length) return current.children
    return undefined
}
