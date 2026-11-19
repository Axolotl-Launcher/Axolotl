import type { CommandDefinition, ExecuteChainSubcommand } from './commands/types.ts'
import { migrateLegacyStep } from './variants.ts'

/** execute 链的结构化状态（与组件里的 ExecuteChainState 保持一致） */
export interface ExecuteChainState {
    chain: Array<{
        subcommandId: string
        values: Record<string, string | number | boolean>
    }>
    finalCommand: string
}

export interface UserPreset {
    id: string
    name: string
    commandId: string
    versionId: string
    /** 普通命令的参数值 */
    values: Record<string, string | number | boolean>
    /** execute 等链式命令的结构化状态 */
    executeState?: ExecuteChainState
    /** 兼容旧版本的纯字符串快照 */
    chainString?: string
    createdAt: number
}

const STORAGE_KEY = 'axolotl:command-generator:user-presets'

export function getUserPresets(): UserPreset[] {
    try {
        const raw = localStorage.getItem(STORAGE_KEY)
        if (!raw) return []
        return JSON.parse(raw)
    } catch {
        return []
    }
}

export function saveUserPreset(
    name: string,
    commandId: string,
    versionId: string,
    values: Record<string, string | number | boolean>,
    extras?: {
        executeState?: ExecuteChainState
        chainString?: string
    },
): UserPreset {
    const presets = getUserPresets()
    const preset: UserPreset = {
        id: `preset-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        name,
        commandId,
        versionId,
        values: { ...values },
        executeState: extras?.executeState,
        chainString: extras?.chainString,
        createdAt: Date.now(),
    }
    presets.push(preset)
    localStorage.setItem(STORAGE_KEY, JSON.stringify(presets))
    return preset
}

export function deleteUserPreset(id: string): void {
    const presets = getUserPresets().filter((p) => p.id !== id)
    localStorage.setItem(STORAGE_KEY, JSON.stringify(presets))
}

export function getPresetsForCommand(commandId: string): UserPreset[] {
    return getUserPresets().filter((p) => p.commandId === commandId)
}

/** 按 id 覆盖一条预置，其余预置保持原样。 */
export function replaceUserPreset(preset: UserPreset): void {
    const presets = getUserPresets().map((candidate) =>
        candidate.id === preset.id ? preset : candidate,
    )
    localStorage.setItem(STORAGE_KEY, JSON.stringify(presets))
}

/**
 * 旧版预置：链步骤里存的是 `if-biome` 这种「一个写法一个子命令」的 id，
 * 或者只剩一段命令字符串、没有结构化状态。
 *
 * 前一种能在打开时自动还原，但存下来的仍是旧写法；后一种没有步骤可以还原。
 */
export function presetNeedsMigration(
    preset: UserPreset,
    lookup: ReadonlyMap<string, ExecuteChainSubcommand>,
): boolean {
    if (!preset.executeState) return Boolean(preset.chainString)
    return preset.executeState.chain.some((step) => migrateLegacyStep(step, lookup) !== null)
}

/** 把旧版步骤换成「子命令 + 写法」；没有需要迁移的步骤时返回 null。 */
export function migratePresetState(
    state: ExecuteChainState,
    lookup: ReadonlyMap<string, ExecuteChainSubcommand>,
): ExecuteChainState | null {
    let changed = false
    const chain = state.chain.map((step) => {
        const migrated = migrateLegacyStep(step, lookup)
        if (!migrated) return step
        changed = true
        return migrated
    })
    return changed ? { chain, finalCommand: state.finalCommand } : null
}

/**
 * 开发构建里插进列表的旧版示例：条件、定位、存储各取一个旧 id，
 * 迁移后应拼成
 * `if biome ~ ~ ~ minecraft:plains positioned as @p store success entity @s Health int 1`。
 */
export const LEGACY_SAMPLE_STATE: ExecuteChainState = {
    chain: [
        { subcommandId: 'if-biome', values: { pos: '~ ~ ~', biome: 'minecraft:plains' } },
        { subcommandId: 'positioned-as', values: { target: '@p' } },
        {
            subcommandId: 'store-success-entity',
            values: { target: '@s', path: 'Health', type: 'int', scale: 1 },
        },
    ],
    finalCommand: 'say migrated',
}

/** 旧版回退形态：只剩一段命令字符串，没有步骤可以还原。 */
export const LEGACY_SAMPLE_CHAIN_STRING = 'as @a at @s run say hi'

/**
 * 完全替换语义：预设里有的用预设值，没有的回退到 defaultValue 或清空。
 */
export function applyPresetToValues(
    preset: UserPreset,
    command: CommandDefinition,
): Record<string, string | number | boolean> {
    const result: Record<string, string | number | boolean> = {}

    for (const arg of command.arguments) {
        const presetValue = preset.values[arg.name]
        if (presetValue !== undefined) {
            result[arg.name] = presetValue
        } else if (arg.defaultValue !== undefined) {
            result[arg.name] = arg.defaultValue
        }
    }

    return result
}
