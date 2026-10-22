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
     * 为 true 时，该参数渲染后直接追加到前一个参数尾部，**不加空格**。
     *
     * 用于 /give、/item 等把 NBT 视为「物品参数一部分」的命令：
     *   /give @s diamond_sword{display:...} 1
     * 而非把 NBT 作为独立参数：
     *   /summon zombie ~ ~ ~ {IsBaby:1b}
     */
    attachToPrevious?: boolean
}

export interface ExecuteChainSubcommand {
    id: string
    sinceVersion?: string
    labelKey: string
    descriptionKey: string
    template: string
    arguments: CommandArgument[]
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
