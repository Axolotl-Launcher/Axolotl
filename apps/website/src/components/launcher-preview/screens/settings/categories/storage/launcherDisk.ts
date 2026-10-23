import { tree as fabricServerTree } from '../../../../data/servers/fabric-26.3.json'
import { tree as vanillaServerTree } from '../../../../data/servers/vanilla-26.3.json'
import type { ContentItem } from '../../../../lib/state/downloads'
import type { PreviewState } from '../../../../lib/state/index'
import type { ManagedServer } from '../../../../lib/state/servers'
import type { GameInstance } from '../../../../lib/types'
import { PREVIEW_APP_DIRECTORY } from '../../state'

/*
 * The preview Mac's disk, as far as the storage page can see it.
 *
 * The desktop app measures the launcher's folders on disk
 * (packages/app-lib/src/storage). The preview builds those folders from its own
 * data instead: every instance holds the files of `preview.downloads.contents`
 * (real names and sizes), the worlds Home lists, the screenshots of the
 * screenshot page and the logs its play time implies; `meta/` holds the game
 * versions, loaders, libraries, assets and Java runtimes those instances need
 * (plus the leftovers of the pack update Fabulously Optimized went through);
 * `caches/` the downloads the installs left behind. Sizes of local files are
 * example values derived from the content; the folder layout and every
 * classification follow app-lib.
 *
 * The parts the cleanups change (`meta/`, `caches/`) are kept in the storage
 * domain for the life of the window; everything else is rebuilt from the
 * preview's live state on each read, so installs, deletions and new worlds
 * show up in the next scan.
 */

export const HOME = '/Users/steve'
export const ROOT = PREVIEW_APP_DIRECTORY
export const VOLUMES = '/Volumes'
export const EXTERNAL_VOLUME = 'Backup'

const KIB = 1024
const MIB = 1024 * 1024
const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/* ---- Entries ------------------------------------------------------------- */

export interface DiskFile {
    kind: 'file'
    name: string
    size: number
    modified: number
}

export interface DiskDir {
    kind: 'dir'
    name: string
    modified: number
    /** `null` for a folder whose entries are not modelled one by one (see `summary`). */
    children: DiskEntry[] | null
    /** Totals of a folder whose entries are not listed (`children === null`). */
    summary?: { files: number; bytes: number }
}

/** A symbolic link (or junction) to another folder. */
export interface DiskLink {
    kind: 'link'
    name: string
    modified: number
    target: string
}

export type DiskEntry = DiskFile | DiskDir | DiskLink

export function file(name: string, size: number, modified: number): DiskFile {
    return { kind: 'file', name, size: Math.max(0, Math.round(size)), modified }
}

export function dir(name: string, modified: number, children: DiskEntry[] = []): DiskDir {
    return { kind: 'dir', name, modified, children }
}

/** A folder whose entries are counted but not modelled one by one. */
export function packed(name: string, modified: number, files: number, bytes: number): DiskDir {
    return {
        kind: 'dir',
        name,
        modified,
        children: null,
        summary: { files: Math.max(0, Math.round(files)), bytes: Math.max(0, Math.round(bytes)) },
    }
}

export function joinPath(...parts: string[]): string {
    return parts
        .filter((part) => part !== '')
        .join('/')
        .replace(/\/{2,}/g, '/')
}

export function baseName(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? path
}

export function parentPath(path: string): string {
    const parts = path.split('/').filter(Boolean)
    parts.pop()
    return `/${parts.join('/')}`
}

/** Looks an absolute path up, following no links. */
export function findEntry(root: DiskDir, path: string): DiskEntry | null {
    let entry: DiskEntry = root
    for (const part of path.split('/').filter(Boolean)) {
        if (entry.kind !== 'dir' || !entry.children) return null
        const next: DiskEntry | undefined = entry.children.find((child) => child.name === part)
        if (!next) return null
        entry = next
    }
    return entry
}

/** Adds (or replaces) the entry at `path`, creating missing parent folders. */
export function placeEntry(root: DiskDir, path: string, entry: DiskEntry, modified: number) {
    const parts = path.split('/').filter(Boolean)
    parts.pop()
    let current = root
    for (const part of parts) {
        current.children ??= []
        let next = current.children.find((child) => child.name === part)
        if (!next || next.kind !== 'dir') {
            next = dir(part, modified)
            current.children = [...current.children.filter((child) => child.name !== part), next]
        }
        current = next
    }
    current.children = [
        ...(current.children ?? []).filter((child) => child.name !== entry.name),
        entry,
    ]
}

/** Bytes and file count below an entry (links count as nothing here). */
export function measure(entry: DiskEntry): { files: number; bytes: number } {
    if (entry.kind === 'file') return { files: 1, bytes: entry.size }
    if (entry.kind === 'link') return { files: 0, bytes: 0 }
    if (!entry.children) return entry.summary ?? { files: 0, bytes: 0 }
    let files = 0
    let bytes = 0
    for (const child of entry.children) {
        const sub = measure(child)
        files += sub.files
        bytes += sub.bytes
    }
    return { files, bytes }
}

/* ---- Deterministic example values --------------------------------------- */

function hashString(text: string): number {
    let hash = 0x811c9dc5
    for (let index = 0; index < text.length; index += 1) {
        hash ^= text.charCodeAt(index)
        hash = Math.imul(hash, 0x01000193)
    }
    return hash >>> 0
}

/** A seeded generator (mulberry32), so every rebuild yields the same values. */
export function randomFor(seed: string) {
    let state = hashString(seed) || 1
    const next = () => {
        state = (state + 0x6d2b79f5) | 0
        let value = Math.imul(state ^ (state >>> 15), 1 | state)
        value = (value + Math.imul(value ^ (value >>> 7), 61 | value)) ^ value
        return ((value ^ (value >>> 14)) >>> 0) / 4294967296
    }
    return {
        next,
        between: (min: number, max: number) => min + (max - min) * next(),
        int: (min: number, max: number) => Math.floor(min + (max - min + 1) * next()),
    }
}

/** A hex digest-like name (sha1 = 40, sha512 = 128 characters). */
export function hexName(seed: string, length: number): string {
    let out = ''
    let round = 0
    while (out.length < length) {
        out += hashString(`${seed}:${round}`).toString(16).padStart(8, '0')
        round += 1
    }
    return out.slice(0, length)
}

function pad(value: number, width = 2) {
    return String(value).padStart(width, '0')
}

/** `YYYY-MM-DD` in the Mac's local time. */
function localDate(time: number) {
    const date = new Date(time)
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** Minecraft's screenshot name, `2026-10-23_14.05.31.png`. */
export function screenshotName(time: number) {
    const date = new Date(time)
    return `${localDate(time)}_${pad(date.getHours())}.${pad(date.getMinutes())}.${pad(date.getSeconds())}.png`
}

/** The launcher's session log name, `session_20261023_140531.log`. */
export function sessionLogName(time: number) {
    const date = new Date(time)
    return `session_${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}_${pad(
        date.getHours(),
    )}${pad(date.getMinutes())}${pad(date.getSeconds())}.log`
}

/* ---- Game versions -------------------------------------------------------- */

/** One launch target of an instance, as `channel_references` derives it. */
export interface LaunchManifest {
    /** `versions/<id>`: the game version, or `<game>-<loader version>` for a loader. */
    id: string
    game: string
    loader: GameInstance['loader']
    loaderVersion: string | null
}

export function manifestFor(
    instance: Pick<GameInstance, 'game_version' | 'loader' | 'loader_version'>,
) {
    const loaderVersion = instance.loader === 'vanilla' ? null : (instance.loader_version ?? null)
    return {
        id: loaderVersion ? `${instance.game_version}-${loaderVersion}` : instance.game_version,
        game: instance.game_version,
        loader: instance.loader,
        loaderVersion,
    } satisfies LaunchManifest
}

/**
 * A Fabric 26.1.2 instance the user created after switching to the Beta
 * channel and deleted again: deleting an instance never touches shared files,
 * so its game version, Fabric profile, natives and the libraries and assets
 * only that version lists are still in `meta/`, and the mods it installed are
 * still in the content store. No channel database references them any more.
 */
export const LEFTOVER_MANIFESTS: LaunchManifest[] = [
    { id: '26.1.2-0.19.5', game: '26.1.2', loader: 'fabric', loaderVersion: '0.19.5' },
]

/** When that instance was created and deleted. */
export const LEFTOVER_CREATED_AGO = 31 * 24 * 60 * 60 * 1000
export const LEFTOVER_DELETED_AGO = 12 * 24 * 60 * 60 * 1000

/* ---- meta/ ---------------------------------------------------------------- */

/** A file directly inside `versions/<id>/`. */
export interface MetaVersionFile {
    name: string
    size: number
    modified: number
}

export interface MetaVersionDir {
    id: string
    files: MetaVersionFile[]
}

/** `natives/<id>/`: platform libraries extracted for one launch target. */
export interface MetaNativesDir {
    id: string
    files: number
    bytes: number
    modified: number
}

/**
 * Shared files under `libraries/`, `assets/` or `log_configs/`, grouped by the
 * launch targets whose version information lists them. A group stands for many
 * files with the same references.
 */
export interface MetaSharedFiles {
    key: string
    /** Top folder below `meta/`, and the folder below it the files live in. */
    category: 'libraries' | 'assets' | 'log_configs'
    folder: string
    files: number
    bytes: number
    modified: number
    /** Game versions (any launch target of them) or exact launch targets that list the files. */
    games: string[]
    manifests: string[]
}

/** `java_versions/<install name>/`. */
export interface MetaRuntime {
    name: string
    files: number
    bytes: number
    modified: number
}

export interface MetaStore {
    versions: MetaVersionDir[]
    natives: MetaNativesDir[]
    shared: MetaSharedFiles[]
}

/** A runtime the launcher installed into `meta/java_versions` (Java page: `ManagedRuntime`). */
interface ManagedRuntimeLike {
    folder: string
    modified: string
    layout?: 'jdk' | 'mojang'
}

/**
 * Temurin 25, downloaded earlier through Settings → Download Java: the Java
 * page's starting runtimes (categories/java/mac.ts `INITIAL_MANAGED_RUNTIMES`).
 */
const DEFAULT_MANAGED_RUNTIMES: ManagedRuntimeLike[] = [
    { folder: 'eclipse-jdk-25', modified: '2026-06-18T19:12:00Z', layout: 'jdk' },
]

/** The runtimes in `meta/java_versions`, as the Java page installed or removed them. */
export function managedRuntimes(preview: PreviewState): MetaRuntime[] {
    const java = preview.domains.get('settings:java')?.value as
        { data?: { managed?: ManagedRuntimeLike[] } } | undefined
    const list = Array.isArray(java?.data?.managed) ? java.data.managed : DEFAULT_MANAGED_RUNTIMES
    return list.map((entry) => {
        const random = randomFor(`runtime:${entry.folder}`)
        // A full JDK from a vendor feed, or one of Mojang's JRE bundles.
        const jdk = entry.layout !== 'mojang'
        return {
            name: entry.folder,
            files: jdk ? random.int(520, 580) : random.int(330, 390),
            bytes: Math.round((jdk ? random.between(298, 326) : random.between(128, 168)) * MIB),
            modified: Date.parse(entry.modified) || 0,
        }
    })
}

/** Client jar size of a game version (an example value per version). */
function clientJarSize(game: string) {
    const known: Record<string, number> = {
        '1.20.1': 23_027_712,
        '1.21.1': 26_840_064,
        '26.1.2': 29_436_928,
        '26.2': 30_113_792,
        '26.3': 30_504_960,
    }
    return known[game] ?? Math.round(randomFor(`jar:${game}`).between(17, 31) * MIB)
}

function addVersion(store: MetaStore, manifest: LaunchManifest, installed: number) {
    const random = randomFor(`version:${manifest.id}`)
    if (!store.versions.some((entry) => entry.id === manifest.game)) {
        store.versions.push({
            id: manifest.game,
            files: [
                {
                    name: `${manifest.game}.jar`,
                    size: clientJarSize(manifest.game),
                    modified: installed,
                },
                {
                    name: `${manifest.game}.json`,
                    size: Math.round(random.between(26, 34) * KIB),
                    modified: installed,
                },
            ],
        })
    }
    if (
        manifest.id !== manifest.game &&
        !store.versions.some((entry) => entry.id === manifest.id)
    ) {
        const forge = manifest.loader === 'forge' || manifest.loader === 'neoforge'
        store.versions.push({
            id: manifest.id,
            files: [
                {
                    name: `${manifest.id}.json`,
                    size: Math.round(
                        (forge ? random.between(38, 46) : random.between(9, 14)) * KIB,
                    ),
                    modified: installed,
                },
            ],
        })
    }
    if (!store.natives.some((entry) => entry.id === manifest.id)) {
        store.natives.push({
            id: manifest.id,
            files: 9,
            bytes: Math.round(random.between(2.3, 3.1) * MIB),
            modified: installed,
        })
    }
    const shared = (entry: Omit<MetaSharedFiles, 'modified'>) => {
        if (!store.shared.some((existing) => existing.key === entry.key)) {
            store.shared.push({ ...entry, modified: installed })
        }
    }
    const gameRandom = randomFor(`game:${manifest.game}`)
    // Libraries and assets of the game version itself.
    shared({
        key: `libraries:${manifest.game}`,
        category: 'libraries',
        folder: 'com',
        files: gameRandom.int(9, 14),
        bytes: Math.round(gameRandom.between(3.2, 5.4) * MIB),
        games: [manifest.game],
        manifests: [],
    })
    shared({
        key: `assets:index:${manifest.game}`,
        category: 'assets',
        folder: 'indexes',
        files: 1,
        bytes: Math.round(gameRandom.between(398, 452) * KIB),
        games: [manifest.game],
        manifests: [],
    })
    shared({
        key: `assets:objects:${manifest.game}`,
        category: 'assets',
        folder: 'objects',
        files: gameRandom.int(70, 140),
        bytes: Math.round(gameRandom.between(6.1, 12.8) * MIB),
        games: [manifest.game],
        manifests: [],
    })
    if (manifest.id === manifest.game) return
    const loaderRandom = randomFor(`loader:${manifest.loader}:${manifest.loaderVersion}`)
    if (manifest.loader === 'forge' || manifest.loader === 'neoforge') {
        // The loader's libraries and the client jars its processors patch.
        shared({
            key: `libraries:${manifest.id}`,
            category: 'libraries',
            folder: 'net',
            files: 164,
            bytes: Math.round(loaderRandom.between(108, 116) * MIB),
            games: [],
            manifests: [manifest.id],
        })
    } else {
        shared({
            key: `libraries:${manifest.loader}:${manifest.loaderVersion}`,
            category: 'libraries',
            folder: 'net',
            files: 12,
            bytes: Math.round(loaderRandom.between(4.3, 4.8) * MIB),
            games: [],
            manifests: [],
        })
        const loaderEntry = store.shared.find(
            (entry) => entry.key === `libraries:${manifest.loader}:${manifest.loaderVersion}`,
        )
        if (loaderEntry && !loaderEntry.manifests.includes(manifest.id)) {
            loaderEntry.manifests.push(manifest.id)
        }
    }
}

/** `meta/` as the preview user's installs left it. */
export function createMetaStore(instances: GameInstance[], now: number): MetaStore {
    const store: MetaStore = { versions: [], natives: [], shared: [] }
    const oldest = Math.min(
        now - 120 * DAY,
        ...instances.map((instance) => Date.parse(instance.created)),
    )
    // Files every modern game version shares (most libraries and assets).
    const modern = ['1.20.1', '1.21.1', '26.1.2', '26.2', '26.3']
    store.shared.push(
        {
            key: 'libraries:common',
            category: 'libraries',
            folder: 'org',
            files: 61,
            bytes: 34_712_576,
            modified: oldest,
            games: modern,
            manifests: [],
        },
        {
            key: 'libraries:1.21+',
            category: 'libraries',
            folder: 'io',
            files: 18,
            bytes: 9_437_184,
            modified: oldest,
            games: modern.slice(1),
            manifests: [],
        },
        {
            key: 'libraries:26.x',
            category: 'libraries',
            folder: 'com',
            files: 22,
            bytes: 14_155_776,
            modified: oldest,
            games: modern.slice(2),
            manifests: [],
        },
        {
            key: 'assets:objects:common',
            category: 'assets',
            folder: 'objects',
            files: 3_412,
            bytes: 401_604_608,
            modified: oldest,
            games: modern,
            manifests: [],
        },
        {
            key: 'assets:objects:1.21+',
            category: 'assets',
            folder: 'objects',
            files: 512,
            bytes: 67_108_864,
            modified: oldest,
            games: modern.slice(1),
            manifests: [],
        },
        {
            key: 'assets:objects:26.x',
            category: 'assets',
            folder: 'objects',
            files: 604,
            bytes: 74_448_896,
            modified: oldest,
            games: modern.slice(2),
            manifests: [],
        },
        {
            key: 'log_configs:client-legacy',
            category: 'log_configs',
            folder: '',
            files: 1,
            bytes: 888,
            modified: oldest,
            games: ['1.20.1', '1.21.1'],
            manifests: [],
        },
        {
            key: 'log_configs:client-26',
            category: 'log_configs',
            folder: '',
            files: 1,
            bytes: 1_014,
            modified: oldest,
            games: ['26.1.2', '26.2', '26.3'],
            manifests: [],
        },
    )
    const leftoverInstalled = now - LEFTOVER_CREATED_AGO
    for (const manifest of LEFTOVER_MANIFESTS) addVersion(store, manifest, leftoverInstalled)
    ensureMetaFor(store, instances)
    return store
}

/** Installs whatever an instance needs that `meta/` does not hold yet. */
export function ensureMetaFor(store: MetaStore, instances: GameInstance[]) {
    for (const instance of instances) {
        addVersion(store, manifestFor(instance), Date.parse(instance.created))
    }
}

/** `meta/` as folders, for the scan and Finder. */
export function metaDir(store: MetaStore, runtimeList: MetaRuntime[], modified: number): DiskDir {
    const versions = dir(
        'versions',
        modified,
        store.versions.map((version) =>
            dir(
                version.id,
                Math.max(...version.files.map((entry) => entry.modified)),
                version.files.map((entry) => file(entry.name, entry.size, entry.modified)),
            ),
        ),
    )
    const natives = dir(
        'natives',
        modified,
        store.natives.map((entry) => packed(entry.id, entry.modified, entry.files, entry.bytes)),
    )
    const folderOf = (category: MetaSharedFiles['category']) => {
        const groups = new Map<string, { files: number; bytes: number; modified: number }>()
        for (const entry of store.shared.filter((shared) => shared.category === category)) {
            const current = groups.get(entry.folder) ?? { files: 0, bytes: 0, modified: 0 }
            current.files += entry.files
            current.bytes += entry.bytes
            current.modified = Math.max(current.modified, entry.modified)
            groups.set(entry.folder, current)
        }
        return groups
    }
    const libraries = dir(
        'libraries',
        modified,
        [...folderOf('libraries')].map(([name, entry]) =>
            packed(name, entry.modified, entry.files, entry.bytes),
        ),
    )
    const assets = dir(
        'assets',
        modified,
        [...folderOf('assets')].map(([name, entry]) =>
            packed(name, entry.modified, entry.files, entry.bytes),
        ),
    )
    const logConfigs = folderOf('log_configs').get('')
    const runtimes = dir(
        'java_versions',
        modified,
        runtimeList.map((runtime) =>
            packed(runtime.name, runtime.modified, runtime.files, runtime.bytes),
        ),
    )
    const children: DiskEntry[] = [versions, libraries, assets, natives, runtimes]
    if (logConfigs) {
        children.push(
            packed('log_configs', logConfigs.modified, logConfigs.files, logConfigs.bytes),
        )
    }
    return dir(
        'meta',
        modified,
        children.filter((entry) => entry.kind !== 'dir' || measure(entry).files > 0),
    )
}

/* ---- caches/ -------------------------------------------------------------- */

export interface CacheFile {
    /** Path below `caches/`. */
    path: string
    size: number
    modified: number
}

/** `caches/content-store/<sha512>`: a verified object an install published. */
export interface StoreObject {
    digest: string
    size: number
    modified: number
    /** The installed file the object was published for (`null`: an instance that no longer exists). */
    source: { instanceId: string; fileName: string } | null
}

export interface CacheStore {
    files: CacheFile[]
    objects: StoreObject[]
    /** `resources/<channel>/resource-cache.db` and its write-ahead log. */
    resourceDbs: { channel: 'release' | 'beta'; bytes: number; wal: number; modified: number }[]
    /** Instance content already published to the store (`instanceId/fileName`). */
    published: string[]
    /** Pack files are listed once the snapshot data has been read. */
    packsListed: boolean
}

export function createCacheStore(instances: GameInstance[], now: number): CacheStore {
    const files: CacheFile[] = []
    // Instance icons the pack installs cached (`write_cached_icon`).
    for (const instance of instances) {
        if (instance.link?.type !== 'modrinth_modpack') continue
        const random = randomFor(`icon:${instance.link.project_id}`)
        files.push({
            path: `icons/${hexName(`icon:${instance.link.project_id}`, 40)}.webp`,
            size: Math.round(random.between(9, 46) * KIB),
            modified: Date.parse(instance.created),
        })
    }
    // Native archives kept for offline launches (`minecraft-natives/<sha1>.jar`).
    for (let index = 0; index < 7; index += 1) {
        const random = randomFor(`natives-archive:${index}`)
        files.push({
            path: `minecraft-natives/${hexName(`natives-archive:${index}`, 40)}.jar`,
            size: Math.round(random.between(0.18, 1.6) * MIB),
            modified: now - random.int(20, 120) * DAY,
        })
    }
    // Mojang Java runtime manifests (`java/manifests/<sha1>.json`).
    for (const component of ['java-runtime-gamma', 'java-runtime-delta', 'java-runtime-epsilon']) {
        const random = randomFor(`runtime-manifest:${component}`)
        files.push({
            path: `java/manifests/${hexName(`runtime-manifest:${component}`, 40)}.json`,
            size: Math.round(random.between(108, 141) * KIB),
            modified: now - random.int(30, 110) * DAY,
        })
    }
    // Mods the deleted Fabric 26.1.2 instance installed, still in the content store.
    const objects: StoreObject[] = []
    for (let index = 0; index < 6; index += 1) {
        const random = randomFor(`deleted-instance-object:${index}`)
        objects.push({
            digest: hexName(`deleted-instance-object:${index}`, 128),
            size: Math.round(random.between(0.08, 4.6) * MIB),
            modified: now - LEFTOVER_CREATED_AGO + index * HOUR,
            source: null,
        })
    }
    return {
        files,
        objects,
        resourceDbs: [{ channel: 'release', bytes: 1_867_776, wal: 0, modified: now - 4 * MINUTE }],
        published: [],
        packsListed: false,
    }
}

/* ---- The live parts -------------------------------------------------------- */

export interface DiskWorld {
    instanceId: string
    name: string
    folder: string
    lastPlayed: number
}

export interface DiskContext {
    preview: PreviewState
    now: number
    /** Leave out what happened after this time (an older snapshot). */
    asOf: number
    meta: MetaStore
    cache: CacheStore
    /** `caches/launcher-storage.cbor`, when a snapshot is cached. */
    snapshotBytes: number | null
    snapshotModified: number
    repositoryPath: string
    /** Folders the visitor created in the open panel. */
    userFolders: string[]
    sessionStart: number
}

/** Seed worlds of Home (screens/home/state.ts), for when Home has not been opened yet. */
const SEED_WORLDS: [instanceId: string, name: string, lastPlayedAgo: number][] = [
    ['fabulously-optimized', 'New World', 5 * MINUTE],
    ['fabulously-optimized', '生存挑战', 3 * DAY],
    ['fabulously-optimized', 'Hardcore Run', 11 * DAY],
    ['cobblemon', 'PokéWorld', 26 * HOUR],
    ['cobblemon', 'Team Rocket', 6 * DAY],
    ['fabric-26-3', 'Redstone Lab', 2 * DAY],
    ['fabric-26-3', 'New World', 6 * DAY],
    ['better-mc', 'Better World', 3 * DAY],
    ['zombie-invade', '100 Days', 9 * DAY],
    ['vanilla', 'World', 16 * DAY],
]

/** Servers the instances' `servers.dat` lists, from the same seed. */
const SEED_SERVER_ENTRIES: Record<string, number> = {
    'fabulously-optimized': 2,
    cobblemon: 1,
}

interface HomeWorldLike {
    type?: string
    instance_id?: string
    name?: string
    path?: string
    last_played?: string
}

/** The singleplayer worlds Home lists (the app's single worlds backend), or the seed. */
function worldsOf(ctx: DiskContext): { worlds: DiskWorld[]; servers: Record<string, number> } {
    const home = ctx.preview.domains.get('home')?.value as
        { worlds?: { value?: unknown } } | undefined
    const list = home?.worlds?.value
    if (Array.isArray(list)) {
        const worlds: DiskWorld[] = []
        const servers: Record<string, number> = {}
        for (const raw of list as HomeWorldLike[]) {
            if (!raw?.instance_id) continue
            if (raw.type === 'server') {
                servers[raw.instance_id] = (servers[raw.instance_id] ?? 0) + 1
                continue
            }
            const folder =
                (raw.path ?? raw.name ?? 'World').split(/[\\/]/).filter(Boolean).pop() ?? 'World'
            worlds.push({
                instanceId: raw.instance_id,
                name: raw.name ?? folder,
                folder,
                lastPlayed: Date.parse(raw.last_played ?? '') || ctx.now - DAY,
            })
        }
        return { worlds, servers }
    }
    return {
        worlds: SEED_WORLDS.map(([instanceId, name, ago]) => ({
            instanceId,
            name,
            folder: name,
            lastPlayed: ctx.sessionStart - ago,
        })),
        servers: SEED_SERVER_ENTRIES,
    }
}

interface InstanceContentMeta {
    folder?: string
    rollback?: { fileName?: string; size?: number } | null
}

/** Folder and rollback file the instance page keeps for an item, when it was opened. */
function contentMetaOf(
    ctx: DiskContext,
    instanceId: string,
    itemId: string,
): InstanceContentMeta | null {
    const domain = ctx.preview.domains.get('instance-content')?.value as
        { meta?: Record<string, InstanceContentMeta> } | undefined
    return domain?.meta?.[`${instanceId}:${itemId}`] ?? null
}

function folderForType(type: string) {
    switch (type) {
        case 'resourcepack':
            return 'resourcepacks'
        case 'shader':
            return 'shaderpacks'
        case 'datapack':
            return 'datapacks'
        default:
            return 'mods'
    }
}

/** Where an installed item lives inside its instance. */
function itemFolder(ctx: DiskContext, instanceId: string, item: ContentItem) {
    const known = contentMetaOf(ctx, instanceId, item.id)?.folder
    if (known) return known
    const slash = item.id.lastIndexOf('/')
    if (slash > 0 && item.id.slice(slash + 1) === item.file_name) return item.id.slice(0, slash)
    return folderForType(item.project_type)
}

/** Whether an install published the item to the content store (individual installs do; pack files do not). */
export function publishedThroughStore(instance: GameInstance, item: ContentItem) {
    if (!item.project_id || item.project_id.startsWith('local:')) return false
    const packFile = instance.link?.type === 'modrinth_modpack' && item.id.includes('/')
    return !packFile
}

/** A mod id the way config files are named after it. */
function modIdOf(fileName: string) {
    const stem = fileName.replace(/\.(jar|zip)(\.disabled)?$/i, '')
    const match = stem.match(/^[A-Za-z][A-Za-z_]*(?:[-_][A-Za-z][A-Za-z_]*)?/)
    return (match?.[0] ?? stem).replace(/-(fabric|forge|neoforge|quilt)$/i, '').toLowerCase()
}

function instanceDir(
    ctx: DiskContext,
    instance: GameInstance,
    items: ContentItem[],
    worlds: DiskWorld[],
    serverEntries: number,
): DiskDir {
    const random = randomFor(`instance:${instance.id}`)
    const created = Date.parse(instance.created)
    const lastPlayed = Math.min(ctx.asOf, Date.parse(instance.last_played ?? instance.created))
    const playtime = Math.max(0, instance.submitted_time_played)
    const playedHours = playtime / HOUR
    const modded = instance.loader !== 'vanilla'
    const children: DiskEntry[] = []
    const folders = new Map<string, DiskEntry[]>()
    const put = (folder: string, entry: DiskEntry) => {
        const list = folders.get(folder) ?? []
        list.push(entry)
        folders.set(folder, list)
    }

    // Installed content, with `.disabled` for turned-off files and the
    // `<new>_<old>.old` files updates keep for a rollback.
    let modBytes = 0
    const modIds = new Set<string>()
    for (const item of items) {
        // A pack file the visitor removed is listed by the launcher but gone from the folder.
        if (item.removed) continue
        const installedAt = Date.parse(item.installed_at) || created
        if (installedAt > ctx.asOf) continue
        const folder = itemFolder(ctx, instance.id, item)
        const fileName = item.enabled ? item.file_name : `${item.file_name}.disabled`
        put(folder, file(fileName, item.size, installedAt))
        if (folder === 'mods') {
            modBytes += item.size
            modIds.add(modIdOf(item.file_name))
        }
        const rollback = contentMetaOf(ctx, instance.id, item.id)?.rollback
        if (rollback?.fileName) {
            put(
                folder,
                file(
                    `${item.file_name}_${rollback.fileName}.old`,
                    rollback.size ?? item.size,
                    installedAt,
                ),
            )
        }
    }
    if (modded && !folders.has('mods')) folders.set('mods', [])

    for (const [folder, entries] of folders) {
        // Datapacks of a world are placed with the world below.
        if (folder.startsWith('saves/')) continue
        const modified = Math.max(created, ...entries.map((entry) => entry.modified))
        children.push(dir(folder, modified, entries))
    }

    // Worlds: region files grow with the time spent in them.
    const own = worlds
        .filter((world) => world.instanceId === instance.id)
        .sort((a, b) => b.lastPlayed - a.lastPlayed)
    if (own.length) {
        const budget = (12 + playedHours * (modded ? 34 : 11)) * MIB
        const weights = own.map((_, index) =>
            index === 0 ? 0.56 : index === 1 ? 0.29 : 0.15 / (own.length - 2),
        )
        const totalWeight = weights.reduce((sum, weight) => sum + weight, 0)
        const saves = own.map((world, index) => {
            const worldRandom = randomFor(`world:${instance.id}:${world.folder}`)
            const bytes =
                (budget * weights[index]) / totalWeight + worldRandom.between(1.5, 4) * MIB
            const stale = world.lastPlayed > ctx.asOf ? 0.86 : 1
            const entries: DiskEntry[] = [
                packed(
                    'region',
                    world.lastPlayed,
                    Math.max(4, bytes / (1.9 * MIB)),
                    bytes * 0.86 * stale,
                ),
                packed(
                    'entities',
                    world.lastPlayed,
                    Math.max(2, bytes / (4.8 * MIB)),
                    bytes * 0.07 * stale,
                ),
                packed(
                    'poi',
                    world.lastPlayed,
                    Math.max(2, bytes / (7 * MIB)),
                    bytes * 0.03 * stale,
                ),
                packed('playerdata', world.lastPlayed, 2, 18 * KIB),
                packed('data', world.lastPlayed, worldRandom.int(6, 22), bytes * 0.025),
                file('level.dat', worldRandom.between(1.6, 3.4) * KIB, world.lastPlayed),
                file(
                    'level.dat_old',
                    worldRandom.between(1.6, 3.4) * KIB,
                    world.lastPlayed - 5 * MINUTE,
                ),
                file('session.lock', 3, world.lastPlayed),
                file('icon.png', worldRandom.between(9, 21) * KIB, world.lastPlayed),
            ]
            const datapacks = folders.get(`saves/${world.folder}/datapacks`)
            if (datapacks?.length) entries.push(dir('datapacks', world.lastPlayed, datapacks))
            return dir(world.folder, world.lastPlayed, entries)
        })
        children.push(dir('saves', Math.max(...own.map((world) => world.lastPlayed)), saves))
    }

    // Screenshots of the screenshot page.
    const shots = ctx.preview.screenshots.screenshots.value.filter(
        (shot) => shot.instance_id === instance.id && Date.parse(shot.taken_at) <= ctx.asOf,
    )
    if (shots.length) {
        children.push(
            dir(
                'screenshots',
                Math.max(...shots.map((shot) => Date.parse(shot.taken_at))),
                shots.map((shot) => {
                    const takenAt = Date.parse(shot.taken_at)
                    return file(
                        screenshotName(takenAt),
                        shot.width * shot.height * 1.93 + 1_840,
                        takenAt,
                    )
                }),
            ),
        )
    }

    // Logs: today's latest.log and one archive per earlier play day.
    if (playtime > 0) {
        const sessions = Math.min(12, Math.max(1, Math.ceil(playedHours / 1.3)))
        const logs: DiskEntry[] = [
            file(
                'latest.log',
                (modded ? random.between(380, 2_600) : random.between(28, 90)) * KIB,
                lastPlayed,
            ),
        ]
        const span = Math.max(DAY, lastPlayed - created)
        const perDay = new Map<string, number>()
        for (let index = 1; index < sessions; index += 1) {
            const time = lastPlayed - (span * index) / sessions
            const day = localDate(time)
            const number = (perDay.get(day) ?? 0) + 1
            perDay.set(day, number)
            logs.push(
                file(
                    `${day}-${number}.log.gz`,
                    (modded ? random.between(42, 260) : random.between(4, 12)) * KIB,
                    time,
                ),
            )
        }
        children.push(dir('logs', lastPlayed, logs))
    }

    if (modded) {
        // One config file for most mods, named after the mod.
        const forge = instance.loader === 'forge' || instance.loader === 'neoforge'
        const configs: DiskEntry[] = []
        for (const id of modIds) {
            const configRandom = randomFor(`config:${instance.id}:${id}`)
            if (configRandom.next() > 0.72) continue
            const name = forge ? `${id}-client.toml` : `${id}.json`
            configs.push(
                file(
                    name,
                    configRandom.between(0.3, 14) * KIB,
                    created + configRandom.between(0, 1) * DAY,
                ),
            )
        }
        if (configs.length) children.push(dir('config', created, configs))
        if (forge) {
            // Server-side defaults the pack ships for new worlds.
            const defaults = [...modIds]
                .filter((id) => randomFor(`defaultconfig:${instance.id}:${id}`).next() < 0.12)
                .map((id) =>
                    file(
                        `${id}-server.toml`,
                        randomFor(`defaultconfig-size:${id}`).between(0.6, 9) * KIB,
                        created,
                    ),
                )
            if (defaults.length) children.push(dir('defaultconfigs', created, defaults))
        } else {
            children.push(
                dir('.fabric', lastPlayed, [
                    packed(
                        'processedMods',
                        lastPlayed,
                        Math.round(modIds.size * 0.4),
                        modBytes * 0.05,
                    ),
                    packed(
                        'remappedJars',
                        lastPlayed,
                        Math.round(modIds.size * 0.2),
                        modBytes * 0.03,
                    ),
                ]),
            )
        }
        // Xaero's Minimap and World Map keep their data in `xaero/`.
        if ([...modIds].some((id) => id.startsWith('xaero')) && playtime > 0) {
            children.push(
                dir('xaero', lastPlayed, [
                    packed(
                        'minimap',
                        lastPlayed,
                        6 + Math.round(playedHours * 2),
                        (0.2 + playedHours * 0.5) * MIB,
                    ),
                    packed(
                        'world-map',
                        lastPlayed,
                        12 + Math.round(playedHours * 6),
                        (0.6 + playedHours * 1.9) * MIB,
                    ),
                ]),
            )
        }
    }
    if (instance.id === 'zombie-invade' && created <= ctx.asOf) {
        // The Forge pack crashed once on its first launch.
        children.push(
            dir('crash-reports', created + 2 * HOUR, [
                file(
                    `crash-${localDate(created)}_21.14.08-client.txt`,
                    61_440 + 5_337,
                    created + 2 * HOUR,
                ),
            ]),
        )
    }

    if (playtime > 0) children.push(file('options.txt', random.between(5.9, 7.1) * KIB, lastPlayed))
    if (serverEntries > 0) children.push(file('servers.dat', 180 + serverEntries * 94, lastPlayed))
    if (playtime > 0)
        children.push(file('usercache.json', random.between(0.15, 0.4) * KIB, lastPlayed))

    return dir(baseName(instance.path), Math.max(created, lastPlayed), children)
}

/** The recorded folders of the server software the managed servers run (`data/servers`). */
const SERVER_TREES: Record<
    NonNullable<ManagedServer['recording']>,
    { path: string; size: number }[]
> = {
    'vanilla-26.3': vanillaServerTree,
    'fabric-26.3': fabricServerTree,
}

/** A managed server's folder: its software's recorded folder and the launcher's manifest. */
function serverDir(ctx: DiskContext, server: ManagedServer): DiskDir {
    const created = Math.min(ctx.asOf, Date.parse(server.manifest.createdAt) || ctx.now)
    const started = Math.min(ctx.asOf, Date.parse(server.manifest.lastStartedAt ?? '') || created)
    const root = dir(server.manifest.id, started)
    for (const entry of server.recording ? SERVER_TREES[server.recording] : []) {
        const path = entry.path.replace(/\/$/, '')
        const modified = path.startsWith('world') || path.startsWith('logs') ? started : created
        if (entry.path.endsWith('/')) {
            if (!findEntry(root, path))
                placeEntry(root, path, dir(baseName(path), modified), modified)
        } else {
            placeEntry(root, path, file(baseName(path), entry.size, modified), modified)
        }
    }
    placeEntry(
        root,
        'axolotl-server.json',
        file('axolotl-server.json', JSON.stringify(server.manifest, null, 2).length, created),
        created,
    )
    return root
}

function cachesDir(ctx: DiskContext): DiskDir {
    const root = dir('caches', ctx.now - 4 * MINUTE)
    for (const entry of ctx.cache.files) {
        if (entry.modified > ctx.asOf) continue
        placeEntry(
            root,
            entry.path,
            file(baseName(entry.path), entry.size, entry.modified),
            entry.modified,
        )
    }
    const objects = ctx.cache.objects.filter((object) => object.modified <= ctx.asOf)
    if (objects.length) {
        placeEntry(
            root,
            'content-store',
            dir(
                'content-store',
                Math.max(...objects.map((object) => object.modified)),
                objects.map((object) => file(object.digest, object.size, object.modified)),
            ),
            ctx.now,
        )
    }
    const resources = ctx.cache.resourceDbs.map((database) =>
        dir(database.channel, database.modified, [
            file('resource-cache.db', database.bytes, database.modified),
            ...(database.wal > 0
                ? [file('resource-cache.db-wal', database.wal, database.modified)]
                : []),
            ...(database.wal > 0 ? [file('resource-cache.db-shm', 32_768, database.modified)] : []),
        ]),
    )
    placeEntry(root, 'resources', dir('resources', ctx.now - 4 * MINUTE, resources), ctx.now)
    if (ctx.snapshotBytes !== null) {
        placeEntry(
            root,
            'launcher-storage.cbor',
            file('launcher-storage.cbor', ctx.snapshotBytes, ctx.snapshotModified),
            ctx.now,
        )
    }
    return root
}

/**
 * The launcher folder (`~/Library/Application Support/red.ghs.axolotl`):
 * settings and config directory at once, as `DirectoryInfo` resolves them
 * without a custom data directory.
 */
function launcherDir(ctx: DiskContext): DiskDir {
    const { preview } = ctx
    const instances = preview.instances.instances.value.filter(
        (instance) => Date.parse(instance.created) <= ctx.asOf,
    )
    const contents = preview.downloads.contents.value
    const { worlds, servers } = worldsOf(ctx)
    const owned = instances.filter((instance) => instance.path.startsWith(`${ROOT}/profiles/`))
    const profiles = dir(
        'profiles',
        Math.max(ctx.now - DAY, ...owned.map((instance) => Date.parse(instance.created))),
        owned.map((instance) => {
            if (instance.symlink_target) {
                return {
                    kind: 'link',
                    name: baseName(instance.path),
                    modified: Date.parse(instance.created),
                    target: instance.symlink_target,
                } satisfies DiskLink
            }
            return instanceDir(
                ctx,
                instance,
                contents[instance.id] ?? [],
                worlds,
                servers[instance.id] ?? 0,
            )
        }),
    )
    const sessionStart = ctx.sessionStart
    const logStart = Math.min(sessionStart, ctx.asOf)
    const launcherLogs = dir('launcher_logs', ctx.now, [
        file(sessionLogName(logStart), 186 * KIB + Math.max(0, ctx.now - logStart) / 900, ctx.now),
        file(sessionLogName(logStart - 26 * HOUR - 17 * MINUTE), 1_412_301, logStart - 22 * HOUR),
        file(sessionLogName(logStart - 50 * HOUR - 41 * MINUTE), 2_217_884, logStart - 47 * HOUR),
    ])
    const local = preview.servers.managed.filter(
        (server) => (Date.parse(server.manifest.createdAt) || 0) <= ctx.asOf,
    )
    const at = (iso: string) => Math.min(ctx.now, Date.parse(iso))
    // The Release database and its backups, as the data-directory settings show
    // them (categories/resources/mac-fs.ts): the newest three snapshots of the
    // updates to 1.9.9 and 1.10.0.
    const releaseUpdate = at('2026-10-23T14:20:58Z')
    const children: DiskEntry[] = [
        profiles,
        cachesDir(ctx),
        metaDir(ctx.meta, managedRuntimes(preview), ctx.now - 9 * DAY),
        launcherLogs,
        dir('release', ctx.now, [
            file('app.db', 3_420_160, ctx.now),
            file('app.db-shm', 32_768, ctx.now),
            file('app.db-wal', 1_134_592, ctx.now),
        ]),
        dir('Backups', releaseUpdate, [
            dir('app-db', releaseUpdate, [
                dir('release', releaseUpdate, [
                    file(
                        'app-db-before-1.9.9-from-1.9.8.db',
                        3_104_768,
                        at('2026-10-20T12:31:49Z'),
                    ),
                    file(
                        'app-db-before-update-release-1.10.0-1792765213000468211.db',
                        3_268_608,
                        at('2026-10-23T14:20:13Z'),
                    ),
                    file('app-db-before-1.10.0-from-1.9.9.db', 3_272_704, releaseUpdate),
                ]),
            ]),
            // The default backup repository: no instance has backups enabled yet.
            dir('instances', at('2026-10-02T18:31:00Z')),
        ]),
        file('app-window-state.json', 214, ctx.now),
        file(
            'instance_groups.json',
            24 + JSON.stringify(preview.instances.groups.value).length + instances.length * 46,
            ctx.now - DAY,
        ),
        file('native-download-reputation.json', 1_934, ctx.now - 9 * DAY),
        file('download.log', 421_903 + local.length * 8_112, ctx.now - 9 * DAY),
        file('.metadata-use.lock', 0, ctx.now),
        file('.DS_Store', 6_148, ctx.now - 12 * DAY),
    ]
    if (local.length) {
        children.push(
            dir(
                'servers',
                Math.max(...local.map((server) => Date.parse(server.manifest.createdAt) || 0)),
                local.map((server) => serverDir(ctx, server)),
            ),
        )
    }
    return dir(baseName(ROOT), ctx.now, children)
}

/** Instances whose game directory lies outside the launcher folder (linked game directories). */
export function externalInstanceDirs(ctx: DiskContext) {
    const contents = ctx.preview.downloads.contents.value
    const { worlds, servers } = worldsOf(ctx)
    return ctx.preview.instances.instances.value
        .filter(
            (instance) =>
                Date.parse(instance.created) <= ctx.asOf && !instance.path.startsWith(`${ROOT}/`),
        )
        .map((instance) => ({
            instance,
            entry: instanceDir(
                ctx,
                instance,
                contents[instance.id] ?? [],
                worlds,
                servers[instance.id] ?? 0,
            ),
        }))
}

/** The whole preview disk: the launcher folder, the user's folders and the external drive. */
export function buildDisk(ctx: DiskContext): DiskDir {
    const root = dir('', ctx.now)
    const t = ctx.now
    const home = dir('steve', t - DAY, [
        dir('Applications', t - 90 * DAY),
        dir('Desktop', t - 2 * DAY, [
            file('截屏2026-10-21 21.14.08.png', 2_481_239, t - 2 * DAY),
            file('Redstone notes.txt', 3_218, t - 6 * DAY),
        ]),
        dir('Documents', t - 5 * DAY, [
            dir('Minecraft', t - 40 * DAY, [file('servers.txt', 412, t - 40 * DAY)]),
            file('Seeds.txt', 1_206, t - 18 * DAY),
        ]),
        dir('Downloads', t - DAY, [
            file(
                'Axolotl.Launcher_1.9.7_universal.dmg',
                137_897_714,
                Math.min(t, Date.parse('2026-10-02T18:24:00Z')),
            ),
            file('Fabulously.Optimized-v14.1.1.mrpack', 192_991, t - 12 * DAY),
        ]),
        dir('Movies', t - 60 * DAY),
        dir('Music', t - 90 * DAY),
        dir('Pictures', t - 20 * DAY),
        dir('Public', t - 90 * DAY),
        dir('Library', t - DAY, [
            dir('Application Support', t - DAY, [
                launcherDir(ctx),
                dir('minecraft', t - 70 * DAY, [
                    packed('versions', t - 70 * DAY, 6, 47 * MIB),
                    packed('saves', t - 70 * DAY, 412, 38 * MIB),
                ]),
            ]),
        ]),
    ])
    placeEntry(root, `${HOME}`, home, t)
    placeEntry(
        root,
        `${VOLUMES}/${EXTERNAL_VOLUME}`,
        dir(EXTERNAL_VOLUME, t - 14 * DAY, [
            dir('Photos 2025', t - 200 * DAY, [packed('Exports', t - 200 * DAY, 318, 2_418 * MIB)]),
            dir('Archive', t - 120 * DAY, [file('old-worlds.zip', 734_003_200, t - 120 * DAY)]),
        ]),
        t,
    )
    for (const folder of ctx.userFolders) {
        if (!findEntry(root, folder)) placeEntry(root, folder, dir(baseName(folder), t), t)
    }
    for (const { instance, entry } of externalInstanceDirs(ctx)) {
        if (!findEntry(root, instance.path)) placeEntry(root, instance.path, entry, t)
    }
    return root
}

/* ---- The avatar and skin preview cache (WebView IndexedDB) --------------- */

/** Head renders (`head-storage`) and skin previews (`skin-previews-v3`) the WebView caches. */
export function previewCacheBytes(preview: PreviewState) {
    let bytes = 0
    for (const account of preview.accounts.accounts.value) {
        bytes += Math.round(randomFor(`head:${account.profile.id}`).between(1.1, 2.3) * KIB)
    }
    for (const skin of preview.accounts.skins.value) {
        bytes += Math.round(randomFor(`skin-preview:${skin.texture_key}`).between(31, 54) * KIB)
    }
    return bytes
}
