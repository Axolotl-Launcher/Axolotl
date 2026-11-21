/**
 * 结构化组：把「附魔」「状态效果」「物品列表」这类固定结构从标签树变成一行行可编辑的记录。
 *
 * 结构随版本变过：1.20.5 之前附魔是 `Enchantments=[{id,lvl}]`（`lvl` 是 short），
 * 之后是 `enchantments={id:等级}`；物品数量也从 `Count`(byte) 变成了 `count`(int)。
 * 编辑器因此按节点当前的结构来读写，而不是按版本号猜——用户在原始标签里写成哪种，
 * 就在哪种上继续编辑。
 *
 * 结构依据 Minecraft Wiki：`Data component format`（修订 3903955）、
 * `Entity format`（修订 3865190），以及 `Template:Nbt inherit/*` 系列模板。
 */

import { groupColumnMessages } from './group-messages.ts'
import { cloneNbtNode, createNbtTag, type NbtTagNode, type NbtTagType } from './model.ts'

export type NbtGroupColumnKind =
    'enchantment' | 'effect' | 'text' | 'id' | 'number' | 'boolean' | 'enum' | 'int-list'

/** 数字列写回时用的标签类型，默认 byte（游戏里绝大多数小整数都是 byte）。 */
type NbtNumericType = Extract<NbtTagType, 'byte' | 'short' | 'int' | 'long' | 'float' | 'double'>

export interface NbtGroupColumn {
    key: string
    kind: NbtGroupColumnKind
    /** 列名（i18n 描述符，见 `group-messages.ts`）。 */
    label: { id: string; defaultMessage: string }
    default: string
    enumValues?: readonly string[]
    min?: number
    max?: number
    /**
     * 列表结构里这个字段的键名与 `key` 不同时的候选键名，第一个是新建时的默认。
     * 附魔旧版叫 `lvl`，物品数量旧版叫 `Count`。
     */
    listKeys?: readonly string[]
    numericType?: NbtNumericType
    /** 值留空时整条键省略（`attribute_modifiers[].id` 这类可选键）。 */
    omitWhenEmpty?: boolean
}

export interface NbtGroupSpec {
    id: string
    /**
     * `map`：键名即 ID、值是单个数字（附魔）；
     * `list`：复合标签的列表（状态效果、物品列表）；
     * `slots`：键名是槽位、值是物品栈（实体装备）。
     */
    shape: 'map' | 'list' | 'slots'
    /** 行放在父复合标签的哪个子键里（烟花、药水内容）。不填时行就是节点本身。 */
    listKey?: string
    /** `slots` 结构里记录槽位名的列。 */
    slotKey?: string
    /** 跟着行一起编辑的标量字段（烟花的飞行时长、药水的种类）。 */
    header?: NbtGroupColumn[]
    columns: NbtGroupColumn[]
}

/** 实体装备槽位，见 `Entity format` 的 `equipment`。 */
export const EQUIPMENT_SLOTS = [
    'mainhand',
    'offhand',
    'head',
    'chest',
    'legs',
    'feet',
    'body',
    'saddle',
] as const

/** 属性修饰符的生效槽位，见 `Data component format` 的 `attribute_modifiers`。 */
export const ATTRIBUTE_SLOTS = [
    'any',
    'hand',
    'armor',
    'mainhand',
    'offhand',
    'head',
    'chest',
    'legs',
    'feet',
    'body',
    'saddle',
] as const

/** 状态效果实例：`active_effects` 与药水的 `custom_effects` 结构相同。 */
const EFFECT_COLUMNS: NbtGroupColumn[] = [
    {
        key: 'id',
        kind: 'effect',
        label: groupColumnMessages.effect,
        default: 'minecraft:speed',
    },
    {
        key: 'amplifier',
        kind: 'number',
        label: groupColumnMessages.amplifier,
        default: '0',
        min: 0,
        max: 255,
        numericType: 'byte',
    },
    {
        key: 'duration',
        kind: 'number',
        label: groupColumnMessages.duration,
        default: '200',
        min: -1,
        numericType: 'int',
    },
    { key: 'ambient', kind: 'boolean', label: groupColumnMessages.ambient, default: '0' },
    {
        key: 'show_particles',
        kind: 'boolean',
        label: groupColumnMessages.showParticles,
        default: '1',
    },
    { key: 'show_icon', kind: 'boolean', label: groupColumnMessages.showIcon, default: '1' },
]

/** 物品栈里的数量列：1.20.5 之前叫 `Count` 且是 byte。 */
const ITEM_COUNT_COLUMN: NbtGroupColumn = {
    key: 'count',
    kind: 'number',
    label: groupColumnMessages.count,
    default: '1',
    min: 1,
    max: 99,
    listKeys: ['count', 'Count'],
    numericType: 'int',
}

const ITEM_ID_COLUMN: NbtGroupColumn = {
    key: 'id',
    kind: 'id',
    label: groupColumnMessages.item,
    default: 'minecraft:stone',
}

export const NBT_GROUPS: Record<string, NbtGroupSpec> = {
    enchantments: {
        id: 'enchantments',
        shape: 'map',
        columns: [
            {
                key: 'id',
                kind: 'enchantment',
                label: groupColumnMessages.enchantment,
                default: 'minecraft:sharpness',
            },
            {
                key: 'level',
                kind: 'number',
                label: groupColumnMessages.level,
                default: '1',
                min: 1,
                max: 255,
                listKeys: ['lvl'],
                numericType: 'short',
            },
        ],
    },
    active_effects: {
        id: 'active_effects',
        shape: 'list',
        columns: EFFECT_COLUMNS,
    },
    lore: {
        id: 'lore',
        shape: 'list',
        columns: [{ key: 'text', kind: 'text', label: groupColumnMessages.text, default: '' }],
    },
    /** 容器里的物品列表：实体 `Items` / `Inventory`、方块实体的 `Items`。 */
    item_list: {
        id: 'item_list',
        shape: 'list',
        columns: [
            {
                key: 'Slot',
                kind: 'number',
                label: groupColumnMessages.slot,
                default: '0',
                min: 0,
                max: 255,
                numericType: 'byte',
            },
            ITEM_ID_COLUMN,
            ITEM_COUNT_COLUMN,
        ],
    },
    /** 实体的 `equipment`：键名是槽位，值是物品栈（1.21.5+）。 */
    equipment: {
        id: 'equipment',
        shape: 'slots',
        slotKey: 'slot',
        columns: [
            {
                key: 'slot',
                kind: 'enum',
                label: groupColumnMessages.slot,
                default: EQUIPMENT_SLOTS[0],
                enumValues: EQUIPMENT_SLOTS,
            },
            ITEM_ID_COLUMN,
            ITEM_COUNT_COLUMN,
        ],
    },
    attribute_modifiers: {
        id: 'attribute_modifiers',
        shape: 'list',
        columns: [
            {
                key: 'type',
                kind: 'id',
                label: groupColumnMessages.attribute,
                default: 'minecraft:scale',
            },
            {
                key: 'amount',
                kind: 'number',
                label: groupColumnMessages.amount,
                default: '0',
                numericType: 'double',
            },
            {
                key: 'operation',
                kind: 'enum',
                label: groupColumnMessages.operation,
                default: 'add_value',
                enumValues: ['add_value', 'add_multiplied_base', 'add_multiplied_total'],
            },
            {
                key: 'slot',
                kind: 'enum',
                label: groupColumnMessages.attributeSlot,
                default: 'any',
                enumValues: ATTRIBUTE_SLOTS,
            },
            {
                key: 'id',
                kind: 'id',
                label: groupColumnMessages.modifierId,
                default: '',
                omitWhenEmpty: true,
            },
        ],
    },
    /** 烟花：飞行时长在父标签上，爆炸效果是一行行记录。 */
    fireworks: {
        id: 'fireworks',
        shape: 'list',
        listKey: 'explosions',
        header: [
            {
                key: 'flight_duration',
                kind: 'number',
                label: groupColumnMessages.flightDuration,
                default: '1',
                min: -128,
                max: 127,
                numericType: 'byte',
            },
        ],
        columns: [
            {
                key: 'shape',
                kind: 'enum',
                label: groupColumnMessages.shape,
                default: 'small_ball',
                enumValues: ['small_ball', 'large_ball', 'star', 'creeper', 'burst'],
            },
            { key: 'colors', kind: 'int-list', label: groupColumnMessages.colors, default: '' },
            {
                key: 'fade_colors',
                kind: 'int-list',
                label: groupColumnMessages.fadeColors,
                default: '',
            },
            {
                key: 'has_trail',
                kind: 'boolean',
                label: groupColumnMessages.hasTrail,
                default: '0',
            },
            {
                key: 'has_twinkle',
                kind: 'boolean',
                label: groupColumnMessages.hasTwinkle,
                default: '0',
            },
        ],
    },
    /** 药水内容：种类与颜色在父标签上，自定义效果是一行行记录。 */
    potion_contents: {
        id: 'potion_contents',
        shape: 'list',
        listKey: 'custom_effects',
        header: [
            {
                key: 'potion',
                kind: 'id',
                label: groupColumnMessages.potion,
                default: 'minecraft:swiftness',
                omitWhenEmpty: true,
            },
            {
                key: 'custom_color',
                kind: 'number',
                label: groupColumnMessages.customColor,
                default: '',
                min: 0,
                numericType: 'int',
                omitWhenEmpty: true,
            },
        ],
        columns: EFFECT_COLUMNS,
    },
}

export type NbtGroupRow = Record<string, string>

/**
 * 每一行来自哪个原节点，`undefined` 表示这一行是新加的。
 *
 * 表格只建模了一部分子键，其余子键要跟着**这一行**走。按位置取不行：删掉一行
 * 之后后面的行整体挪位，会把上一行的 `display` 之类挂到别人身上。
 */
export type NbtGroupOrigins = readonly (NbtTagNode | undefined)[]

export function groupSpec(id: string | undefined): NbtGroupSpec | undefined {
    return id ? NBT_GROUPS[id] : undefined
}

/** 表格每一行对应的原节点，顺序与 `readGroupRows` 一致。 */
export function groupRowNodes(node: NbtTagNode, spec: NbtGroupSpec): NbtTagNode[] {
    const container = rowsNode(node, spec)
    if (!container) return []
    if (spec.shape === 'slots') {
        return (container.children ?? []).filter((child) => child.type === 'compound')
    }
    if (container.type === 'list') return [...(container.children ?? [])]
    if (container.type === 'compound' && spec.shape === 'map') {
        return (mapRowsNode(container).children ?? []).filter((child) =>
            NAMESPACED_ID.test(child.name),
        )
    }
    return []
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

/** `{"text":"…","color":"red"}` 这种 JSON 字符串条目。 */
function parseTextComponent(raw: string): Record<string, unknown> | undefined {
    const trimmed = raw.trim()
    if (!trimmed.startsWith('{')) return undefined
    try {
        const parsed: unknown = JSON.parse(trimmed)
        if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return undefined
        return parsed as Record<string, unknown>
    } catch {
        return undefined
    }
}

/** 一个 `{text:…}`、JSON 字符串或裸字符串的文本组件取纯文本。 */
function textOf(node: NbtTagNode | undefined): string {
    if (!node) return ''
    if (node.type === 'string') {
        const raw = bare(node.value)
        // 样式写成 JSON 字符串时，表格里要显示文字而不是整段 JSON
        const component = parseTextComponent(raw)
        if (component) return typeof component.text === 'string' ? component.text : ''
        return raw
    }
    if (node.type === 'compound') {
        const text = node.children?.find((child) => child.name === 'text')
        if (text) return bare(text.value)
    }
    return node.value
}

function textNode(name: string, value: string): NbtTagNode {
    return createNbtTag(name, 'string', JSON.stringify({ text: value }))
}

/**
 * 写回一条描述文本：只换文字，颜色、粗体这些样式字段与写法（复合标签 / JSON
 * 字符串 / 裸字符串）都跟着原条目走。没有原条目时才新建 `{text:…}`。
 */
function loreNode(origin: NbtTagNode | undefined, index: number, text: string): NbtTagNode {
    if (origin?.type === 'compound') {
        const clone = cloneNbtNode(origin)
        const field = clone.children?.find((child) => child.name === 'text')
        if (field) field.value = text
        else clone.children = [createNbtTag('text', 'string', text), ...(clone.children ?? [])]
        return clone
    }
    if (origin?.type === 'string') {
        const raw = bare(origin.value)
        const component = parseTextComponent(raw)
        if (component) {
            return createNbtTag(origin.name, 'string', JSON.stringify({ ...component, text }))
        }
        // 裸字符串本身就是一个纯文本组件，别包成 JSON 再套一层
        return createNbtTag(origin.name, 'string', text)
    }
    return textNode(String(index), text)
}

/** 节点当前用的是哪种结构；两种都不是时返回 `undefined`。 */
export function groupShapeOf(node: NbtTagNode): 'map' | 'list' | undefined {
    if (node.type === 'compound') return 'map'
    if (node.type === 'list') return 'list'
    return undefined
}

/**
 * 节点结构是否与组定义相符。
 *
 * `map` 与 `slots` 都是复合标签（键名承载语义），`list` 是列表；`listKey`
 * 结构把行放在父复合标签的子键里，所以节点本身必须是复合标签。
 *
 * 形状不符时不能套表格：用表格写回会把 `{levels:…,show_in_tooltip:…}` 这类
 * 另一种写法整体改成列表，等于静默改写用户的标签。
 */
export function shapeMatches(node: NbtTagNode, spec: NbtGroupSpec): boolean {
    if (spec.listKey) return node.type === 'compound'
    if (spec.shape === 'list') return node.type === 'list'
    return node.type === 'compound'
}

/** 映射结构的行键是命名空间 ID（`minecraft:sharpness`）；其余子键不是行。 */
const NAMESPACED_ID = /^[a-z0-9_.-]+:[a-z0-9_./-]+$/

/**
 * 映射结构的行容器。
 *
 * 1.20.5 起的附魔组件是 `{levels:{…},show_in_tooltip:…}`：等级在 `levels` 里，
 * 直接写在外层会被游戏忽略。没有这层包装（旧版 `Enchantments=[…]` 之外的裸映射）
 * 时行就是节点自己。
 */
function mapRowsNode(container: NbtTagNode): NbtTagNode {
    if (container.type !== 'compound') return container
    const levels = container.children?.find((child) => child.name === 'levels')
    return levels?.type === 'compound' ? levels : container
}

/** 按候选键名找子标签（`count` / `Count`、`level` / `lvl`）。 */
function childField(node: NbtTagNode, column: NbtGroupColumn): NbtTagNode | undefined {
    for (const name of column.listKeys ?? [column.key]) {
        const found = node.children?.find((entry) => entry.name === name)
        if (found) return found
    }
    return undefined
}

/** 单元格 → 文本。颜色这类整数列表读成 `255,0,0`。 */
function readCell(column: NbtGroupColumn, field: NbtTagNode | undefined): string {
    if (!field) return column.default
    if (column.kind === 'int-list') {
        if (field.type === 'list') {
            return (field.children ?? []).map((child) => bare(child.value)).join(',')
        }
        return field.value
    }
    return bare(field.value)
}

/** 文本 → 单元格节点；`omitWhenEmpty` 的列留空时返回 `undefined`，表示不要这条键。 */
function cellNode(
    column: NbtGroupColumn,
    name: string,
    raw: string,
    existingType?: NbtTagType,
): NbtTagNode | undefined {
    const text = raw.trim()
    if (column.omitWhenEmpty && text === '') return undefined
    if (column.kind === 'boolean') {
        return createNbtTag(name, 'byte', text === '1' || text === 'true' ? '1' : '0')
    }
    if (column.kind === 'number') {
        // 沿用节点上已有的数值类型：旧版的 `Count` 是 byte，不该被改成 int
        const type = existingType && NUMERIC_TYPES.includes(existingType) ? existingType : undefined
        return createNbtTag(name, type ?? column.numericType ?? 'byte', text === '' ? '0' : text)
    }
    if (column.kind === 'int-list') {
        const values = text
            .split(',')
            .map((value) => value.trim())
            .filter((value) => value !== '')
        const node = createNbtTag(
            name,
            'list',
            '',
            values.map((value, index) => createNbtTag(String(index), 'int', value)),
        )
        node.elementType = 'int'
        return node
    }
    return createNbtTag(name, 'string', raw)
}

/** 数字标签类型，用来判断已有字段能不能直接沿用。 */
const NUMERIC_TYPES: readonly NbtTagType[] = ['byte', 'short', 'int', 'long', 'float', 'double']

/**
 * 写回时用哪个键名：优先沿用已有行里的写法（`Count` 不会悄悄变成 `count`）。
 *
 * `rows` 是写之前的行节点，所以要往它们的子标签里找键名，而不是看行自己的名字。
 */
function writeName(column: NbtGroupColumn, rows: NbtTagNode[]): string {
    const names = column.listKeys ?? [column.key]
    for (const name of names) {
        if (existingFieldType(rows, name) !== undefined) return name
    }
    return names[0]
}

/** 已有行里同名字段的类型，用于沿用（`Count` 是 byte、`count` 是 int）。 */
function existingFieldType(rows: NbtTagNode[], name: string): NbtTagType | undefined {
    for (const row of rows) {
        const found = row.children?.find((entry) => entry.name === name)
        if (found) return found.type
    }
    return undefined
}

/** 行所在的容器：`listKey` 结构下是某个子标签，否则就是节点自己。 */
function rowsNode(node: NbtTagNode, spec: NbtGroupSpec): NbtTagNode | undefined {
    if (!spec.listKey) return node
    return node.children?.find((child) => child.name === spec.listKey)
}

function ensureRowsNode(node: NbtTagNode, spec: NbtGroupSpec): NbtTagNode {
    if (!spec.listKey) return node
    node.type = 'compound'
    node.value = ''
    if (!node.children) node.children = []
    let child = node.children.find((entry) => entry.name === spec.listKey)
    if (!child) {
        child = createNbtTag(spec.listKey, 'list', '', [])
        child.elementType = 'compound'
        node.children.push(child)
    }
    return child
}

/**
 * 节点 → 行。结构与组不匹配时返回空数组，由调用方退回原始标签编辑。
 *
 * 实际形状以节点为准：附魔的定义是映射，但 1.20.5 之前是列表，两种都要能读。
 */
export function readGroupRows(node: NbtTagNode, spec: NbtGroupSpec): NbtGroupRow[] {
    const container = rowsNode(node, spec)
    if (!container) return []

    if (spec.shape === 'slots') {
        if (container.type !== 'compound') return []
        return (container.children ?? [])
            .filter((child) => child.type === 'compound')
            .map((child) => {
                const row: NbtGroupRow = {}
                for (const column of spec.columns) {
                    row[column.key] =
                        column.key === spec.slotKey
                            ? child.name
                            : readCell(column, childField(child, column))
                }
                return row
            })
    }

    if (container.type === 'list') {
        return (container.children ?? []).map((child) => {
            if (spec.id === 'lore') return { text: textOf(child) }
            const row: NbtGroupRow = {}
            for (const column of spec.columns) {
                row[column.key] = readCell(column, childField(child, column))
            }
            return row
        })
    }

    if (container.type === 'compound' && spec.shape === 'map') {
        const [idColumn, ...rest] = spec.columns
        return (mapRowsNode(container).children ?? [])
            .filter((child) => NAMESPACED_ID.test(child.name))
            .map((child) => {
                const row: NbtGroupRow = { [idColumn.key]: child.name }
                for (const column of rest) row[column.key] = bare(child.value)
                return row
            })
    }

    return []
}

/** 行 → 节点。保留节点已有的结构，只有节点还空着时才用 `spec.shape`。 */
export function writeGroupRows(
    node: NbtTagNode,
    spec: NbtGroupSpec,
    rows: NbtGroupRow[],
    origins?: NbtGroupOrigins,
) {
    const container = ensureRowsNode(node, spec)
    const shape = spec.shape === 'slots' ? 'slots' : container.type === 'list' ? 'list' : spec.shape

    if (shape === 'slots') {
        const slotKey = spec.slotKey ?? spec.columns[0].key
        const slotNames = spec.columns.find((column) => column.key === slotKey)?.enumValues ?? []
        const managed = new Set(
            spec.columns.flatMap((column) => [column.key, ...(column.listKeys ?? [])]),
        )
        // 槽位是有限集合，其余子键不是行（`equipment` 里可能还有别的写法），原样留着
        const extras = (container.children ?? []).filter((child) => !slotNames.includes(child.name))
        const previous = new Map(
            (container.children ?? [])
                .filter((child) => child.type === 'compound')
                .map((child) => [child.name, child] as const),
        )
        container.type = 'compound'
        container.value = ''
        container.children = rows.map((row, index) => {
            const name = row[slotKey]?.trim() || spec.columns[0].default
            const child = createNbtTag(name, 'compound', '')
            // 表格只建模了 id / count：槽位里原有的组件、`tag`、附魔都得留下，
            // 否则在表格里改一次数量就把这把剑变成白板
            const origin = origins?.[index] ?? previous.get(name)
            child.children = [
                ...spec.columns
                    .filter((column) => column.key !== slotKey)
                    .map((column) =>
                        cellNode(column, column.key, row[column.key] ?? column.default),
                    )
                    .filter((entry): entry is NbtTagNode => entry !== undefined),
                ...(origin?.children ?? [])
                    .filter((entry) => !managed.has(entry.name))
                    .map(cloneNbtNode),
            ]
            return child
        })
        container.children.push(...extras.map(cloneNbtNode))
        return
    }

    if (shape === 'map') {
        const [idColumn, levelColumn] = spec.columns
        // `show_in_tooltip` 这类兄弟键不是行；写回时也不能顺手抹掉
        const target = mapRowsNode(container)
        const extras = (target.children ?? []).filter((child) => !NAMESPACED_ID.test(child.name))
        target.type = 'compound'
        target.value = ''
        target.children = rows.map((row) =>
            // 1.20.5+ 的附魔映射值统一是 int
            createNbtTag(
                row[idColumn.key]?.trim() || idColumn.default,
                'int',
                row[levelColumn?.key]?.trim() || levelColumn?.default || '1',
            ),
        )
        target.children.push(...extras.map(cloneNbtNode))
        return
    }

    const previous = container.children ?? []
    container.type = 'list'
    container.value = ''

    if (spec.id === 'lore') {
        container.children = rows.map((row, index) =>
            loreNode(origins?.[index] ?? previous[index], index, row.text ?? ''),
        )
        // 元素类型跟着实际条目走：复合条目不能标成 string
        container.elementType = container.children[0]?.type
        return
    }

    container.elementType = 'compound'
    // 列的可选键名（`count` / `Count`）都算表格托管，否则会被当成「未知子键」留下第二条
    const managed = new Set(
        spec.columns.flatMap((column) => [column.key, ...(column.listKeys ?? [])]),
    )
    container.children = rows.map((row, index) => {
        const child = createNbtTag(String(index), 'compound', '')
        // 表格没建模的子键（`attribute_modifiers[].display` 之类）原样保留，
        // 否则在表格里改一次数量就会把整条记录的其他字段吞掉
        const origin = origins?.[index] ?? previous[index]
        const extras = (origin?.children ?? [])
            .filter((entry) => !managed.has(entry.name))
            .map(cloneNbtNode)
        child.children = [
            ...spec.columns
                .map((column) => {
                    const name = writeName(column, previous)
                    return cellNode(
                        column,
                        name,
                        row[column.key] ?? column.default,
                        existingFieldType(previous, name),
                    )
                })
                .filter((entry): entry is NbtTagNode => entry !== undefined),
            ...extras,
        ]
        return child
    })
}

/** 父标签上的标量字段（烟花的 `flight_duration`、药水的 `potion`）。 */
export function readGroupHeader(node: NbtTagNode, spec: NbtGroupSpec): NbtGroupRow {
    const row: NbtGroupRow = {}
    for (const column of spec.header ?? []) {
        row[column.key] = readCell(column, childField(node, column))
    }
    return row
}

export function writeGroupHeader(node: NbtTagNode, spec: NbtGroupSpec, row: NbtGroupRow) {
    const header = spec.header ?? []
    if (header.length === 0) return
    node.type = 'compound'
    node.value = ''
    if (!node.children) node.children = []

    for (const column of header) {
        const names = column.listKeys ?? [column.key]
        const index = node.children.findIndex((entry) => names.includes(entry.name))
        const next = cellNode(
            column,
            index >= 0 ? node.children[index].name : names[0],
            row[column.key] ?? column.default,
        )
        if (!next) {
            if (index >= 0) node.children.splice(index, 1)
            continue
        }
        if (index >= 0) node.children[index] = next
        // 头部字段排在行容器前面，读起来和 wiki 的顺序一致
        else node.children.unshift(next)
    }
}

export function emptyGroupRow(spec: NbtGroupSpec): NbtGroupRow {
    const row: NbtGroupRow = {}
    for (const column of spec.columns) row[column.key] = column.default
    return row
}
