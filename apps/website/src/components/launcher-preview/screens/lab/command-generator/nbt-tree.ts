import { NbtTag, NbtType, StringReader } from '../../../lib/snbt.ts'

export type NbtTagType =
    'byte' | 'short' | 'int' | 'long' | 'float' | 'double' | 'string' | 'list' | 'compound' | 'raw'

export interface NbtTagNode {
    id: string
    name: string
    type: NbtTagType
    value: string | number | boolean
    children?: NbtTagNode[]
}

let counter = 0

export function createNbtTag(
    name: string,
    type: NbtTagType,
    value: string | number | boolean,
    children?: NbtTagNode[],
): NbtTagNode {
    return {
        id: `nbt-${counter++}`,
        name,
        type,
        value,
        children,
    }
}

export function serializeNbt(nodes: NbtTagNode[]): string {
    if (nodes.length === 0) return '{}'

    const pairs = nodes.map((node) => {
        const key = quoteSnbt(node.name)
        const value = serializeNbtValue(node)
        return `${key}:${value}`
    })

    return `{${pairs.join(',')}}`
}

export function quoteSnbt(value: string): string {
    return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
}

export function serializeNbtValue(node: NbtTagNode): string {
    switch (node.type) {
        case 'byte':
            return `${node.value}b`
        case 'short':
            // 修复：short 类型需要 s 后缀，否则会被解析为 int
            return `${node.value}s`
        case 'int':
            return String(node.value)
        case 'long':
            return `${node.value}L`
        case 'float':
            return `${node.value}f`
        case 'double':
            return `${node.value}d`
        case 'string':
            return quoteSnbt(String(node.value))
        case 'compound':
            return serializeNbt(node.children ?? [])
        case 'list': {
            const items = (node.children ?? []).map((c) => serializeNbtValue(c)).join(',')
            return `[${items}]`
        }
        default:
            return String(node.value)
    }
}

function fromTag(name: string, tag: NbtTag): NbtTagNode {
    if (tag.isCompound()) {
        return createNbtTag(
            name,
            'compound',
            '',
            [...tag.keys()].map((key) => fromTag(key, tag.get(key)!)),
        )
    }
    if (tag.isList()) {
        return createNbtTag(
            name,
            'list',
            '',
            Array.from({ length: tag.length }, (_, i) => fromTag(String(i), tag.get(i))),
        )
    }
    if (tag.isString()) return createNbtTag(name, 'string', tag.getAsString())
    const type = NbtType[tag.getId()].toLowerCase()
    if (['byte', 'short', 'int', 'long', 'float', 'double'].includes(type)) {
        const value = tag.toString().replace(/[bslfd]$/i, '')
        return createNbtTag(name, type as NbtTagType, value)
    }
    // Typed arrays retain their exact SNBT representation, including long precision.
    const raw = tag.isLongArray()
        ? `[L;${Array.from({ length: tag.length }, (_, i) => tag.get(i).toString()).join(',')}]`
        : tag.toString()
    return createNbtTag(name, 'raw', raw)
}

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

export function serializeNbtDocument(nodes: NbtTagNode[], components: boolean): string {
    return components
        ? `[${nodes.map((node) => `${node.name}=${serializeNbtValue(node)}`).join(',')}]`
        : serializeNbt(nodes)
}

/** Keep tags outside the simple item's editing scope when a field changes. */
export function mergeItemNbt(original: string, edited: string, components: boolean): string {
    let previous: ReturnType<typeof parseNbt>
    try {
        previous = parseNbt(original)
    } catch {
        return edited
    }
    if (previous.components !== components || previous.nodes.length === 0) return edited
    const next = parseNbt(edited)
    const managed = components
        ? ['custom_name', 'lore', 'enchantments', 'unbreakable']
        : ['display', 'Enchantments', 'Unbreakable']
    const nodes = previous.nodes.filter(
        (node) => !managed.includes(node.name.replace(/^minecraft:/, '')),
    )
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
                createNbtTag('minecraft:sharpness', 'int', 5),
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
                createNbtTag('lvl', 'short', 5),
            ]),
        ]),
        createNbtTag('Unbreakable', 'byte', 1),
    ]
}
