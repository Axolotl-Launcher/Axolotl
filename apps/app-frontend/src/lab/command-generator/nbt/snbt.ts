/**
 * SNBT 序列化与解析。
 *
 * 语法依据 Minecraft Wiki `NBT_format`（修订 3725656）：
 * - 标量后缀：`1b` `1s` `1` `1L` `1f` `1d`，后缀大小写不敏感；
 * - 数组：`[B;1b,2b]` `[I;1,2]` `[L;1L,2L]`，缺少前缀的 `[1,2]` 是列表；
 * - 列表元素同构，空列表没有元素类型；
 * - 键与字符串只在必要时加引号，引号内的 `\` 与引号本身需要转义。
 *
 * 约定：节点的 `value` 存裸值，后缀与引号在这里补。
 */

import { NbtTag, NbtType } from 'deepslate/nbt'
import { StringReader } from 'deepslate/util'

import {
    createNbtTag,
    isNbtArray,
    isNbtContainer,
    nbtNumericSuffix,
    type NbtTagNode,
    type NbtTagType,
} from './model.ts'

/** 可以直接不加引号书写的键名（与游戏一致）。 */
const UNQUOTED_KEY = /^[A-Za-z0-9._+-]+$/

/** 字符串字面量：始终加引号。 */
export function quoteSnbt(value: string): string {
    return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
}

/** 键名：只在不能裸写时才加引号（与游戏的 SNBT 输出一致）。 */
export function formatSnbtKey(value: string): string {
    if (value.length > 0 && UNQUOTED_KEY.test(value)) return value
    return quoteSnbt(value)
}

export function serializeNbt(nodes: NbtTagNode[]): string {
    if (nodes.length === 0) return '{}'
    const pairs = nodes.map((node) => `${formatSnbtKey(node.name)}:${serializeNbtValue(node)}`)
    return `{${pairs.join(',')}}`
}

export function serializeNbtValue(node: NbtTagNode): string {
    switch (node.type) {
        case 'byte':
        case 'short':
        case 'int':
        case 'long':
        case 'float':
        case 'double': {
            const text = String(node.value).trim() || '0'
            return `${text}${nbtNumericSuffix(node.type)}`
        }
        case 'string':
            return quoteSnbt(String(node.value))
        case 'compound':
            return serializeNbt(node.children ?? [])
        case 'list': {
            const items = (node.children ?? []).map((child) => serializeNbtValue(child)).join(',')
            return `[${items}]`
        }
        case 'byte_array':
            return `[B;${serializeNbtArray(node, 'b')}]`
        case 'int_array':
            return `[I;${serializeNbtArray(node, '')}]`
        case 'long_array':
            return `[L;${serializeNbtArray(node, 'L')}]`
        default:
            return String(node.value)
    }
}

function serializeNbtArray(node: NbtTagNode, suffix: string): string {
    const items = String(node.value ?? '')
        .split(',')
        .map((item) => item.trim().replace(/[bslfd]$/i, ''))
        .filter((item) => item.length > 0)
    return items.map((item) => `${item}${suffix}`).join(',')
}

export function serializeNbtDocument(nodes: NbtTagNode[], components: boolean): string {
    return components
        ? `[${nodes.map((node) => `${node.name}=${serializeNbtValue(node)}`).join(',')}]`
        : serializeNbt(nodes)
}

/** deepslate 的标签类型 → 编辑器的节点类型。 */
function tagTypeName(tag: NbtTag): NbtTagType {
    switch (tag.getId()) {
        case NbtType.Byte:
            return 'byte'
        case NbtType.Short:
            return 'short'
        case NbtType.Int:
            return 'int'
        case NbtType.Long:
            return 'long'
        case NbtType.Float:
            return 'float'
        case NbtType.Double:
            return 'double'
        case NbtType.String:
            return 'string'
        case NbtType.List:
            return 'list'
        case NbtType.Compound:
            return 'compound'
        case NbtType.ByteArray:
            return 'byte_array'
        case NbtType.IntArray:
            return 'int_array'
        case NbtType.LongArray:
            return 'long_array'
        default:
            return 'string'
    }
}

function fromTag(name: string, tag: NbtTag): NbtTagNode {
    const type = tagTypeName(tag)

    if (tag.isCompound()) {
        return createNbtTag(
            name,
            'compound',
            '',
            [...tag.keys()].map((key) => fromTag(key, tag.get(key)!)),
        )
    }

    if (tag.isList()) {
        const items = Array.from({ length: tag.length }, (_, index) => tag.get(index)!)
        const elementType = items.length > 0 ? tagTypeName(items[0]) : undefined
        return {
            ...createNbtTag(
                name,
                'list',
                '',
                items.map((item, index) => fromTag(String(index), item)),
            ),
            elementType,
        }
    }

    if (isNbtArray({ type })) {
        const values = Array.from({ length: tag.length }, (_, index) =>
            tag
                .get(index)!
                .toString()
                .replace(/[bslfd]$/i, ''),
        )
        return createNbtTag(name, type, values.join(','))
    }

    if (type === 'string') return createNbtTag(name, 'string', tag.getAsString())

    return createNbtTag(name, type, tag.toString().replace(/[bslfd]$/i, ''))
}

/**
 * 解析 SNBT 文档。两种根形态：
 * - `{…}` 复合标签（`/summon`、`/give` 的旧版 tag、选择器 `nbt=`）；
 * - `[k=v,…]` 数据组件（1.20.5+ 的物品）。
 */
export function parseNbt(input: string): { nodes: NbtTagNode[]; components: boolean } {
    if (!input.trim()) return { nodes: [], components: false }
    const reader = new StringReader(input.trim())

    if (reader.peek() === '[') {
        reader.skip()
        const nodes: NbtTagNode[] = []
        reader.skipWhitespace()
        while (reader.canRead() && reader.peek() !== ']') {
            const start = reader.cursor
            while (reader.canRead() && /[a-z0-9_:./-]/.test(reader.peek())) reader.skip()
            const name = reader.getRead(start)
            if (!name) throw reader.createError('Expected component name')
            reader.expect('=', true)
            nodes.push(fromTag(name, NbtTag.fromString(reader)))
            reader.skipWhitespace()
            if (reader.peek() === ']') break
            reader.expect(',')
            reader.skipWhitespace()
        }
        reader.expect(']')
        reader.skipWhitespace()
        if (reader.canRead()) throw reader.createError('Unexpected trailing input')
        return { nodes, components: true }
    }

    const tag = NbtTag.fromString(reader)
    reader.skipWhitespace()
    if (reader.canRead()) throw reader.createError('Unexpected trailing input')
    if (!tag.isCompound()) throw new Error('NBT root must be a compound')
    return { nodes: fromTag('', tag).children ?? [], components: false }
}

/** 解析任意单个标签（`/data modify … value` 可以不是复合标签）。 */
export function parseNbtTag(input: string): NbtTagNode {
    const reader = new StringReader(input.trim())
    const node = fromTag('', NbtTag.fromString(reader))
    reader.skipWhitespace()
    if (reader.canRead()) throw reader.createError('Unexpected trailing input')
    return node
}

/** 列表的元素类型：优先用记下来的，其次看第一个元素。 */
export function listElementType(node: NbtTagNode): NbtTagType | undefined {
    if (node.elementType) return node.elementType
    const first = node.children?.[0]
    return first?.type
}

/** 容器 / 数组的统一判定，供 UI 使用。 */
export function isNbtExpandable(node: NbtTagNode): boolean {
    return isNbtContainer(node) || isNbtArray(node)
}
