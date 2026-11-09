export const MINECRAFT_DIRECTORIES_STORAGE_KEY = 'axolotl-minecraft-directories'
export const BUILTIN_GAME_DIRECTORY = 'builtin'

export interface ConfiguredMinecraftDirectory {
    path: string
    mode: 'automatic' | 'isolated' | 'shared'
}

export function loadMinecraftDirectories(): ConfiguredMinecraftDirectory[] {
    try {
        const values: unknown = JSON.parse(
            localStorage.getItem(MINECRAFT_DIRECTORIES_STORAGE_KEY) ?? '[]',
        )
        if (!Array.isArray(values)) return []
        const directories = values.flatMap((value): ConfiguredMinecraftDirectory[] => {
            if (typeof value === 'string' && value.trim()) {
                return [{ path: value.trim(), mode: 'automatic' }]
            }
            if (
                value &&
                typeof value === 'object' &&
                typeof value.path === 'string' &&
                value.path.trim()
            ) {
                return [
                    {
                        path: value.path.trim(),
                        mode:
                            value.mode === 'shared'
                                ? 'shared'
                                : value.mode === 'automatic'
                                  ? 'automatic'
                                  : 'isolated',
                    },
                ]
            }
            return []
        })
        return [...new Map(directories.map((directory) => [directory.path, directory])).values()]
    } catch {
        return []
    }
}
