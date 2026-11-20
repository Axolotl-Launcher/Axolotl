/**
 * NBT 节点模型：编辑器里的一棵树，而不是 NBT 本身。
 *
 * 这里只负责"结构 + 编辑操作"，序列化与解析在 `snbt.ts`。
 * 数值一律以文本保存：`long` 超出 JS 安全整数范围，转成 number 会静默丢精度。
 */

export type NbtTagType =
    | 'byte'
    | 'short'
    | 'int'
    | 'long'
    | 'float'
    | 'double'
    | 'string'
    | 'compound'
    | 'list'
    | 'byte_array'
    | 'int_array'
    | 'long_array'

/** 单值标签（带数字后缀或引号）。 */
export const NBT_SCALAR_TYPES: readonly NbtTagType[] = [
    'byte',
    'short',
    'int',
    'long',
    'float',
    'double',
    'string',
]

/** 定长整型数组标签，SNBT 写作 `[B;…]` / `[I;…]` / `[L;…]`。 */
export const NBT_ARRAY_TYPES: readonly NbtTagType[] = ['byte_array', 'int_array', 'long_array']

/** 需要 `children` 的容器标签。 */
export const NBT_CONTAINER_TYPES: readonly NbtTagType[] = ['compound', 'list']

export interface NbtTagNode {
    id: string
    name: string
    type: NbtTagType
    /**
     * 标量：值的文本；数组：逗号分隔的元素文本（不带类型后缀）；
     * 容器：空字符串。
     */
    value: string
    /** compound 的子标签；list 的元素（`name` 只用于显示序号）。 */
    children?: NbtTagNode[]
    /** list 的元素类型。NBT 列表同构，空列表由这里记住它要装什么。 */
    elementType?: NbtTagType
    /** 结构化编辑器标识，例如 `enchantments`；只由目录入口写入。 */
    group?: string
    /** 强制按原始标签编辑，忽略 `group` 对应的结构化编辑器。 */
    raw?: boolean
}

let counter = 0

export function createNbtTag(
    name: string,
    type: NbtTagType,
    value: string | number | boolean = '',
    children?: NbtTagNode[],
): NbtTagNode {
    return {
        id: `nbt-${counter++}`,
        name,
        type,
        value: value === undefined || value === null ? '' : String(value),
        children,
    }
}

/** 该类型是否为容器（需要子节点）。 */
export function isNbtContainer(node: Pick<NbtTagNode, 'type'>): boolean {
    return NBT_CONTAINER_TYPES.includes(node.type)
}

/** 该类型是否为数组。 */
export function isNbtArray(node: Pick<NbtTagNode, 'type'>): boolean {
    return NBT_ARRAY_TYPES.includes(node.type)
}

/** 新建某个类型时的默认值文本。 */
export function defaultNbtValue(type: NbtTagType): string {
    switch (type) {
        case 'byte':
            return '0b'
        case 'short':
            return '0s'
        case 'long':
            return '0L'
        case 'float':
            return '0f'
        case 'double':
            return '0d'
        case 'int':
            return '0'
        case 'string':
            return '""'
        default:
            return ''
    }
}

/** 深拷贝，并为每个节点分配新的 id（粘贴 / 复制用）。 */
export function cloneNbtNode(node: NbtTagNode): NbtTagNode {
    return {
        ...node,
        id: `nbt-${counter++}`,
        children: node.children?.map(cloneNbtNode),
    }
}

export function findNbtNode(nodes: NbtTagNode[], id: string): NbtTagNode | undefined {
    for (const node of nodes) {
        if (node.id === id) return node
        const found = node.children ? findNbtNode(node.children, id) : undefined
        if (found) return found
    }
    return undefined
}

/** 从根到目标节点的下标路径；找不到返回 undefined。 */
export function findNbtNodePath(nodes: NbtTagNode[], id: string): number[] | undefined {
    for (let index = 0; index < nodes.length; index++) {
        const node = nodes[index]
        if (node.id === id) return [index]
        if (!node.children) continue
        const childPath = findNbtNodePath(node.children, id)
        if (childPath) return [index, ...childPath]
    }
    return undefined
}

/** 取出目标节点所在的同级数组，移动 / 排序用。 */
export function nbtSiblings(nodes: NbtTagNode[], id: string): NbtTagNode[] | undefined {
    if (nodes.some((node) => node.id === id)) return nodes
    for (const node of nodes) {
        if (!node.children) continue
        const siblings = nbtSiblings(node.children, id)
        if (siblings) return siblings
    }
    return undefined
}

export function removeNbtNode(nodes: NbtTagNode[], id: string): boolean {
    const siblings = nbtSiblings(nodes, id)
    if (!siblings) return false
    const index = siblings.findIndex((node) => node.id === id)
    if (index < 0) return false
    siblings.splice(index, 1)
    return true
}

/**
 * 在同级里移动一个节点。`offset` 为 -1 上移、1 下移。
 * 返回是否真的移动了（已在边界时返回 false）。
 */
export function moveNbtNode(nodes: NbtTagNode[], id: string, offset: number): boolean {
    const siblings = nbtSiblings(nodes, id)
    if (!siblings) return false
    const index = siblings.findIndex((node) => node.id === id)
    const target = index + offset
    if (index < 0 || target < 0 || target >= siblings.length) return false
    const [node] = siblings.splice(index, 1)
    siblings.splice(target, 0, node)
    return true
}

/**
 * 修改类型时把值调整成该类型的合法形态，避免留下 `1b` 挂在 int 上这类脏值。
 *
 * 约定：标量的 `value` 存**裸值**——数字不带后缀、字符串不带引号，
 * 后缀与引号由 `snbt.ts` 在序列化时补上。
 */
export function coerceNbtValue(value: string, type: NbtTagType): string {
    if (isNbtContainer({ type }) || isNbtArray({ type })) return ''
    if (type === 'string') return unquoteSnbtText(value)
    const numeric = value.trim().replace(/[bslfd]$/i, '')
    if (numeric === '' || !Number.isFinite(Number(numeric))) return defaultNbtValue(type)
    return numeric
}

/** 去掉用户可能连引号一起输入的字符串字面量。 */
export function unquoteSnbtText(value: string): string {
    const text = value.trim()
    if (text.length >= 2) {
        const first = text[0]
        const last = text[text.length - 1]
        if ((first === '"' || first === "'") && first === last) {
            const inner = text.slice(1, -1)
            return first === '"'
                ? inner.replace(/\\"/g, '"').replace(/\\\\/g, '\\')
                : inner.replace(/\\'/g, "'").replace(/\\\\/g, '\\')
        }
    }
    return value
}

/** 数字标签的后缀；int 没有后缀。 */
export function nbtNumericSuffix(type: NbtTagType): string {
    switch (type) {
        case 'byte':
            return 'b'
        case 'short':
            return 's'
        case 'long':
            return 'L'
        case 'float':
            return 'f'
        case 'double':
            return 'd'
        default:
            return ''
    }
}
