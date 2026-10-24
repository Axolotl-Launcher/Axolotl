import { computed, type ComputedRef, ref } from 'vue'

import { loadCommandsForVersion, loadManifest } from '../loader.ts'
import type { CommandCategory, CommandDefinition } from './types.ts'

const currentCommands = ref<CommandDefinition[]>([])
let currentVersion = '1.21'
let commandIndex = new Map<string, CommandDefinition>()

export function initializeRegistry(versionId?: string): void {
    const manifest = loadManifest()
    currentVersion = versionId ?? manifest.defaultVersion
    currentCommands.value = loadCommandsForVersion(currentVersion)
    commandIndex = new Map()
    for (const cmd of currentCommands.value) {
        commandIndex.set(cmd.id, cmd)
    }
}

export function switchVersion(versionId: string): void {
    initializeRegistry(versionId)
}

export function getCurrentVersion(): string {
    return currentVersion
}

/** 响应式命令列表 */
export const allCommandsRef: ComputedRef<readonly CommandDefinition[]> = computed(
    () => currentCommands.value,
)

/** 兼容旧接口（非响应式快照） */
export function getAllCommands(): readonly CommandDefinition[] {
    return currentCommands.value
}

export function findCommand(id: string): CommandDefinition | undefined {
    return commandIndex.get(id)
}

export function getCommandsByCategory(category: CommandCategory): CommandDefinition[] {
    return currentCommands.value.filter((c) => c.category === category)
}

export const CATEGORY_KEYS: Record<CommandCategory, string> = {
    player: 'app.lab.command-generator.cat.player',
    world: 'app.lab.command-generator.cat.world',
    entity: 'app.lab.command-generator.cat.entity',
    block: 'app.lab.command-generator.cat.block',
    item: 'app.lab.command-generator.cat.item',
    communication: 'app.lab.command-generator.cat.communication',
    server: 'app.lab.command-generator.cat.server',
    data: 'app.lab.command-generator.cat.data',
    misc: 'app.lab.command-generator.cat.misc',
}

export const PERMISSION_KEYS: Record<number, string> = {
    0: 'app.lab.command-generator.perm.0',
    1: 'app.lab.command-generator.perm.1',
    2: 'app.lab.command-generator.perm.2',
    3: 'app.lab.command-generator.perm.3',
    4: 'app.lab.command-generator.perm.4',
}

/**
 * 权限等级的语义名称，与 Minecraft Wiki / MC 百科的 Java 版定义对齐。
 * 高等级继承低等级的全部权限。
 */
export const PERMISSION_LABELS: Record<number, string> = {
    0: 'app.lab.command-generator.perm.0',
    1: 'app.lab.command-generator.perm.1',
    2: 'app.lab.command-generator.perm.2',
    3: 'app.lab.command-generator.perm.3',
    4: 'app.lab.command-generator.perm.4',
}

/** 权限等级描述的翻译键 */
export const PERMISSION_DESC_KEYS: Record<number, string> = {
    0: 'app.lab.command-generator.perm.0.desc',
    1: 'app.lab.command-generator.perm.1.desc',
    2: 'app.lab.command-generator.perm.2.desc',
    3: 'app.lab.command-generator.perm.3.desc',
    4: 'app.lab.command-generator.perm.4.desc',
}

/** 权限等级描述的英文兜底，翻译缺失时使用 */
export const PERMISSION_DESCRIPTIONS: Record<number, string> = {
    0: 'No special permission required.',
    1: 'Can bypass spawn protection.',
    2: 'Most gameplay commands, command blocks and functions.',
    3: 'Multiplayer management commands.',
    4: 'Server management commands.',
}

export function searchCommands(query: string): CommandDefinition[] {
    if (!query.trim()) return currentCommands.value
    const q = query.toLowerCase().trim()
    return currentCommands.value.filter((cmd) => {
        if (cmd.name.toLowerCase().includes(q)) return true
        if (cmd.aliases?.some((a) => a.toLowerCase().includes(q))) return true
        return false
    })
}
