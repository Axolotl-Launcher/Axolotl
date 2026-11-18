export type ArgumentType =
    | 'string'
    | 'number'
    | 'integer'
    | 'boolean'
    | 'enum'
    | 'coordinates'
    | 'entity-selector'
    | 'item'
    | 'block'
    | 'nbt'
    | 'message'
    | 'time'
    | 'gamemode'
    | 'difficulty'
    | 'biome'
    | 'particle'
    | 'sound'
    | 'effect'
    | 'enchantment'
    | 'attribute'
    | 'slot'
    | 'objective'
    | 'team'
    | 'advancement'
    | 'gamerule'

export type NbtKind = 'item' | 'entity' | 'generic'

export interface ArgumentPreset {
    labelKey: string
    value: string | number | boolean
    descriptionKey?: string
}

export interface CommandArgument {
    name: string
    displayNameKey: string
    type: ArgumentType
    required: boolean
    descriptionKey: string
    enumValues?: string[]
    defaultValue?: string | number | boolean
    selector?: boolean
    placeholder?: string
    supportsNbtEditor?: boolean
    nbtKind?: NbtKind
    presets?: ArgumentPreset[]
    /** Only include this argument when all named values match. */
    when?: Record<string, Array<string | number | boolean>>
    unless?: Record<string, Array<string | number | boolean>>
    /** A boolean switch such as /list uuids, rather than a boolean value. */
    booleanKeyword?: string
    /** Fixed syntax emitted by the generator without an editable form field. */
    literal?: string
    prefix?: string
    min?: number
    max?: number
    exclusiveMin?: number
    /**
     * 坐标参数的分量数量。
     * 3（默认）= x y z；2 = x z（如 /spreadplayers 的 center、/forceload 的 from/to）。
     */
    coordinatesDimensions?: 2 | 3
    /**
     * 为 true 时，该参数渲染后直接追加到前一个参数尾部，**不加空格**。
     *
     * 用于 /give、/item 等把 NBT 视为「物品参数一部分」的命令：
     *   /give @s diamond_sword{display:...} 1
     * 而非把 NBT 作为独立参数：
     *   /summon zombie ~ ~ ~ {IsBaby:1b}
     */
    coordinatesOptionalTarget?: boolean
    attachToPrevious?: boolean
}

/**
 * 旧数据里这个写法曾经用过的子命令 id。
 *
 * 合并前每种写法都是一个独立子命令（`positioned-as`），用户预置里存的就是这种 id。
 * 不填时按 `<子命令 id>-<写法 id>` 推导，`if-biome` 这类就靠这条规则还原。
 */
export interface ExecuteChainLegacyOption {
    id: string
    /** 旧 id 里顺带编进去的字段，例如 `store-result-score` 的 `mode: 'result'` */
    values?: Record<string, string | number | boolean>
}

/**
 * 一个子命令的一种写法，例如 `positioned as <targets>`。
 *
 * 同一行的括号分支（`positioned (<pos>|as <targets>|over <heightmap>)`）参数表
 * 完全不同，所以模板与参数都挂在这一层，而不是挂在子命令上。
 */
export interface ExecuteChainVariantOption {
    /** 写进步骤 `values[variants.argument]` 的值，例如 `biome` */
    id: string
    legacy?: ExecuteChainLegacyOption[]
    labelKey: string
    descriptionKey: string
    template: string
    arguments: CommandArgument[]
    sinceVersion?: string
}

/**
 * 子命令的写法列表：`if` / `unless` 的条件、`positioned` 的定位方式、
 * `store` 的存储目标都走这里。
 *
 * 步骤里只存 `{ subcommandId: 'if', values: { condition: 'biome' } }`，
 * 具体写法由编辑区选择，所以一条链里可以有多个同族子命令。
 */
export interface ExecuteChainVariants {
    /** 选择结果写入步骤 `values` 的字段名 */
    argument: string
    labelKey: string
    descriptionKey?: string
    /** 不填则强制用户选；填了就按这个写法入链 */
    defaultValue?: string
    options: ExecuteChainVariantOption[]
}

export interface ExecuteChainSubcommand {
    id: string
    sinceVersion?: string
    labelKey: string
    descriptionKey: string
    template: string
    arguments: CommandArgument[]
    /** 有该字段时，模板与参数取自选中的变体，本层的 template/arguments 留空 */
    variants?: ExecuteChainVariants
}

export interface ExecuteChainCategory {
    id: string
    labelKey: string
    subcommands: ExecuteChainSubcommand[]
}

export interface CommandDefinition {
    id: string
    name: string
    descriptionKey: string
    category: CommandCategory
    permissionLevel: 0 | 1 | 2 | 3 | 4
    arguments: CommandArgument[]
    aliases?: string[]
    sinceVersion?: string
    removedVersion?: string
    isChainCommand?: boolean
    chainCategories?: ExecuteChainCategory[]
}

export type CommandCategory =
    'player' | 'world' | 'entity' | 'block' | 'item' | 'communication' | 'server' | 'data' | 'misc'

export interface BaseDataFile {
    version: string
    generatedAt: string
    commands: CommandDefinition[]
}

export interface PatchDataFile {
    version: string
    base: string
    generatedAt: string
    added?: CommandDefinition[]
    removed?: string[]
    patched?: Record<string, Partial<CommandDefinition>>
    chainPatches?: Record<
        string,
        {
            categoryId: string
            added?: ExecuteChainSubcommand[]
            patched?: Record<string, Partial<ExecuteChainSubcommand>>
            /** 往已有子命令的变体列表追加写法，例如 26.3 的 `slots` */
            addedOptions?: Array<{
                subcommandId: string
                option: ExecuteChainVariantOption
            }>
        }
    >
    sources?: string[]
}

export type CommandDataFile = BaseDataFile | PatchDataFile

export interface VersionManifest {
    versions: VersionEntry[]
    defaultVersion: string
}

export interface VersionEntry {
    id: string
    label: string
    dataFile: string
}
