import type { ImportableLauncher } from '@modrinth/ui'

export type FoundInstance = ImportableLauncher['instances'][number]

/**
 * One entry of "Found on this device": a `.minecraft` folder whose versions the launcher
 * can manage in place, or the launcher's own instances, which can only be imported.
 */
export type FoundGroup = {
    key: string
    launcher: ImportableLauncher
    /** The `.minecraft` folder every instance is a version of. */
    root?: string
    instances: FoundInstance[]
}

const VERSION_FOLDER = /^(.+)[\\/]+versions[\\/]+[^\\/]+[\\/]*$/i

/** A shared game directory is not a version's identity or metadata location. */
export function instancePathOf(instance: FoundInstance) {
    return instance.version_path || instance.path || instance.name
}

/** The folder a `<root>/versions/<id>` version folder sits in; other layouts have none. */
export function minecraftRootOf(instancePath: string): string | undefined {
    return VERSION_FOLDER.exec(instancePath)?.[1]
}

/** Compares folder paths the way Windows does: by either separator, in any case. */
export function folderKey(path: string) {
    return path.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase()
}

export function folderName(path: string) {
    const parts = path.split(/[\\/]+/).filter(Boolean)
    return parts[parts.length - 1] ?? path
}

/**
 * Splits what the launchers reported into `.minecraft` folders and launcher instances.
 * A folder two launchers both list shows once, under the first.
 */
export function groupFound(launchers: readonly ImportableLauncher[]): FoundGroup[] {
    const groups: FoundGroup[] = []
    const roots = new Map<string, FoundGroup>()
    const ordered = [...launchers].sort(
        (a, b) => Number(b.name === 'MinecraftLauncher') - Number(a.name === 'MinecraftLauncher'),
    )
    for (const launcher of ordered) {
        if (launcher.name === 'MinecraftLauncher') {
            const key = folderKey(launcher.path)
            const group = {
                key: `root:${key}`,
                launcher,
                root: launcher.path,
                instances: [] as FoundInstance[],
            }
            roots.set(key, group)
            groups.push(group)
        }
        const own: FoundInstance[] = []
        for (const instance of launcher.instances) {
            const root =
                launcher.name === 'MinecraftLauncher'
                    ? launcher.path
                    : minecraftRootOf(instancePathOf(instance))
            if (!root) {
                own.push(instance)
                continue
            }
            const key = folderKey(root)
            let group = roots.get(key)
            if (!group) {
                group = { key: `root:${key}`, launcher, root, instances: [] }
                roots.set(key, group)
                groups.push(group)
            }
            if (
                group.launcher === launcher &&
                !group.instances.some(
                    (entry) =>
                        folderKey(instancePathOf(entry)) === folderKey(instancePathOf(instance)),
                )
            )
                group.instances.push(instance)
        }
        if (own.length > 0) {
            groups.push({ key: `launcher:${launcher.name}`, launcher, instances: own })
        }
    }
    return groups
}
