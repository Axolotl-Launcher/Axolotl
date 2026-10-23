import type { CommandDefinition } from './commands/types.ts'

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
