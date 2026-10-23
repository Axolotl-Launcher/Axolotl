import { reactive } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'
import type { GameInstance } from '../../../../lib/types'
import { PREVIEW_APP_DIRECTORY } from '../../state'

/**
 * The preview Mac's disk, as the native folder dialogs and Finder show it.
 *
 * It is the machine the other pages describe: the home folder the import flow
 * scans (the official launcher's `minecraft` game directory, a Prism Launcher
 * data folder, the same downloads), the JDKs of the Java page, and what the
 * launcher keeps in its data directory (`DirectoryInfo`, app-lib
 * `state/dirs.rs`): caches, metadata, instance folders and the per-channel app
 * databases with their backups (`Backups/app-db/<channel>`, named and pruned as
 * `state/db_backup.rs` does). Contents, sizes and dates are example local
 * values; mod files are real files (names and sizes) of the snapshot's versions.
 */

export const HOME = '/Users/steve'
export const APP_SUPPORT = `${HOME}/Library/Application Support`
/** `dirs::download_dir()`. */
export const DOWNLOADS_DIR = `${HOME}/Downloads`
/** `app_db_backup_dir()`: `<settings dir>/Backups/app-db`. */
export const APP_DB_BACKUPS_DIR = `${PREVIEW_APP_DIRECTORY}/Backups/app-db`
/** `DirectoryInfo::instances_dir()`. */
export const PROFILES_DIR = `${PREVIEW_APP_DIRECTORY}/profiles`

export type FsMeta =
    | {
          type: 'version-json'
          gameVersion: string
          loader: string
          loaderVersion: string | null
      }
    | {
          type: 'mod'
          projectId: string
          versionId: string
          title: string
          versionNumber: string
      }

export interface FsNode {
    name: string
    kind: 'dir' | 'file'
    modified: string
    size: number
    children?: FsNode[]
    meta?: FsMeta
    /** Hidden from Finder and the open panel (`~/Library`, the Unix folders). */
    hidden?: boolean
    /** A bundle Finder shows as one item (apps, JDKs); panels do not enter it. */
    package?: boolean
}

function dir(
    name: string,
    modified: string,
    children: FsNode[] = [],
    extra: Partial<FsNode> = {},
): FsNode {
    return { name, kind: 'dir', modified, size: 0, children, ...extra }
}

function file(name: string, modified: string, size: number, meta?: FsMeta): FsNode {
    return { name, kind: 'file', modified, size, meta }
}

function bundle(name: string, modified: string, size: number): FsNode {
    return { name, kind: 'dir', modified, size, children: [], package: true }
}

export function isDirectory(node: FsNode | null | undefined): node is FsNode {
    return !!node && node.kind === 'dir'
}

export function joinPath(base: string, name: string): string {
    return base === '/' ? `/${name}` : `${base}/${name}`
}

export function parentPath(path: string): string {
    const parts = path.split('/').filter(Boolean)
    parts.pop()
    return `/${parts.join('/')}`
}

export function baseName(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? ''
}

/** Ancestors of a path, nearest first, ending with `/`. */
export function ancestors(path: string): string[] {
    const result: string[] = []
    let current = path
    while (current !== '/') {
        result.push(current)
        current = parentPath(current)
    }
    result.push('/')
    return result
}

export function childNamed(node: FsNode | null | undefined, name: string): FsNode | null {
    // APFS is case-insensitive by default.
    return node?.children?.find((child) => child.name.toLowerCase() === name.toLowerCase()) ?? null
}

/**
 * `canonicalize` as the launcher process sees it: relative paths resolve
 * against `/` (the working directory of an app started from Finder), `.` and
 * `..` collapse, and the result carries the on-disk names. Null when missing.
 */
export function canonicalize(root: FsNode, raw: string): { path: string; node: FsNode } | null {
    if (!raw) return null
    const stack: string[] = []
    for (const part of raw.split('/')) {
        if (!part || part === '.') continue
        if (part === '..') stack.pop()
        else stack.push(part)
    }
    let node: FsNode = root
    const names: string[] = []
    for (const part of stack) {
        const next = childNamed(node, part)
        if (!next) return null
        node = next
        names.push(next.name)
    }
    return { path: `/${names.join('/')}`, node }
}

export function findNode(root: FsNode, path: string): FsNode | null {
    return canonicalize(root, path || '/')?.node ?? null
}

/** Finder's name order: case-insensitive, numbers compared by value. */
export function sortedChildren(node: FsNode | null | undefined): FsNode[] {
    return [...(node?.children ?? [])].sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' }),
    )
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
    const data = await import('../../../../data')
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
        file('level.dat', modified, 2_871),
        file('level.dat_old', modified, 2_865),
        file('session.lock', modified, 3),
        dir('region', modified, [
            file('r.0.0.mca', modified, regionBytes),
            file('r.-1.0.mca', modified, Math.round(regionBytes * 0.62)),
        ]),
        dir('playerdata', modified, [file('5c1a9d6e.dat', modified, 1_204)]),
    ])
}

function versionJson(
    id: string,
    modified: string,
    size: number,
    gameVersion: string,
    loader = 'vanilla',
    loaderVersion: string | null = null,
) {
    return file(`${id}.json`, modified, size, {
        type: 'version-json',
        gameVersion,
        loader,
        loaderVersion,
    })
}

function prismInstance(folder: string, modified: string, mods: FsNode[], worlds: FsNode[]): FsNode {
    return dir(folder, modified, [
        file('instance.cfg', modified, 412),
        file('mmc-pack.json', modified, 318),
        dir('minecraft', modified, [
            dir('config', modified),
            dir('mods', modified, mods),
            dir('resourcepacks', modified),
            dir('saves', modified, worlds),
            dir('screenshots', modified),
            file('options.txt', modified, 5_312),
            file('servers.dat', modified, 102),
        ]),
    ])
}

/**
 * The launcher's data directory: the Release database this install has always
 * used, and the snapshots `backup_app_db_for_update` and the pre-migration
 * backup took along the way (1.9.7 → 1.9.8 → 1.9.9 → 1.10.0). Pruning keeps the
 * newest three, so the 1.9.8 update's snapshots are gone.
 */
function launcherDataDirectory(): FsNode {
    const releaseUpdate = '2026-10-23T14:20:58Z'
    const now = '2026-10-23T19:12:00Z'
    return dir('red.ghs.axolotl', now, [
        dir('Backups', releaseUpdate, [
            dir('app-db', releaseUpdate, [
                dir('release', releaseUpdate, [
                    file('app-db-before-1.9.9-from-1.9.8.db', '2026-10-20T12:31:49Z', 3_104_768),
                    file(
                        'app-db-before-update-release-1.10.0-1792765213000468211.db',
                        '2026-10-23T14:20:13Z',
                        3_268_608,
                    ),
                    file('app-db-before-1.10.0-from-1.9.9.db', releaseUpdate, 3_272_704),
                ]),
            ]),
            dir('instances', '2026-10-02T18:31:00Z'),
        ]),
        dir('caches', now, [
            dir('content-store', now),
            dir('icons', '2026-10-02T18:31:00Z'),
            dir('resources', now, [
                dir('release', now, [file('resource-cache.db', now, 1_867_776)]),
            ]),
        ]),
        dir('icons', '2026-10-02T18:31:00Z'),
        dir('launcher_logs', now, [
            // `rotating_log_path`: one file per session, `session_<local start time>.log`.
            file('session_20261022_183044.log', '2026-10-22T23:59:00Z', 1_207_808),
            file('session_20261023_191200.log', now, 486_912),
        ]),
        dir('meta', now, [
            dir('assets', now, [dir('indexes', now), dir('objects', now)]),
            dir('java_versions', '2026-06-18T19:12:00Z', [
                bundle('eclipse-jdk-25', '2026-06-18T19:12:00Z', 0),
            ]),
            dir('libraries', now),
            dir('log_configs', now),
            dir('natives', now),
            dir('versions', now),
        ]),
        dir('profiles', now),
        dir('release', now, [
            file('app.db', now, 3_420_160),
            file('app.db-shm', now, 32_768),
            file('app.db-wal', now, 1_134_592),
        ]),
        dir('servers', '2026-10-02T18:31:00Z'),
    ])
}

async function buildFileSystem(): Promise<FsNode> {
    const [minecraftMods, prismFabricMods, prismForgeMods] = await Promise.all([
        resolveMods(['AANobbMI', '1bokaNcj', 'aC3cM3Vq'], '1.21.1', 'fabric'),
        resolveMods(
            ['AANobbMI', '1bokaNcj', '5ZwdcRci', '51shyZVL', 'aC3cM3Vq', '1IjD5062'],
            '1.21.1',
            'fabric',
        ),
        resolveMods(
            ['e0M1UDsY', 'MBAkmtvl', 'MPCX6s5C', 'aC3cM3Vq', 'BVzZfTc1'],
            '1.20.1',
            'forge',
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
        dir('mods', '2026-10-02T18:30:00Z', modNodes(minecraftMods, '2026-10-02T18:30:00Z')),
        dir('resourcepacks', '2026-08-14T12:00:00Z'),
        dir('saves', minecraftModified, [world('New World', minecraftModified, 4_194_304)]),
        dir('screenshots', '2026-10-18T20:44:00Z'),
        dir('versions', minecraftModified, [
            dir('26.3', '2026-10-20T21:00:00Z', [
                file('26.3.jar', '2026-10-20T21:00:00Z', 29_360_128),
                versionJson('26.3', '2026-10-20T21:00:00Z', 28_412, '26.3'),
            ]),
            dir('1.21.1', '2026-09-30T17:12:00Z', [
                file('1.21.1.jar', '2026-09-30T17:12:00Z', 26_214_400),
                versionJson('1.21.1', '2026-09-30T17:12:00Z', 27_104, '1.21.1'),
            ]),
            dir('fabric-loader-0.19.5-1.21.1', '2026-10-02T18:26:00Z', [
                versionJson(
                    'fabric-loader-0.19.5-1.21.1',
                    '2026-10-02T18:26:00Z',
                    3_418,
                    '1.21.1',
                    'fabric',
                    '0.19.5',
                ),
            ]),
        ]),
        file('launcher_profiles.json', minecraftModified, 1_962),
        file('options.txt', minecraftModified, 5_544),
    ])

    const prismModified = '2026-10-19T09:15:00Z'
    const prismLauncher = dir('PrismLauncher', prismModified, [
        dir('assets', prismModified),
        dir('icons', '2026-06-02T10:00:00Z'),
        dir('instances', prismModified, [
            prismInstance(
                'Fabric 1.21.1',
                '2026-10-11T19:42:00Z',
                modNodes(prismFabricMods, '2026-10-11T19:42:00Z'),
                [world('Survival', '2026-10-11T19:42:00Z', 9_437_184)],
            ),
            prismInstance(
                'Forge 1.20.1',
                '2026-09-27T15:08:00Z',
                modNodes(prismForgeMods, '2026-09-27T15:08:00Z'),
                [world('Adventure', '2026-09-27T15:08:00Z', 6_291_456)],
            ),
            prismInstance(
                '26.3',
                prismModified,
                [],
                [world('New World', prismModified, 3_145_728)],
            ),
        ]),
        dir('libraries', prismModified),
        dir('logs', prismModified, [file('PrismLauncher-0.log', prismModified, 61_440)]),
        dir('meta', prismModified),
        dir('themes', '2026-06-02T10:00:00Z'),
        file('accounts.json', prismModified, 1_104),
        file('prismlauncher.cfg', prismModified, 2_388),
    ])

    const home = dir('steve', '2026-10-22T18:40:00Z', [
        dir('Applications', '2026-05-20T10:00:00Z'),
        dir('Desktop', '2026-10-18T20:44:00Z', [
            file('Screenshot 2026-10-18 at 20.44.12.png', '2026-10-18T20:44:00Z', 2_514_944),
        ]),
        dir('Documents', '2026-09-03T14:21:00Z'),
        dir('Downloads', '2026-10-22T18:40:00Z', [
            file('Axolotl.Launcher_1.9.7_universal.dmg', '2026-10-02T18:24:00Z', 137_897_714),
            // Fabulously Optimized 14.1.1 as Modrinth serves it (data/project/1KVo5zza.json).
            file('Fabulously.Optimized-v14.1.1.mrpack', '2026-10-22T18:40:00Z', 192_991),
        ]),
        dir(
            'Library',
            '2026-10-20T21:03:00Z',
            [
                dir('Application Support', '2026-10-23T19:12:00Z', [
                    dir('CrashReporter', '2026-10-07T08:20:00Z', [
                        file('Intervals_0D7F2F70.plist', '2026-10-07T08:20:00Z', 1_018),
                    ]),
                    minecraft,
                    prismLauncher,
                    launcherDataDirectory(),
                ]),
                dir('Caches', '2026-10-22T18:40:00Z'),
                dir('Preferences', '2026-10-22T18:40:00Z'),
            ],
            { hidden: true },
        ),
        dir('Movies', '2026-05-20T10:00:00Z'),
        dir('Music', '2026-05-20T10:00:00Z'),
        dir('Pictures', '2026-08-01T10:00:00Z'),
        dir('Public', '2026-05-20T10:00:00Z', [dir('Drop Box', '2026-05-20T10:00:00Z')]),
    ])

    const systemModified = '2026-08-06T07:12:00Z'
    return dir('', '2026-05-20T10:00:00Z', [
        dir('Applications', '2026-10-16T11:05:00Z', [
            bundle('Axolotl Launcher.app', '2026-10-16T11:05:00Z', 121_634_816),
            bundle('Minecraft.app', '2026-04-02T18:20:00Z', 389_021_696),
            bundle('Safari.app', systemModified, 18_874_368),
        ]),
        dir('Library', '2026-03-12T16:48:00Z', [
            dir('Application Support', '2026-03-12T16:48:00Z'),
            dir('Fonts', '2026-05-20T10:00:00Z'),
            dir('Java', '2026-03-12T16:48:00Z', [
                dir('Extensions', '2025-11-04T09:31:00Z'),
                dir('JavaVirtualMachines', '2026-03-12T16:48:00Z', [
                    bundle('temurin-21.jdk', '2026-03-12T16:48:00Z', 0),
                    bundle('zulu-17.jdk', '2025-11-04T09:31:00Z', 0),
                ]),
            ]),
            dir('Preferences', '2026-10-22T18:40:00Z'),
        ]),
        dir('System', systemModified, [dir('Library', systemModified)]),
        dir('Users', '2026-05-20T10:00:00Z', [dir('Shared', '2026-05-20T10:00:00Z'), home]),
        dir('bin', systemModified, [], { hidden: true }),
        dir('opt', '2026-05-20T10:00:00Z', [], { hidden: true }),
        dir('private', systemModified, [], { hidden: true }),
        dir('sbin', systemModified, [], { hidden: true }),
        dir('usr', systemModified, [], { hidden: true }),
    ])
}

export interface MacFileSystem {
    /** The disk once built (the snapshot's mod files load asynchronously). */
    root: FsNode | null
    /** NSOpenPanel's remembered directory for the app. */
    lastDirectory: string
    /** Whether hidden items are shown (Shift-Command-Period). */
    showHidden: boolean
    load: () => Promise<FsNode>
    /**
     * Builds the launcher's data directory (`red.ghs.axolotl`) from the live
     * launcher state. The storage page sets it, so its storage tree, Finder and
     * the open panel show the same folder; without it the profiles follow the
     * library and the rest stays as built.
     */
    launcherDirectory: (() => FsNode) | null
}

/** The preview Mac's disk, built on first use and kept for the life of the window. */
export function useMacFileSystem(): MacFileSystem {
    return usePreviewDomain('settings:mac-fs', () => {
        let building: Promise<FsNode> | null = null
        const fs: MacFileSystem = reactive({
            root: null as FsNode | null,
            lastDirectory: HOME,
            showHidden: false,
            launcherDirectory: null as (() => FsNode) | null,
            load: () => {
                building ??= buildFileSystem().then((root) => {
                    fs.root = root
                    return fs.root
                })
                return building
            },
        })
        return { value: fs }
    })
}

/**
 * Brings the launcher's own folder up to date before Finder or the open panel
 * shows the disk: the storage page's live tree when it provides one, else the
 * instance folders the library currently has.
 */
export function refreshDisk(fs: MacFileSystem, instances: GameInstance[]) {
    const root = fs.root
    if (!root) return
    const parent = findNode(root, APP_SUPPORT)
    if (fs.launcherDirectory && parent) {
        const fresh = fs.launcherDirectory()
        const kept = (parent.children ?? []).filter((child) => child.name !== fresh.name)
        parent.children = [...kept, fresh]
        return
    }
    syncProfiles(root, instances)
}

/**
 * The instance folders under `profiles`: one per managed instance, named after
 * its profile path, as the library currently has them.
 */
export function syncProfiles(root: FsNode, instances: GameInstance[]) {
    const profiles = findNode(root, PROFILES_DIR)
    if (!profiles) return
    const managed = instances.filter((instance) => instance.path.startsWith(`${PROFILES_DIR}/`))
    const names = new Set(managed.map((instance) => baseName(instance.path)))
    const kept = (profiles.children ?? []).filter((child) => names.has(child.name))
    for (const instance of managed) {
        const name = baseName(instance.path)
        if (kept.some((child) => child.name === name)) continue
        const modified = instance.modified || instance.created
        kept.push(
            dir(name, modified, [
                dir('config', modified),
                dir('logs', modified),
                dir('mods', modified),
                dir('resourcepacks', modified),
                dir('saves', modified),
                dir('screenshots', modified),
                file('options.txt', modified, 5_288),
            ]),
        )
    }
    profiles.children = kept
}

/** `create_dir_all`: creates the missing folders of an absolute path. */
export function createDirAll(root: FsNode, path: string) {
    let node = root
    for (const part of path.split('/').filter(Boolean)) {
        let next = childNamed(node, part)
        if (!next) {
            next = dir(part, new Date().toISOString())
            node.children = [...(node.children ?? []), next]
        }
        node = next
    }
}

/** Finder's New Folder; returns null when the name is already taken. */
export function createFolder(root: FsNode, parent: string, name: string): FsNode | null {
    const node = findNode(root, parent)
    if (!isDirectory(node) || childNamed(node, name)) return null
    const created = dir(name, new Date().toISOString())
    node.children = [...(node.children ?? []), created]
    node.modified = created.modified
    return created
}
