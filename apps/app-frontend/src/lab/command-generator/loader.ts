import type {
    BaseDataFile,
    CommandDataFile,
    CommandDefinition,
    PatchDataFile,
    VersionManifest,
} from './commands/types.ts'
import manifestData from './data/manifest.json' with { type: 'json' }
import v116Data from './data/v1.16.json' with { type: 'json' }
import v117Data from './data/v1.17.json' with { type: 'json' }
import v118Data from './data/v1.18.json' with { type: 'json' }
import v119Data from './data/v1.19.json' with { type: 'json' }
import v120Data from './data/v1.20.json' with { type: 'json' }
import v121Data from './data/v1.21.json' with { type: 'json' }
import v261Data from './data/v26.1.json' with { type: 'json' }
import v262Data from './data/v26.2.json' with { type: 'json' }
import v263Data from './data/v26.3.json' with { type: 'json' }

const dataMap: Record<string, CommandDataFile> = {
    'v1.16.json': v116Data as unknown as CommandDataFile,
    'v1.17.json': v117Data as unknown as CommandDataFile,
    'v1.18.json': v118Data as unknown as CommandDataFile,
    'v1.19.json': v119Data as unknown as CommandDataFile,
    'v1.20.json': v120Data as unknown as CommandDataFile,
    'v1.21.json': v121Data as unknown as CommandDataFile,
    'v26.1.json': v261Data as unknown as CommandDataFile,
    'v26.2.json': v262Data as unknown as CommandDataFile,
    'v26.3.json': v263Data as unknown as CommandDataFile,
}

let manifestCache: VersionManifest | null = null
const resolvedCache = new Map<string, CommandDefinition[]>()

export function loadManifest(): VersionManifest {
    if (manifestCache) return manifestCache
    // Vite 会把 JSON 里的 `default` 键当作模块默认导出，
    // 因此 manifest.json 使用 `defaultVersion` 而非 `default`。
    manifestCache = manifestData as unknown as VersionManifest
    return manifestCache
}

function resolveCommandsForVersion(versionId: string): CommandDefinition[] {
    const cached = resolvedCache.get(versionId)
    if (cached) return cached

    const manifest = loadManifest()
    const entry = manifest.versions.find((v) => v.id === versionId)
    if (!entry) throw new Error(`Unknown version: ${versionId}`)

    const data = dataMap[entry.dataFile]
    if (!data) throw new Error(`Data file not registered: ${entry.dataFile}`)

    const resolved =
        'base' in data
            ? resolvePatch(data as PatchDataFile, new Set())
            : (data as BaseDataFile).commands

    resolvedCache.set(versionId, resolved)
    return resolved
}

export function loadCommandsForVersion(versionId: string): CommandDefinition[] {
    return resolveCommandsForVersion(versionId).map((command) =>
        command.chainCategories
            ? {
                  ...command,
                  chainCategories: command.chainCategories.map((category) => ({
                      ...category,
                      subcommands: category.subcommands.filter(
                          (subcommand) =>
                              !subcommand.sinceVersion ||
                              versionId.localeCompare(subcommand.sinceVersion, undefined, {
                                  numeric: true,
                              }) >= 0,
                      ),
                  })),
              }
            : command,
    )
}

function resolvePatch(patch: PatchDataFile, visited: Set<string>): CommandDefinition[] {
    if (visited.has(patch.version)) {
        throw new Error(`Circular patch: ${patch.version}`)
    }
    visited.add(patch.version)

    const baseCommands = resolveCommandsForVersion(patch.base)
    return applyPatch(baseCommands, patch)
}

function applyPatch(base: CommandDefinition[], patch: PatchDataFile): CommandDefinition[] {
    const removedSet = new Set(patch.removed ?? [])
    const patchedMap = patch.patched ?? {}
    const addedList = patch.added ?? []

    const result: CommandDefinition[] = []
    const seen = new Set<string>()

    for (const cmd of base) {
        if (removedSet.has(cmd.id)) continue
        const override = patchedMap[cmd.id]
        const merged = override ? mergeCommand(cmd, override) : cmd
        result.push(merged)
        seen.add(cmd.id)
    }

    for (const cmd of addedList) {
        if (seen.has(cmd.id)) {
            const idx = result.findIndex((c) => c.id === cmd.id)
            if (idx >= 0) result[idx] = cmd
        } else {
            result.push(cmd)
        }
    }

    return result.map((command) => {
        const chainPatch = patch.chainPatches?.[command.id]
        if (!chainPatch || !command.chainCategories) return command
        return {
            ...command,
            chainCategories: command.chainCategories.map((category) => ({
                ...category,
                subcommands: category.subcommands
                    .map((subcommand) => ({
                        ...subcommand,
                        ...chainPatch.patched?.[subcommand.id],
                    }))
                    .concat(category.id === chainPatch.categoryId ? (chainPatch.added ?? []) : []),
            })),
        }
    })
}

function mergeCommand(
    base: CommandDefinition,
    override: Partial<CommandDefinition>,
): CommandDefinition {
    return {
        ...base,
        ...override,
        arguments: override.arguments ?? base.arguments,
        chainCategories: override.chainCategories ?? base.chainCategories,
    }
}

export function clearDataCache(): void {
    resolvedCache.clear()
    manifestCache = null
}
