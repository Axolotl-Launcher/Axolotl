/**
 * 文本组件（`display.Name` / `custom_name` / `Lore`）的读写。
 *
 * 简单模式只给一个文本框，但文本组件里还有 `color`、`bold`、`extra` 这些字段。
 * 直接按纯文本重建会把这些字段抹掉，所以这里保留原始节点：编辑时只替换 `text`，
 * 其余字段原样写回。三种写法都要支持：
 *
 * - 旧版 `display.Name` 与 1.20.5–1.21.4 的组件：值是 JSON 字符串；
 * - 1.21.5 起的内联形式：值直接是 SNBT 复合标签。
 */

import { cloneNbtNode, createNbtTag, type NbtTagNode } from './model.ts'
import { serializeNbt } from './snbt.ts'

/** 从节点里取出纯文本与原始格式。 */
export function readTextComponent(node?: NbtTagNode): { text: string; nodes: NbtTagNode[] } {
    if (!node) return { text: '', nodes: [] }

    if (node.type === 'compound') {
        const text = node.children?.find((child) => child.name === 'text')
        return { text: text ? String(text.value) : '', nodes: node.children ?? [] }
    }

    if (node.type !== 'string') return { text: '', nodes: [] }

    try {
        const parsed: unknown = JSON.parse(node.value)
        if (typeof parsed === 'string') return { text: parsed, nodes: [] }
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
            const record = parsed as Record<string, unknown>
            const text = typeof record.text === 'string' ? record.text : ''
            return { text, nodes: jsonToNodes(record) }
        }
    } catch {
        // 不是 JSON，就是一段纯文本
    }
    return { text: node.value, nodes: [] }
}

/** 把文本写进原始格式，得到可以直接序列化的节点。 */
export function textComponentNodes(nodes: NbtTagNode[], text: string): NbtTagNode[] {
    const rest = nodes.filter((node) => node.name !== 'text').map(cloneNbtNode)
    if (!text && rest.length === 0) return []
    return [createNbtTag('text', 'string', text), ...rest]
}

/** JSON 字符串形式（旧版 `display.Name` 与 1.20.5–1.21.4 的组件）。 */
export function textComponentJson(nodes: NbtTagNode[]): string {
    return JSON.stringify(nodeToJson(nodes))
}

/** 内联 SNBT 形式（1.21.5 起可以直接写 `{text:"…"}`）。 */
export function textComponentSnbt(nodes: NbtTagNode[]): string {
    return serializeNbt(nodes)
}

/** JSON 对象 → 节点。数组里的整数写成整型数组，其余按列表处理。 */
function jsonToNodes(record: Record<string, unknown>): NbtTagNode[] {
    return Object.entries(record).map(([name, value]) => jsonToNode(name, value))
}

function jsonToNode(name: string, value: unknown): NbtTagNode {
    if (typeof value === 'string') return createNbtTag(name, 'string', value)
    if (typeof value === 'boolean') return createNbtTag(name, 'byte', value ? '1' : '0')
    if (typeof value === 'number') {
        return Number.isInteger(value)
            ? createNbtTag(name, 'int', String(value))
            : createNbtTag(name, 'double', String(value))
    }
    if (Array.isArray(value)) {
        if (value.length > 0 && value.every((entry) => Number.isInteger(entry))) {
            return createNbtTag(name, 'int_array', value.join(','))
        }
        const children = value.map((entry, index) => jsonToNode(String(index), entry))
        const node = createNbtTag(name, 'list', '', children)
        node.elementType = children[0]?.type
        return node
    }
    if (value && typeof value === 'object') {
        return createNbtTag(name, 'compound', '', jsonToNodes(value as Record<string, unknown>))
    }
    return createNbtTag(name, 'string', '')
}

/** 节点 → JSON 对象。文本组件里的 byte 只用于布尔字段，所以还原成 `true` / `false`。 */
function nodeToJson(nodes: NbtTagNode[]): Record<string, unknown> {
    const record: Record<string, unknown> = {}
    for (const node of nodes) record[node.name] = nodeValueToJson(node)
    return record
}

function nodeValueToJson(node: NbtTagNode): unknown {
    switch (node.type) {
        case 'compound':
            return nodeToJson(node.children ?? [])
        case 'list':
            return (node.children ?? []).map(nodeValueToJson)
        case 'byte_array':
        case 'int_array':
        case 'long_array':
            return node.value
                .split(',')
                .map((value) => value.trim())
                .filter((value) => value !== '')
                .map((value) => Number(value.replace(/[bslfd]$/i, '')))
        case 'byte':
            return node.value === '1'
        case 'short':
        case 'int':
        case 'long':
        case 'float':
        case 'double':
            return Number(node.value)
        default:
            return node.value
    }
}
