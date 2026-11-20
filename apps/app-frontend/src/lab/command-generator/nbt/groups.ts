/**
 * 结构化组：把「附魔」「状态效果」这类固定结构从标签树变成一行行可编辑的记录。
 *
 * 落盘结构随版本变过：1.20.5 之前附魔是 `Enchantments=[{id,lvl}]`，之后是
 * `enchantments={id:等级}`。编辑器因此按节点当前的结构来读写，而不是按版本号
 * 猜——用户在原始标签里写成哪种，就在哪种上继续编辑。
 */

import { createNbtTag, type NbtTagNode } from './model.ts'

export type NbtGroupColumnKind = 'enchantment' | 'effect' | 'text' | 'number' | 'boolean' | 'enum'

export interface NbtGroupColumn {
    key: string
    kind: NbtGroupColumnKind
    zh: string
    default: string
    enumValues?: readonly string[]
    min?: number
    max?: number
    /** 列表结构里这个字段的键名与 `key` 不同时填这里（附魔旧版叫 `lvl`）。 */
    listKey?: string
}

export interface NbtGroupSpec {
    id: string
    /** 新建节点时优先用哪种结构。 */
    shape: 'map' | 'list'
    columns: NbtGroupColumn[]
}

export const NBT_GROUPS: Record<string, NbtGroupSpec> = {
    enchantments: {
        id: 'enchantments',
        shape: 'map',
        columns: [
            { key: 'id', kind: 'enchantment', zh: '附魔', default: 'minecraft:sharpness' },
            {
                key: 'level',
                kind: 'number',
                zh: '等级',
                default: '1',
                min: 1,
                max: 255,
                listKey: 'lvl',
            },
        ],
    },
    active_effects: {
        id: 'active_effects',
        shape: 'list',
        columns: [
            { key: 'id', kind: 'effect', zh: '效果', default: 'minecraft:speed' },
            { key: 'amplifier', kind: 'number', zh: '等级', default: '0', min: 0, max: 255 },
            { key: 'duration', kind: 'number', zh: '持续时间（刻）', default: '200', min: -1 },
            { key: 'ambient', kind: 'boolean', zh: '信标粒子', default: '0' },
            { key: 'show_particles', kind: 'boolean', zh: '显示粒子', default: '1' },
            { key: 'show_icon', kind: 'boolean', zh: '显示图标', default: '1' },
        ],
    },
    lore: {
        id: 'lore',
        shape: 'list',
        columns: [{ key: 'text', kind: 'text', zh: '一行文本', default: '' }],
    },
}

export type NbtGroupRow = Record<string, string>

export function groupSpec(id: string | undefined): NbtGroupSpec | undefined {
    return id ? NBT_GROUPS[id] : undefined
}

/**
 * 去掉 SNBT 的引号与数字后缀，得到可编辑的裸值。
 *
 * 后缀只对数字生效：`minecraft:speed` 也以 `d` 结尾，无条件剥掉会把 ID 咬掉一个字母。
 */
function bare(value: string): string {
    const unquoted = value.replace(/^"(.*)"$/s, '$1')
    if (/^-?\d+(?:\.\d+)?[bslfd]$/i.test(unquoted)) return unquoted.slice(0, -1)
    return unquoted
}

/** 一个 `{text:…}` 或裸字符串的文本组件取纯文本。 */
function textOf(node: NbtTagNode | undefined): string {
    if (!node) return ''
    if (node.type === 'string') return bare(node.value)
    if (node.type === 'compound') {
        const text = node.children?.find((child) => child.name === 'text')
        if (text) return bare(text.value)
    }
    return node.value
}

function textNode(name: string, value: string): NbtTagNode {
    return createNbtTag(name, 'string', JSON.stringify({ text: value }))
}

/** 节点当前用的是哪种结构；两种都不是时返回 `undefined`。 */
export function groupShapeOf(node: NbtTagNode): 'map' | 'list' | undefined {
    if (node.type === 'compound') return 'map'
    if (node.type === 'list') return 'list'
    return undefined
}

/** 节点 → 行。结构与组不匹配时返回空数组，由调用方退回原始标签编辑。 */
export function readGroupRows(node: NbtTagNode, spec: NbtGroupSpec): NbtGroupRow[] {
    const shape = groupShapeOf(node)
    if (shape === 'map') {
        return (node.children ?? []).map((child) => {
            const row: NbtGroupRow = {}
            for (const column of spec.columns) {
                row[column.key] = column.key === 'id' ? child.name : bare(child.value)
            }
            return row
        })
    }
    if (shape !== 'list') return []

    return (node.children ?? []).map((child) => {
        if (spec.id === 'lore') return { text: textOf(child) }
        const row: NbtGroupRow = {}
        for (const column of spec.columns) {
            const field = child.children?.find(
                (entry) => entry.name === (column.listKey ?? column.key),
            )
            row[column.key] = field ? bare(field.value) : column.default
        }
        return row
    })
}

/** 行 → 节点。保留节点已有的结构，只有节点还空着时才用 `spec.shape`。 */
export function writeGroupRows(node: NbtTagNode, spec: NbtGroupSpec, rows: NbtGroupRow[]) {
    const shape = groupShapeOf(node) ?? spec.shape

    if (shape === 'map') {
        node.type = 'compound'
        node.value = ''
        node.children = rows.map((row) =>
            createNbtTag(
                row.id?.trim() || spec.columns[0].default,
                'int',
                row.level?.trim() || '1',
            ),
        )
        return
    }

    node.type = 'list'
    node.value = ''
    if (spec.id === 'lore') {
        node.elementType = 'string'
        node.children = rows.map((row, index) => textNode(String(index), row.text ?? ''))
        return
    }

    node.elementType = 'compound'
    node.children = rows.map((row, index) => {
        const child = createNbtTag(String(index), 'compound', '')
        child.children = spec.columns.map((column) => {
            const raw = row[column.key] ?? column.default
            const name = column.listKey ?? column.key
            if (column.kind === 'boolean') {
                return createNbtTag(name, 'byte', raw === '1' || raw === 'true' ? '1b' : '0b')
            }
            if (column.kind === 'number') {
                // 附魔等级与状态效果等级在游戏里是 byte，持续时间是 int
                return createNbtTag(name, column.key === 'duration' ? 'int' : 'byte', raw)
            }
            return createNbtTag(name, 'string', raw)
        })
        return child
    })
}

export function emptyGroupRow(spec: NbtGroupSpec): NbtGroupRow {
    const row: NbtGroupRow = {}
    for (const column of spec.columns) row[column.key] = column.default
    return row
}
