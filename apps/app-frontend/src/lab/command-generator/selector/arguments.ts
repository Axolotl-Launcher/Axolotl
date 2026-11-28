/**
 * Java 版目标选择器的参数元数据。
 *
 * 权威来源：Minecraft Wiki `Target selector`（修订号见
 * `.workbuddy-ai/wiki-cache/out/Target_selector.txt`）。这里只描述 Java 版，
 * 基岩版的 `family` / `hasitem` / `has_property` / `haspermission` 以及
 * `r` `rm` `rx` `rxm` `ry` `rym` `l` `lm` `m` `c` 全部不在列表里，
 * 出现在输入中时按「Java 版不支持」提示，而不是当作合法参数。
 */

/** Java 版的六个目标变量。`@c` / `@v` / `@initiator` 是基岩版与教育版专属。 */
export type SelectorVariable = 'p' | 'r' | 'a' | 'e' | 's' | 'n'

export const SELECTOR_VARIABLES: readonly SelectorVariable[] = ['p', 'r', 'a', 'e', 's', 'n']

/** 只选中玩家的变量：`@a` / `@p` / `@r` 不接受 `type`。 */
export const PLAYER_ONLY_VARIABLES: readonly SelectorVariable[] = ['a', 'p', 'r']

export type SelectorValueKind =
    /** 单个数字，例如 `limit=3`、`dx=4`。 */
    | 'number'
    /** 单值或区间，例如 `10`、`8..16`、`..10`、`10..`。 */
    | 'range'
    /** 只接受无符号值（`distance`）。 */
    | 'unsigned-range'
    | 'coordinate'
    | 'string'
    | 'enum'
    | 'entity-type'
    | 'gamemode'
    | 'scores'
    | 'advancements'
    | 'nbt'
    | 'predicate'
    | 'tag'
    | 'team'
    | 'item'
    | 'permission'
    | 'property'

export type SelectorArgumentGroup =
    'space' | 'scoreboard' | 'species' | 'entity-data' | 'player-data' | 'limit'

export interface SelectorArgumentSpec {
    key: string
    kind: SelectorValueKind
    group: SelectorArgumentGroup
    /** 中文短标签，用于筛选卡片标题。 */
    zh: string
    /** 一句话说明，用于补全列表与卡片副标题。 */
    hint: string
    /** 同一个选择器里能否出现多次。 */
    repeatable?: boolean
    /** 值前面可以加 `!` 表示取反。 */
    negatable?: boolean
    /** 只对玩家有意义（会自动过滤掉非玩家目标）。 */
    playersOnly?: boolean
    /** 出现在这些变量上不合法。 */
    forbiddenOn?: readonly SelectorVariable[]
    /** 加了这个参数才有意义，用于提示默认行为。 */
    implies?: string
    enumValues?: readonly string[]
    min?: number
    max?: number
}

/**
 * 21 个 Java 版参数，顺序按 wiki 的汇总表：空间、计分板、物种、实体数据、
 * 玩家数据、数量。
 */
export const SELECTOR_ARGUMENTS: readonly SelectorArgumentSpec[] = [
    {
        key: 'x',
        kind: 'coordinate',
        group: 'space',
        zh: 'X 坐标',
        hint: '以该点作为距离与排序的基准',
    },
    {
        key: 'y',
        kind: 'coordinate',
        group: 'space',
        zh: 'Y 坐标',
        hint: '以该点作为距离与排序的基准',
    },
    {
        key: 'z',
        kind: 'coordinate',
        group: 'space',
        zh: 'Z 坐标',
        hint: '以该点作为距离与排序的基准',
    },
    {
        key: 'distance',
        kind: 'unsigned-range',
        group: 'space',
        zh: '距离',
        hint: '与基准点的欧氏距离，支持 8..16 这样的区间',
        min: 0,
    },
    { key: 'dx', kind: 'number', group: 'space', zh: '体积 X', hint: '体积在 X 方向的长度' },
    { key: 'dy', kind: 'number', group: 'space', zh: '体积 Y', hint: '体积在 Y 方向的长度' },
    { key: 'dz', kind: 'number', group: 'space', zh: '体积 Z', hint: '体积在 Z 方向的长度' },
    {
        key: 'x_rotation',
        kind: 'range',
        group: 'space',
        zh: '俯仰角',
        hint: '-90 为正上方，0 为水平，90 为正下方',
        min: -90,
        max: 90,
    },
    {
        key: 'y_rotation',
        kind: 'range',
        group: 'space',
        zh: '偏航角',
        hint: '0 朝南，90 朝西，180 与 -180 朝北',
        min: -180,
        max: 180,
    },
    {
        key: 'scores',
        kind: 'scores',
        group: 'scoreboard',
        zh: '计分板分数',
        hint: '{objective=10} 或 {objective=8..16}',
    },
    {
        key: 'tag',
        kind: 'tag',
        group: 'scoreboard',
        zh: '计分板标签',
        hint: '可写多个；!tag 表示排除，tag= 表示没有标签',
        repeatable: true,
        negatable: true,
    },
    {
        key: 'team',
        kind: 'team',
        group: 'scoreboard',
        zh: '队伍',
        hint: 'team= 表示不在任何队伍',
        negatable: true,
    },
    {
        key: 'name',
        kind: 'string',
        group: 'species',
        zh: '名称',
        hint: '实体名称，含空格时要加引号',
        negatable: true,
    },
    {
        key: 'type',
        kind: 'entity-type',
        group: 'species',
        zh: '实体类型',
        hint: '实体 ID 或实体类型标签；! 表示排除',
        negatable: true,
        forbiddenOn: PLAYER_ONLY_VARIABLES,
    },
    {
        key: 'predicate',
        kind: 'predicate',
        group: 'species',
        zh: '谓词',
        hint: '数据包里的谓词 ID；可写多个，! 表示排除',
        // wiki：Duplicate predicate arguments are allowed，且 `[predicate=!<id>]` 合法
        repeatable: true,
        negatable: true,
    },
    {
        key: 'nbt',
        kind: 'nbt',
        group: 'entity-data',
        zh: 'NBT 数据',
        hint: '只比较写出来的键，可写多个',
        repeatable: true,
        negatable: true,
    },
    {
        key: 'level',
        kind: 'range',
        group: 'player-data',
        zh: '经验等级',
        hint: '支持 8..16 区间，只对玩家生效',
        playersOnly: true,
        min: 0,
    },
    {
        key: 'gamemode',
        kind: 'gamemode',
        group: 'player-data',
        zh: '游戏模式',
        hint: 'survival / creative / adventure / spectator；! 表示排除',
        playersOnly: true,
        negatable: true,
        enumValues: ['survival', 'creative', 'adventure', 'spectator'],
    },
    {
        key: 'advancements',
        kind: 'advancements',
        group: 'player-data',
        zh: '进度',
        hint: '{story/smelt_iron=true}，可细分到准则',
        playersOnly: true,
    },
    {
        key: 'limit',
        kind: 'number',
        group: 'limit',
        zh: '数量上限',
        hint: '最多选中几个目标',
        min: 1,
        implies: 'sort',
    },
    {
        key: 'sort',
        kind: 'enum',
        group: 'limit',
        zh: '排序方式',
        hint: 'nearest / furthest / random / arbitrary',
        enumValues: ['nearest', 'furthest', 'random', 'arbitrary'],
        implies: 'limit',
    },
]

const BY_KEY = new Map(SELECTOR_ARGUMENTS.map((spec) => [spec.key, spec]))

export function selectorArgumentSpec(key: string): SelectorArgumentSpec | undefined {
    return BY_KEY.get(key)
}

/**
 * 基岩版（及教育版）专属的参数。出现在 Java 版指令里要给出明确提示，
 * 而不是当成「未知参数」——用户很可能只是抄了基岩版的写法。
 */
export const BEDROCK_ONLY_ARGUMENTS: ReadonlyMap<string, string> = new Map([
    ['r', 'distance'],
    ['rm', 'distance'],
    ['rx', 'x_rotation'],
    ['rxm', 'x_rotation'],
    ['ry', 'y_rotation'],
    ['rym', 'y_rotation'],
    ['l', 'level'],
    ['lm', 'level'],
    ['m', 'gamemode'],
    ['c', 'limit'],
])

/** 基岩版专属参数对应的 Java 版写法，用于提示。 */
export function bedrockEquivalent(key: string): string | undefined {
    return BEDROCK_ONLY_ARGUMENTS.get(key)
}

/** 只对玩家生效的参数：`@e` 上写了也不会选中非玩家。 */
export const PLAYER_ONLY_ARGUMENTS: readonly string[] = SELECTOR_ARGUMENTS.filter(
    (spec) => spec.playersOnly,
).map((spec) => spec.key)

/** 补全时的参数顺序：先空间，再计分板，最后数量。 */
export const SELECTOR_GROUP_ORDER: readonly SelectorArgumentGroup[] = [
    'space',
    'scoreboard',
    'species',
    'entity-data',
    'player-data',
    'limit',
]

/** 该变量下可用的参数。`@a` / `@p` / `@r` 上没有 `type`。 */
export function argumentsForVariable(
    variable: SelectorVariable,
    options: { includeForbidden?: boolean } = {},
): SelectorArgumentSpec[] {
    if (options.includeForbidden) return [...SELECTOR_ARGUMENTS]
    return SELECTOR_ARGUMENTS.filter((spec) => !spec.forbiddenOn?.includes(variable))
}

/** 按前缀过滤参数名，用于逐字符补全。 */
export function suggestSelectorArguments(
    variable: SelectorVariable,
    prefix: string,
    used: Iterable<string> = [],
): SelectorArgumentSpec[] {
    const taken = new Set(used)
    const needle = prefix.trim().toLowerCase()
    return argumentsForVariable(variable)
        .filter((spec) => {
            if (taken.has(spec.key) && !spec.repeatable) return false
            if (!needle) return true
            return (
                spec.key.toLowerCase().startsWith(needle) ||
                spec.zh.includes(needle) ||
                spec.hint.toLowerCase().includes(needle)
            )
        })
        .sort((left, right) => {
            const leftStarts = left.key.toLowerCase().startsWith(needle) ? 0 : 1
            const rightStarts = right.key.toLowerCase().startsWith(needle) ? 0 : 1
            if (leftStarts !== rightStarts) return leftStarts - rightStarts
            const leftIndex = SELECTOR_GROUP_ORDER.indexOf(left.group)
            const rightIndex = SELECTOR_GROUP_ORDER.indexOf(right.group)
            if (leftIndex !== rightIndex) return leftIndex - rightIndex
            return left.key.localeCompare(right.key)
        })
}
