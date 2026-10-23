/**
 * The preview Mac's file system, as far as the import flow can see it.
 *
 * The desktop app picks folders through the native macOS open panel and then
 * classifies and scans them on disk. The preview simulates that disk: the user's
 * home with the data folders the launchers keep in `~/Library/Application Support`
 * on macOS (`dirs::data_dir()`, where the app's importer looks): the official
 * launcher's `minecraft` game directory and a Prism Launcher data folder. Instance
 * names and worlds are example values; game versions come from the snapshot and mod
 * jars are real files (names and sizes) of the snapshot's project versions.
 */

export const HOME = '/Users/steve'
export const APP_SUPPORT = `${HOME}/Library/Application Support`

export type FsMeta =
    | {
          type: 'version-json'
          id: string
          gameVersion: string
          loader: string
          loaderVersion: string | null
      }
    | { type: 'mmc-pack'; gameVersion: string; loader: string; loaderVersion: string | null }
    | {
          type: 'mod'
          projectId: string
          versionId: string
          title: string
          versionNumber: string
      }
    | { type: 'level' }
    /** A Modrinth pack file of the snapshot (`data/modpacks.json`). */
    | { type: 'mrpack'; projectId: string; versionId: string }

export interface FsNode {
    name: string
    kind: 'dir' | 'file'
    modified: string
    size: number
    children?: FsNode[]
    meta?: FsMeta
}

function dir(name: string, modified: string, children: FsNode[] = []): FsNode {
    return { name, kind: 'dir', modified, size: 0, children }
}

function file(name: string, modified: string, size: number, meta?: FsMeta): FsNode {
    return { name, kind: 'file', modified, size, meta }
}

export function joinPath(...parts: string[]): string {
    return parts
        .filter(Boolean)
        .join('/')
        .replace(/\/{2,}/g, '/')
}

export function baseName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).pop() ?? path
}

export function parentPath(path: string): string {
    const parts = path.split('/').filter(Boolean)
    parts.pop()
    return `/${parts.join('/')}`
}

export function findNode(root: FsNode, path: string): FsNode | null {
    const parts = path.split('/').filter(Boolean)
    let node: FsNode | undefined = root
    for (const part of parts) {
        // APFS is case-insensitive by default, as `Path::join(...).exists()` sees it.
        node = node?.children?.find((child) => child.name.toLowerCase() === part.toLowerCase())
        if (!node) return null
    }
    return node ?? null
}

export function childNamed(node: FsNode | null | undefined, name: string): FsNode | null {
    return node?.children?.find((child) => child.name.toLowerCase() === name.toLowerCase()) ?? null
}

export function hasFile(node: FsNode | null | undefined, name: string): boolean {
    const child = childNamed(node, name)
    return !!child && child.kind === 'file'
}

/** Recursive file count and size of a node. */
export function measure(node: FsNode): { files: number; bytes: number } {
    if (node.kind === 'file') return { files: 1, bytes: node.size }
    let files = 0
    let bytes = 0
    for (const child of node.children ?? []) {
        const inner = measure(child)
        files += inner.files
        bytes += inner.bytes
    }
    return { files, bytes }
}

interface ModFile {
    projectId: string
    versionId: string
    title: string
    versionNumber: string
    filename: string
    size: number
}

async function resolveMods(ids: string[], gameVersion: string, loader: string) {
    const data = await import('../../data')
    const resolved: ModFile[] = []
    for (const id of ids) {
        const project = await data.loadProject(id).catch(() => null)
        if (!project) continue
        const version = project.versions.find(
            (entry) => entry.game_versions.includes(gameVersion) && entry.loaders.includes(loader),
        )
        const primary = version?.files.find((entry) => entry.primary) ?? version?.files[0]
        if (!version || !primary) continue
        resolved.push({
            projectId: project.id,
            versionId: version.id,
            title: project.title,
            versionNumber: version.version_number,
            filename: primary.filename,
            size: primary.size,
        })
    }
    return resolved
}

function modNodes(mods: ModFile[], modified: string): FsNode[] {
    return mods.map((mod) =>
        file(mod.filename, modified, mod.size, {
            type: 'mod',
            projectId: mod.projectId,
            versionId: mod.versionId,
            title: mod.title,
            versionNumber: mod.versionNumber,
        }),
    )
}

function world(name: string, modified: string, regionBytes: number): FsNode {
    return dir(name, modified, [
        file('level.dat', modified, 2_871, { type: 'level' }),
        file('level.dat_old', modified, 2_865),
        file('session.lock', modified, 3),
        dir('region', modified, [
            file('r.0.0.mca', modified, regionBytes),
            file('r.-1.0.mca', modified, Math.round(regionBytes * 0.62)),
        ]),
        dir('playerdata', modified, [file('5c1a9d6e.dat', modified, 1_204)]),
    ])
}

interface PrismInstanceSpec {
    folder: string
    gameVersion: string
    loader: string
    loaderVersion: string | null
    mods: string[]
    worlds: { name: string; regionBytes: number }[]
    modified: string
}

const prismLoaderUids: Record<string, string> = {
    fabric: 'net.fabricmc.fabric-loader',
    forge: 'net.minecraftforge',
    neoforge: 'net.neoforged',
    quilt: 'org.quiltmc.quilt-loader',
}

async function prismInstance(spec: PrismInstanceSpec): Promise<FsNode> {
    const mods =
        spec.loader === 'vanilla' ? [] : await resolveMods(spec.mods, spec.gameVersion, spec.loader)
    const components = [{ uid: 'net.minecraft', version: spec.gameVersion }]
    if (spec.loader !== 'vanilla' && spec.loaderVersion) {
        components.push({
            uid: prismLoaderUids[spec.loader] ?? spec.loader,
            version: spec.loaderVersion,
        })
    }
    const mmcPack = JSON.stringify({ components, formatVersion: 1 })
    return dir(spec.folder, spec.modified, [
        file('instance.cfg', spec.modified, 412),
        file('mmc-pack.json', spec.modified, mmcPack.length, {
            type: 'mmc-pack',
            gameVersion: spec.gameVersion,
            loader: spec.loader,
            loaderVersion: spec.loaderVersion,
        }),
        dir('minecraft', spec.modified, [
            dir(
                'config',
                spec.modified,
                mods.map((mod) =>
                    file(`${mod.projectId.toLowerCase()}.json`, spec.modified, 1_536),
                ),
            ),
            dir('mods', spec.modified, modNodes(mods, spec.modified)),
            dir('resourcepacks', spec.modified),
            dir(
                'saves',
                spec.modified,
                spec.worlds.map((entry) => world(entry.name, spec.modified, entry.regionBytes)),
            ),
            dir('screenshots', spec.modified),
            file('options.txt', spec.modified, 5_312),
            file('servers.dat', spec.modified, 102),
        ]),
    ])
}

let cached: Promise<FsNode> | null = null

/** Builds the simulated disk once per page; the folder panel and scans share it. */
export function loadMacFileSystem(): Promise<FsNode> {
    if (!cached) cached = buildFileSystem()
    return cached
}

async function buildFileSystem(): Promise<FsNode> {
    const [fabricMods, prism] = await Promise.all([
        resolveMods(['AANobbMI', '1bokaNcj', 'aC3cM3Vq'], '1.21.1', 'fabric'),
        Promise.all(
            (
                [
                    {
                        folder: 'Fabric 1.21.1',
                        gameVersion: '1.21.1',
                        loader: 'fabric',
                        loaderVersion: '0.16.9',
                        mods: [
                            'AANobbMI',
                            '1bokaNcj',
                            '5ZwdcRci',
                            '51shyZVL',
                            'aC3cM3Vq',
                            '1IjD5062',
                        ],
                        worlds: [{ name: 'Survival', regionBytes: 9_437_184 }],
                        modified: '2026-10-11T19:42:00Z',
                    },
                    {
                        folder: 'Forge 1.20.1',
                        gameVersion: '1.20.1',
                        loader: 'forge',
                        loaderVersion: '47.4.10',
                        mods: ['e0M1UDsY', 'MBAkmtvl', 'MPCX6s5C', 'aC3cM3Vq', 'BVzZfTc1'],
                        worlds: [{ name: 'Adventure', regionBytes: 6_291_456 }],
                        modified: '2026-09-27T15:08:00Z',
                    },
                    {
                        folder: '26.3',
                        gameVersion: '26.3',
                        loader: 'vanilla',
                        loaderVersion: null,
                        mods: [],
                        worlds: [{ name: 'New World', regionBytes: 3_145_728 }],
                        modified: '2026-10-19T09:15:00Z',
                    },
                ] satisfies PrismInstanceSpec[]
            ).map(prismInstance),
        ),
    ])

    const minecraftModified = '2026-10-20T21:03:00Z'
    const minecraft = dir('minecraft', minecraftModified, [
        dir('assets', '2026-10-20T21:01:00Z', [
            dir('indexes', '2026-10-20T21:01:00Z', [
                file('27.json', '2026-10-20T21:01:00Z', 448_216),
            ]),
            dir('objects', '2026-10-20T21:01:00Z'),
        ]),
        dir('libraries', '2026-10-20T21:01:00Z'),
        dir('logs', minecraftModified, [file('latest.log', minecraftModified, 48_211)]),
        dir('mods', '2026-10-02T18:30:00Z', modNodes(fabricMods, '2026-10-02T18:30:00Z')),
        dir('resourcepacks', '2026-08-14T12:00:00Z'),
        dir('saves', minecraftModified, [world('New World', minecraftModified, 4_194_304)]),
        dir('screenshots', '2026-10-18T20:44:00Z'),
        dir('versions', minecraftModified, [
            dir('26.3', '2026-10-20T21:00:00Z', [
                file('26.3.jar', '2026-10-20T21:00:00Z', 29_360_128),
                file('26.3.json', '2026-10-20T21:00:00Z', 28_412, {
                    type: 'version-json',
                    id: '26.3',
                    gameVersion: '26.3',
                    loader: 'vanilla',
                    loaderVersion: null,
                }),
            ]),
            dir('1.21.1', '2026-09-30T17:12:00Z', [
                file('1.21.1.jar', '2026-09-30T17:12:00Z', 26_214_400),
                file('1.21.1.json', '2026-09-30T17:12:00Z', 27_104, {
                    type: 'version-json',
                    id: '1.21.1',
                    gameVersion: '1.21.1',
                    loader: 'vanilla',
                    loaderVersion: null,
                }),
            ]),
            dir('fabric-loader-0.19.5-1.21.1', '2026-10-02T18:26:00Z', [
                file('fabric-loader-0.19.5-1.21.1.json', '2026-10-02T18:26:00Z', 3_418, {
                    type: 'version-json',
                    id: 'fabric-loader-0.19.5-1.21.1',
                    gameVersion: '1.21.1',
                    loader: 'fabric',
                    loaderVersion: '0.19.5',
                }),
            ]),
        ]),
        file('launcher_profiles.json', minecraftModified, 1_962),
        file('options.txt', minecraftModified, 5_544),
    ])

    const prismModified = '2026-10-19T09:15:00Z'
    const prismLauncher = dir('PrismLauncher', prismModified, [
        dir('assets', prismModified),
        dir('icons', '2026-06-02T10:00:00Z'),
        dir('instances', prismModified, prism),
        dir('libraries', prismModified),
        dir('logs', prismModified, [file('PrismLauncher-0.log', prismModified, 61_440)]),
        dir('meta', prismModified),
        dir('themes', '2026-06-02T10:00:00Z'),
        file('accounts.json', prismModified, 1_104),
        file('prismlauncher.cfg', prismModified, 2_388),
    ])

    const crashReporter = dir('CrashReporter', '2026-10-07T08:20:00Z', [
        file('Intervals_0D7F2F70.plist', '2026-10-07T08:20:00Z', 1_018),
    ])

    const home = dir('steve', '2026-10-20T21:03:00Z', [
        dir('Desktop', '2026-10-18T20:44:00Z', [
            file('Screenshot 2026-10-18 at 20.44.12.png', '2026-10-18T20:44:00Z', 2_514_944),
        ]),
        dir('Documents', '2026-09-03T14:21:00Z'),
        dir('Downloads', '2026-10-22T18:40:00Z', [
            file('Axolotl.Launcher_1.9.7_universal.dmg', '2026-10-02T18:24:00Z', 137_897_714),
            // Fabulously Optimized 14.1.1 as Modrinth serves it (data/project/1KVo5zza.json).
            file('Fabulously.Optimized-v14.1.1.mrpack', '2026-10-22T18:40:00Z', 192_991, {
                type: 'mrpack',
                projectId: '1KVo5zza',
                versionId: 'uQxD0ENB',
            }),
        ]),
        dir('Library', '2026-10-20T21:03:00Z', [
            dir('Application Support', '2026-10-20T21:03:00Z', [
                crashReporter,
                minecraft,
                prismLauncher,
            ]),
        ]),
        dir('Movies', '2026-05-20T10:00:00Z'),
        dir('Music', '2026-05-20T10:00:00Z'),
        dir('Pictures', '2026-08-01T10:00:00Z'),
        dir('Public', '2026-05-20T10:00:00Z'),
    ])

    return dir('', '2026-05-20T10:00:00Z', [dir('Users', '2026-05-20T10:00:00Z', [home])])
}
