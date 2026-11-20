/**
 * NBT 编辑器的兼容入口。
 *
 * 实现已拆分到 `nbt/model.ts`（节点模型与编辑操作）与 `nbt/snbt.ts`（SNBT 读写）。
 * 这里保留原有导出名，避免一次性改动所有调用方；新代码应直接从 `nbt/` 引入。
 */

import { createNbtTag, type NbtTagNode, type NbtTagType } from './nbt/model.ts'
import {
    parseNbt,
    parseNbtTag,
    quoteSnbt,
    serializeNbt,
    serializeNbtDocument,
    serializeNbtValue,
} from './nbt/snbt.ts'

export type { NbtTagNode, NbtTagType }
export {
    createNbtTag,
    parseNbt,
    parseNbtTag,
    quoteSnbt,
    serializeNbt,
    serializeNbtDocument,
    serializeNbtValue,
}

/**
 * 简单模式只托管这几个字段，其余标签在合并时原样保留。
 *
 * 组件语法下用组件名，旧版 NBT 下用大写键名——两边都要认，否则
 * 切换模式或切换版本时用户手写的字段会被吞掉。
 */
const MANAGED_COMPONENTS = ['custom_name', 'lore', 'enchantments', 'unbreakable']
const MANAGED_TAGS = ['display', 'Enchantments', 'Unbreakable']

/** Keep tags outside the simple item's editing scope when a field changes. */
export function mergeItemNbt(original: string, edited: string, components: boolean): string {
    let previous: ReturnType<typeof parseNbt>
    try {
        previous = parseNbt(original)
    } catch {
        return edited
    }
    if (previous.components !== components || previous.nodes.length === 0) return edited

    const managed = components ? MANAGED_COMPONENTS : MANAGED_TAGS
    const nodes = previous.nodes.filter(
        (node) => !managed.includes(node.name.replace(/^minecraft:/, '')),
    )

    const next = parseNbt(edited)

    if (!components) {
        const oldDisplay = previous.nodes.find((node) => node.name === 'display')
        const extras =
            oldDisplay?.children?.filter((node) => !['Name', 'Lore'].includes(node.name)) ?? []
        if (extras.length) {
            const display = next.nodes.find((node) => node.name === 'display')
            if (display) display.children = [...extras, ...(display.children ?? [])]
            else next.nodes.push(createNbtTag('display', 'compound', '', extras))
        }
    }

    const result = [...nodes, ...next.nodes]
    return result.length ? serializeNbtDocument(result, components) : ''
}

/**
 * 简单模式的初始模板。
 *
 * `legacy` 是 1.16–1.20.4 的物品 tag；`components` 是 1.20.5+ 的组件；
 * `inline` 是 1.21.5 起可以直接写 `{text:…}` 的内联文本形式。
 */
export function createItemNbtTree(
    format: 'legacy' | 'components' | 'inline' = 'legacy',
): NbtTagNode[] {
    if (format !== 'legacy') {
        const text = (name: string, value: string) =>
            format === 'inline'
                ? createNbtTag(name, 'compound', '', [createNbtTag('text', 'string', value)])
                : createNbtTag(name, 'string', JSON.stringify({ text: value }))
        return [
            text('custom_name', '自定义名称'),
            createNbtTag('lore', 'list', '', [text('0', '第一行描述')]),
            createNbtTag('enchantments', 'compound', '', [
                createNbtTag('minecraft:sharpness', 'int', '5'),
            ]),
            createNbtTag('unbreakable', 'compound', '', []),
        ]
    }
    return [
        createNbtTag('display', 'compound', '', [
            createNbtTag('Name', 'string', '{"text":"自定义名称","color":"gold"}'),
            createNbtTag('Lore', 'list', '', [
                createNbtTag('0', 'string', '{"text":"第一行描述","color":"gray"}'),
            ]),
        ]),
        createNbtTag('Enchantments', 'list', '', [
            createNbtTag('0', 'compound', '', [
                createNbtTag('id', 'string', 'minecraft:sharpness'),
                createNbtTag('lvl', 'short', '5'),
            ]),
        ]),
        createNbtTag('Unbreakable', 'byte', '1'),
    ]
}
