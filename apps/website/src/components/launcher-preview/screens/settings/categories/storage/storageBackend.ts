import { loadProject } from '../../../../data'
import { previewDomain, type PreviewState, usePreview } from '../../../../lib/state/index'
import type { GameInstance } from '../../../../lib/types'
import {
    buildDisk,
    type CacheStore,
    createCacheStore,
    createMetaStore,
    type DiskContext,
    type DiskDir,
    type DiskEntry,
    ensureMetaFor,
    findEntry,
    hexName,
    type LaunchManifest,
    manifestFor,
    measure,
    type MetaStore,
    previewCacheBytes,
    publishedThroughStore,
    ROOT,
} from './launcherDisk'
import type {
    BackupOperation,
    BackupProgressEvent,
    BackupRepositoryStatus,
    CacheCleanupResult,
    MetaAnalysis,
    MetaCleanupResult,
    MetaGroup,
    MetaWarning,
    StorageNode,
    StorageNodeType,
    StorageOpenResult,
    StoragePath,
    StorageScanEvent,
    StorageSize,
    StorageTree,
} from './storageData'

/*
 * The storage plugin (apps/app/src/api/storage.rs → packages/app-lib/src/storage)
 * and the backup repository commands of the instance plugin, run over the
 * preview disk (launcherDisk.ts). The scan, the metadata reachability rules,
 * the cleanup checks and the cache purge follow app-lib step by step; their
 * durations follow the amount of files each step reads, on the window clock.
 */

const STORAGE_CACHE_VERSION = 5
/** `snapshot_is_fresh`: a cached scan younger than five minutes is reused. */
const FRESH_SNAPSHOT_MS = 300_000
/** `PLAN_AGE`: a metadata analysis can be cleaned within fifteen minutes. */
const PLAN_AGE_MS = 15 * 60_000
const DAY = 24 * 60 * 60_000
const HOUR = 60 * 60_000
/** The previous app session left a scan from six days ago in `launcher-storage.cbor`. */
const PREVIOUS_SCAN_AGO = 6 * DAY + 3 * HOUR

const DEFAULT_REPOSITORY = `${ROOT}/Backups/instances`
const REPOSITORY_OPERATION_KEY = '__backup_repository__'

type CandidateRef =
    | { kind: 'version-file'; version: string; name: string }
    | { kind: 'natives'; id: string }
    | { kind: 'shared'; key: string }

interface Candidate {
    group: string
    ref: CandidateRef
    files: number
    size: number
}

interface StorageBackendState {
    createdAt: number
    meta: MetaStore
    cache: CacheStore
    /** `caches/launcher-storage.cbor`. */
    snapshot: { tree: StorageTree; bytes: number; roots: string; modified: number } | null
    /** Whether the previous session's snapshot has been written. */
    seeded: boolean
    /** `settings.backup_repository_path` (`null`: the default location). */
    repositoryPath: string | null
    /** Folders created in the open panel. */
    userFolders: string[]
    /** Pack files already placed in `caches/modpacks` (`instanceId:versionId`). */
    listedPacks: string[]
    plans: Map<string, { created: number; candidates: Candidate[] }>
    move: { id: string; processed: number; total: number; created: number } | null
    listeners: Set<(event: BackupProgressEvent) => void>
    /** `SCAN_LOCK`. */
    scanLock: Promise<void>
    /** The exclusive metadata lock (`usage::exclusive`), held by a cleanup. */
    exclusive: Promise<void> | null
}

function createState(preview: PreviewState): StorageBackendState {
    const now = Date.now()
    const instances = preview.instances.instances.value
    return {
        createdAt: now,
        meta: createMetaStore(instances, now),
        cache: createCacheStore(instances, now),
        snapshot: null,
        seeded: false,
        repositoryPath: null,
        userFolders: [],
        listedPacks: [],
        plans: new Map(),
        move: null,
        listeners: new Set(),
        scanLock: Promise.resolve(),
        exclusive: null,
    }
}

function sizeTotal(size: StorageSize) {
    return size.actual + size.symlink
}

function addSize(into: StorageSize, size: StorageSize) {
    into.actual += size.actual
    into.symlink += size.symlink
}

/** `format!("category-{kind:?}")`: the Rust variant name. */
function variantName(type: StorageNodeType) {
    return type
        .split(/[-_]/)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join('')
}

function folderNode(path: string, type: StorageNodeType): StorageNode {
    return {
        id: path,
        type,
        size: { actual: 0, symlink: 0 },
        paths: [{ path, kind: 'directory' }],
    }
}

function aggregate(node: StorageNode, children: StorageNode[]) {
    const size = { actual: 0, symlink: 0 }
    for (const child of children) addSize(size, child.size)
    node.size = size
    node.children = children
}

function isWithin(path: string, root: string) {
    return path === root || path.startsWith(`${root}/`)
}

/** `minimal_roots`: drops paths that lie inside another one. */
function minimalRoots(paths: string[]) {
    const sorted = [...paths].sort((a, b) => a.split('/').length - b.split('/').length)
    const roots: string[] = []
    for (const path of sorted) {
        if (!roots.some((root) => isWithin(path, root))) roots.push(path)
    }
    return roots
}

/** `scan_entry`: links are measured as references and never add to the owned bytes. */
function scanEntry(
    disk: DiskDir,
    entry: DiskEntry,
    path: string,
    depth: number,
    excluded: Set<string>,
    reference: boolean,
    visited: Set<string>,
): StorageNode | null {
    if (excluded.has(path)) return null
    const node = folderNode(path, 'folder')
    node.name = entry.name
    if (entry.kind === 'link') {
        const target = findEntry(disk, entry.target)
        if (!target || visited.has(entry.target)) return node
        visited.add(entry.target)
        const targetNode = scanEntry(disk, target, entry.target, 0, excluded, true, visited)
        if (targetNode) node.size.symlink = sizeTotal(targetNode.size)
        visited.delete(entry.target)
        return node
    }
    if (entry.kind === 'file') {
        node.paths[0].kind = 'file'
        if (reference) node.size.symlink = entry.size
        else node.size.actual = entry.size
        node.count = 1
        return node
    }
    if (!entry.children) {
        const summary = entry.summary ?? { files: 0, bytes: 0 }
        if (reference) node.size.symlink = summary.bytes
        else node.size.actual = summary.bytes
        node.count = summary.files
        return node
    }
    const children: StorageNode[] = []
    let count = 0
    for (const child of entry.children) {
        const childNode = scanEntry(
            disk,
            child,
            `${path}/${child.name}`,
            Math.max(0, depth - 1),
            excluded,
            reference,
            visited,
        )
        if (!childNode) continue
        addSize(node.size, childNode.size)
        count += childNode.count ?? 0
        if (depth > 0 && sizeTotal(childNode.size) > 0) children.push(childNode)
    }
    node.count = count
    if (children.length) node.children = children
    return node
}

function scan(disk: DiskDir, path: string, depth: number, excluded: Set<string>) {
    const entry = findEntry(disk, path)
    if (!entry) return null
    return scanEntry(disk, entry, path, depth, excluded, false, new Set())
}

/** History files an update keeps for a rollback (`history_cleanup::recognizable`). */
function recognizableHistoryFile(name: string) {
    const resource = name.match(/^\.resource-(.+)-(\d+)\.old$/)
    if (resource) {
        return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(resource[1])
    }
    if (!name.endsWith('.old')) return false
    const stem = name.slice(0, -4)
    return ['.jar_', '.zip_'].some((separator) => {
        const index = stem.indexOf(separator)
        if (index <= 0) return false
        const old = stem.slice(index + separator.length)
        return old.endsWith('.jar') || old.endsWith('.zip')
    })
}

/** `classify_children`. */
function classifyChildren(node: StorageNode, category: StorageNodeType) {
    for (const child of node.children ?? []) {
        const name = child.name ?? ''
        let kind: StorageNodeType = 'folder'
        if (category === 'meta') {
            kind =
                (
                    {
                        versions: 'versions',
                        libraries: 'libraries',
                        assets: 'assets',
                        natives: 'natives',
                        java_versions: 'java_versions',
                        log_configs: 'log_configs',
                        'game-locales': 'game_locales',
                        resources: 'resources',
                    } as Record<string, StorageNodeType>
                )[name] ?? 'folder'
        } else if (category === 'cache') {
            if (name === 'content-store') kind = 'content_store'
            else if (name === 'resources') kind = 'resource_cache'
            else if (name === 'resource-staging' || name === 'modpack-import') kind = 'staging'
        } else if (category === 'instance') {
            kind =
                (
                    {
                        mods: 'mods',
                        resourcepacks: 'resourcepacks',
                        shaderpacks: 'shaderpacks',
                        saves: 'saves',
                        schematics: 'schematics',
                        screenshots: 'screenshots',
                        flashback: 'replay',
                        replay_recordings: 'replay',
                        voxelmap: 'minimap',
                        xaero: 'minimap',
                        XaeroWaypoints_BACKUP: 'minimap',
                        '.voxy': 'distant-horizons',
                        Distant_Horizons_server_data: 'distant-horizons',
                    } as Record<string, StorageNodeType>
                )[name] ?? 'folder'
        }
        if (kind === 'folder') {
            if (name === 'logs') kind = 'logs'
            else if (name === 'crash-reports') kind = 'crash_reports'
        }
        if (kind !== 'folder') child.name = undefined
        child.type = kind
        if (node.instance_id) child.instance_id = node.instance_id
        else delete child.instance_id
        if (
            (child.type === 'mods' ||
                child.type === 'resourcepacks' ||
                child.type === 'shaderpacks') &&
            child.children
        ) {
            const entries = child.children
            child.children = undefined
            const previous = entries.filter(
                (entry) =>
                    entry.paths[0]?.kind === 'file' && recognizableHistoryFile(entry.name ?? ''),
            )
            const current = entries.filter((entry) => !previous.includes(entry))
            if (previous.length) {
                const backups = folderNode(child.paths[0].path, 'resource_backups')
                backups.id = `${child.id}-previous`
                backups.paths = previous.flatMap((entry) => entry.paths)
                aggregate(backups, previous)
                current.push(backups)
            }
            child.children = current
        }
        if (child.type === 'saves') {
            for (const world of child.children ?? []) {
                world.type = 'world'
                if (node.instance_id) world.instance_id = node.instance_id
            }
        }
    }
}

interface ScanTarget {
    path: string
    kind: StorageNodeType
    depth: number
}

/** The scan targets of `scan_with_directories_progress`, in its order. */
function scanTargets(disk: DiskDir, backup: string): ScanTarget[] {
    const targets: ScanTarget[] = [
        { path: `${ROOT}/profiles`, kind: 'instances', depth: 3 },
        { path: `${ROOT}/caches`, kind: 'cache', depth: 2 },
        { path: `${ROOT}/meta`, kind: 'meta', depth: 1 },
        { path: backup, kind: 'backups', depth: 1 },
        { path: `${ROOT}/servers`, kind: 'servers', depth: 2 },
        { path: `${ROOT}/synced-options`, kind: 'synced_options', depth: 1 },
        { path: `${ROOT}/install-rollbacks`, kind: 'install_rollbacks', depth: 1 },
        { path: `${ROOT}/icons`, kind: 'icons', depth: 0 },
        { path: `${ROOT}/hongshi`, kind: 'components', depth: 1 },
        { path: `${ROOT}/terracotta`, kind: 'components', depth: 1 },
        { path: `${ROOT}/launcher_logs`, kind: 'launcher_logs', depth: 0 },
        { path: `${ROOT}/release`, kind: 'database', depth: 1 },
        { path: `${ROOT}/beta`, kind: 'database', depth: 1 },
        { path: `${ROOT}/Backups/app-db`, kind: 'db-backup', depth: 1 },
    ]
    const extra: [string, StorageNodeType, number][] = [
        ['caches', 'cache', 2],
        ['hongshi', 'components', 1],
        ['terracotta', 'components', 1],
        ['install-rollbacks', 'install_rollbacks', 1],
        ['icons', 'icons', 0],
        ['instance-shortcut-icons', 'icons', 0],
        ['backgrounds', 'launcher_data', 1],
        ['app-window-state.json', 'launcher_data', 0],
        ['instance_groups.json', 'launcher_data', 0],
        ['native-download-reputation.json', 'launcher_data', 0],
        ['update-channel.json', 'launcher_data', 0],
        ['download.log', 'launcher_logs', 0],
    ]
    // The settings and config directories are the same folder here.
    for (const root of [ROOT, ROOT]) {
        for (const [name, kind, depth] of extra) {
            const path = `${root}/${name}`
            if (!targets.some((target) => target.path === path)) targets.push({ path, kind, depth })
        }
        for (const candidate of [`${root}/Backups`, `${root}/Backups/instances`]) {
            if (
                !targets.some((target) => target.path === candidate) &&
                findEntry(disk, `${candidate}/backup.db`)?.kind === 'file'
            ) {
                targets.push({ path: candidate, kind: 'backups', depth: 1 })
            }
        }
    }
    return targets
}

/** How long reading a folder takes, from the number of files below it. */
function readDuration(entry: DiskEntry | null) {
    if (!entry) return 4
    return Math.min(1_800, 16 + measure(entry).files * 0.42)
}

function newId() {
    const random = globalThis.crypto?.randomUUID?.()
    if (random) return random
    const hex = hexName(`${Date.now()}:${Math.random()}`, 32)
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-a${hex.slice(17, 20)}-${hex.slice(20, 32)}`
}

/** The storage commands, bound to the window's storage state. */
export function useStorageBackend() {
    return storageBackend(usePreview())
}

export function storageBackend(preview: PreviewState) {
    const state = previewDomain<StorageBackendState>(preview, 'settings:storage', (source) => ({
        value: createState(source),
    }))
    return createStorageBackend(preview, state)
}

function createStorageBackend(preview: PreviewState, state: StorageBackendState) {
    function wait(ms: number) {
        return new Promise<void>((resolve) => {
            preview.clock.after(Math.max(0, Math.round(ms)), resolve)
        })
    }

    function withScanLock<T>(task: () => Promise<T>): Promise<T> {
        const run = state.scanLock.then(task, task)
        state.scanLock = run.then(
            () => undefined,
            () => undefined,
        )
        return run
    }

    /** Holds the exclusive metadata lock while `task` runs. */
    async function withExclusive<T>(task: () => Promise<T>): Promise<T> {
        // Launch preparation holds a shared lease on the metadata.
        if (
            preview.processes.processes.value.some(
                (entry) => entry.stage !== 'running' && entry.stage !== 'stopping',
            )
        ) {
            throw new Error(
                'Metadata is in use. Wait for launch preparation or installation to finish.',
            )
        }
        let release!: () => void
        state.exclusive = new Promise<void>((resolve) => {
            release = resolve
        })
        try {
            return await task()
        } finally {
            state.exclusive = null
            release()
        }
    }

    function repositoryPath() {
        return state.repositoryPath ?? DEFAULT_REPOSITORY
    }

    function rootsKey() {
        return `${ROOT}\u001f${ROOT}\u001f${repositoryPath()}`
    }

    function context(now = Date.now(), asOf = now): DiskContext {
        return {
            preview,
            now,
            asOf,
            meta: state.meta,
            cache: state.cache,
            snapshotBytes: asOf < now ? null : (state.snapshot?.bytes ?? null),
            snapshotModified: state.snapshot?.modified ?? now,
            repositoryPath: repositoryPath(),
            userFolders: state.userFolders,
            sessionStart: state.createdAt,
        }
    }

    function instances(): GameInstance[] {
        return preview.instances.instances.value
    }

    /** Whether a store object is still referenced by an installed file (`store_instance_files`). */
    function objectReferenced(source: { instanceId: string; fileName: string } | null) {
        if (!source) return false
        if (!instances().some((instance) => instance.id === source.instanceId)) return false
        return (preview.downloads.contents.value[source.instanceId] ?? []).some(
            (item) => item.file_name === source.fileName && !item.removed,
        )
    }

    /** Publishes individually installed content to the store, as its install did. */
    function syncContentStore() {
        const cache = state.cache
        for (const instance of instances()) {
            for (const item of preview.downloads.contents.value[instance.id] ?? []) {
                if (!publishedThroughStore(instance, item)) continue
                const key = `${instance.id}/${item.file_name}`
                if (cache.published.includes(key)) continue
                cache.published.push(key)
                cache.objects.push({
                    digest: hexName(`object:${instance.id}:${item.file_name}:${item.size}`, 128),
                    size: item.size,
                    modified: Date.parse(item.installed_at) || Date.now(),
                    source: { instanceId: instance.id, fileName: item.file_name },
                })
            }
        }
    }

    /** `caches/modpacks/<project>/<version>/<file>`: the pack file each pack install downloaded. */
    async function listPackFiles() {
        for (const instance of instances()) {
            const link = instance.link
            if (link?.type !== 'modrinth_modpack') continue
            const key = `${instance.id}:${link.version_id}`
            if (state.listedPacks.includes(key)) continue
            state.listedPacks.push(key)
            const project = await loadProject(link.project_id).catch(() => null)
            const version = project?.versions.find((entry) => entry.id === link.version_id)
            const packFile = version?.files.find((entry) => entry.primary) ?? version?.files[0]
            if (!packFile) continue
            state.cache.files.push({
                path: `modpacks/${link.project_id}/${link.version_id}/${packFile.filename}`,
                size: packFile.size,
                modified: Date.parse(instance.created),
            })
        }
    }

    async function prepare() {
        await preview.downloads.ensureContents()
        ensureMetaFor(state.meta, instances())
        syncContentStore()
        await listPackFiles()
    }

    function currentDisk(now = Date.now()) {
        return buildDisk(context(now))
    }

    /** `scan_with_directories_progress`. */
    async function scanDisk(
        disk: DiskDir,
        ctx: DiskContext,
        progress: ((event: StorageScanEvent) => void) | undefined,
        timed: boolean,
    ): Promise<StorageTree> {
        const warnings: string[] = []
        const backup = ctx.repositoryPath
        const targets = scanTargets(disk, backup)
        const categories: StorageNode[] = []
        const claimed = new Set<string>()
        const totalSections = targets.length + 2
        const list = ctx.preview.instances.instances.value.filter(
            (instance) => Date.parse(instance.created) <= ctx.asOf,
        )
        for (const [index, target] of targets.entries()) {
            progress?.({
                kind: 'progress',
                completed: index,
                total: totalSections,
                category: target.kind,
            })
            if (claimed.has(target.path)) continue
            claimed.add(target.path)
            const excluded = new Set(
                targets
                    .filter(
                        (other) => other.path !== target.path && isWithin(other.path, target.path),
                    )
                    .map((other) => other.path),
            )
            if (timed) await wait(readDuration(findEntry(disk, target.path)))
            const node = scan(disk, target.path, target.depth, excluded)
            if (!node) continue
            node.type = target.kind
            if (target.kind !== 'components' && target.kind !== 'database') node.name = undefined
            classifyChildren(node, target.kind)
            if (target.kind === 'instances') {
                for (const child of node.children ?? []) {
                    const instance = list.find((entry) => entry.path === child.id)
                    if (!instance) continue
                    child.name = instance.name
                    child.type = 'instance'
                    child.instance_id = instance.id
                    classifyChildren(child, 'instance')
                }
            }
            if (sizeTotal(node.size) === 0) continue
            const existingIndex = categories.findIndex((existing) => existing.type === target.kind)
            if (existingIndex === -1) {
                categories.push(node)
                continue
            }
            const existing = categories[existingIndex]
            if (node.name === undefined) node.name = target.path
            if (existing.id.startsWith('category-')) {
                existing.paths.push(...node.paths)
                aggregate(existing, [...(existing.children ?? []), node])
            } else {
                const old = { ...existing }
                if (old.name === undefined) old.name = old.paths[0]?.path
                const parent = folderNode(target.path, target.kind)
                parent.id = `category-${variantName(target.kind)}`
                parent.paths = [...old.paths, ...node.paths]
                aggregate(parent, [old, node])
                categories[existingIndex] = parent
            }
        }

        progress?.({
            kind: 'progress',
            completed: targets.length,
            total: totalSections,
            category: 'other',
        })
        const owned = minimalRoots([ROOT, ROOT, backup])
        if (timed) await wait(120 + owned.length * 40)
        const remainder: StorageNode[] = []
        for (const root of owned) {
            const node = scan(disk, root, 2, claimed)
            if (node && sizeTotal(node.size) > 0) remainder.push(...(node.children ?? []))
        }
        let rootOther: StorageNode | null = null
        if (remainder.length) {
            rootOther = folderNode(ROOT, 'other')
            rootOther.id = 'root-other'
            rootOther.paths = remainder.flatMap((child) => child.paths)
            aggregate(rootOther, remainder)
        }

        progress?.({
            kind: 'progress',
            completed: targets.length + 1,
            total: totalSections,
            category: 'instance',
        })
        const external: StorageNode[] = []
        const externalRoots = list
            .map((instance) => instance.path)
            .filter((path) => !owned.some((root) => isWithin(path, root)))
        for (const path of minimalRoots(externalRoots)) {
            if (timed) await wait(readDuration(findEntry(disk, path)))
            const node = scan(disk, path, 2, new Set())
            if (!node) continue
            const matching = list.filter((instance) => isWithin(instance.path, path))
            node.name = matching.map((instance) => instance.name).join(', ')
            node.type = 'instance'
            node.instance_id = matching[0]?.id
            classifyChildren(node, 'instance')
            external.push(node)
        }

        const total = { actual: 0, symlink: 0 }
        for (const category of categories) addSize(total, category.size)
        if (rootOther) addSize(total, rootOther.size)
        progress?.({
            kind: 'progress',
            completed: totalSections,
            total: totalSections,
            category: 'other',
        })
        return {
            version: STORAGE_CACHE_VERSION,
            scannedAt: new Date(ctx.now).toISOString(),
            total,
            categories,
            rootOther,
            external,
            warnings: warnings.slice(0, 100),
        }
    }

    function writeSnapshot(tree: StorageTree, modified: number) {
        state.snapshot = {
            tree: structuredClone(tree),
            // CBOR is a little denser than the JSON form of the same tree.
            bytes: Math.round(JSON.stringify(tree).length * 0.71),
            roots: rootsKey(),
            modified,
        }
    }

    /** The scan the previous app session cached, six days ago. */
    async function seedPreviousSnapshot() {
        if (state.seeded) return
        state.seeded = true
        const at = Date.now() - PREVIOUS_SCAN_AGO
        const ctx = context(at, at)
        const tree = await scanDisk(buildDisk(ctx), ctx, undefined, false)
        writeSnapshot(tree, at)
    }

    /** `plugin:storage|storage_scan`. */
    function scanStorage(
        force: boolean,
        progress?: (event: StorageScanEvent) => void,
    ): Promise<StorageTree> {
        return withScanLock(async () => {
            await prepare()
            await seedPreviousSnapshot()
            const snapshot = state.snapshot
            if (!force && snapshot && snapshot.roots === rootsKey()) {
                await wait(40)
                progress?.({ kind: 'cached', tree: structuredClone(snapshot.tree) })
                if (Date.now() - Date.parse(snapshot.tree.scannedAt ?? '') < FRESH_SNAPSHOT_MS) {
                    await wait(20)
                    return structuredClone(snapshot.tree)
                }
            }
            const now = Date.now()
            const ctx = context(now)
            const tree = await scanDisk(buildDisk(ctx), ctx, progress, true)
            writeSnapshot(tree, Date.now())
            return structuredClone(tree)
        })
    }

    /** `previewCacheUsage()`: image payloads the WebView keeps in IndexedDB. */
    async function previewCacheUsage(): Promise<number | null> {
        await wait(30)
        return previewCacheBytes(preview)
    }

    /* ---- Metadata reachability (storage/meta.rs) ------------------------ */

    interface References {
        versions: Set<string>
        natives: Set<string>
        games: Set<string>
        manifests: Set<string>
        blocked: boolean
        busy: boolean
        warnings: MetaWarning[]
    }

    /** `references()`: every channel database's instances, read-only. */
    function references(): References {
        const refs: References = {
            versions: new Set(),
            natives: new Set(),
            games: new Set(),
            manifests: new Set(),
            blocked: false,
            busy: false,
            warnings: [],
        }
        // Queued or running install jobs (content installs included).
        refs.busy = preview.downloads.jobs.value.length > 0
        const overrides = instances().map((instance) =>
            JSON.stringify({
                java_path: instance.java_path ?? null,
                extra_launch_args: instance.extra_launch_args ?? null,
                hooks: instance.hooks,
            }),
        )
        const settings = [JSON.stringify(preview.settings.extra_launch_args ?? [])]
        if (
            [...overrides, ...settings].some((value) => {
                const lower = value.toLowerCase()
                return (
                    lower.includes('.jar') ||
                    lower.includes('.zip') ||
                    lower.includes('meta/') ||
                    lower.includes('meta\\\\')
                )
            })
        ) {
            refs.blocked = true
            refs.warnings.push({ kind: 'custom_arguments', subject: '', detail: '' })
        }
        // The Release database lists every instance.
        for (const instance of instances()) {
            if (instance.loader !== 'vanilla' && !instance.loader_version) {
                refs.blocked = true
                refs.warnings.push({
                    kind: 'database_unavailable',
                    subject: `${ROOT}/release/app.db`,
                    detail: 'An instance has no exact loader version',
                })
                continue
            }
            const manifest: LaunchManifest = manifestFor(instance)
            refs.versions.add(manifest.game)
            refs.games.add(manifest.game)
            refs.manifests.add(manifest.id)
            if (manifest.id !== manifest.game) {
                refs.versions.add(manifest.id)
                refs.natives.add(manifest.id)
            } else {
                refs.natives.add(manifest.game)
            }
        }
        if (refs.busy) refs.warnings.push({ kind: 'active_operation', subject: '', detail: '' })
        return refs
    }

    /** `References::used` for one cleanup candidate: `true` used, `false` unused, `null` unknown. */
    function usage(ref: CandidateRef, refs: References): boolean | null {
        const uncertain = refs.blocked || refs.busy
        switch (ref.kind) {
            case 'version-file': {
                if (refs.versions.has(ref.version)) return true
                if (uncertain) return null
                const own = [
                    `${ref.version}.jar`,
                    `${ref.version}.json`,
                    `${ref.version}.json.axolotl-format`,
                ]
                return own.includes(ref.name) ? false : null
            }
            case 'natives':
                if (refs.natives.has(ref.id)) return true
                return uncertain ? null : false
            case 'shared': {
                const entry = state.meta.shared.find((shared) => shared.key === ref.key)
                if (!entry) return null
                const referenced =
                    entry.games.some((game) => refs.games.has(game)) ||
                    entry.manifests.some((manifest) => refs.manifests.has(manifest))
                if (referenced) return true
                return uncertain ? null : false
            }
        }
    }

    function sortById(a: { id: string }, b: { id: string }) {
        return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
    }

    /** `plugin:storage|storage_meta_analyze`. */
    async function analyzeMeta(): Promise<MetaAnalysis> {
        // A shared lease: waits while a cleanup holds the exclusive lock.
        while (state.exclusive) await state.exclusive
        await prepare()
        const refs = references()
        const meta = state.meta
        const files =
            meta.versions.reduce((sum, version) => sum + version.files.length, 0) +
            meta.natives.reduce((sum, entry) => sum + entry.files, 0) +
            meta.shared.reduce((sum, entry) => sum + entry.files, 0) +
            meta.runtimes.reduce((sum, runtime) => sum + runtime.files, 0)
        await wait(Math.min(2_600, 700 + files * 0.16))

        const groups = new Map<string, MetaGroup>()
        const candidates: Candidate[] = []
        const group = (id: string, category: string, name: string) => {
            const existing = groups.get(id)
            if (existing) return existing
            const created: MetaGroup = {
                id,
                category,
                name,
                used: 0,
                unused: 0,
                unknown: 0,
                files: 0,
            }
            groups.set(id, created)
            return created
        }
        const account = (target: MetaGroup, ref: CandidateRef, count: number, bytes: number) => {
            target.files += count
            const used = usage(ref, refs)
            if (used === true) target.used += bytes
            else if (used === false) {
                target.unused += bytes
                candidates.push({ group: target.id, ref, files: count, size: bytes })
            } else target.unknown += bytes
        }
        for (const version of meta.versions) {
            const target = group(`versions/${version.id}`, 'versions', version.id)
            for (const entry of version.files) {
                account(
                    target,
                    { kind: 'version-file', version: version.id, name: entry.name },
                    1,
                    entry.size,
                )
            }
        }
        for (const entry of meta.natives) {
            account(
                group(`natives/${entry.id}`, 'natives', entry.id),
                { kind: 'natives', id: entry.id },
                entry.files,
                entry.bytes,
            )
        }
        for (const entry of meta.shared) {
            account(
                group(entry.category, entry.category, ''),
                { kind: 'shared', key: entry.key },
                entry.files,
                entry.bytes,
            )
        }
        for (const runtime of meta.runtimes) {
            // Java runtimes are never referenced by path, so they are always kept.
            const target = group(`java_versions/${runtime.name}`, 'java_versions', runtime.name)
            target.files += runtime.files
            target.unknown += runtime.bytes
        }

        const id = newId()
        const now = Date.now()
        for (const [planId, plan] of state.plans) {
            if (now - plan.created >= PLAN_AGE_MS) state.plans.delete(planId)
        }
        if (state.plans.size >= 4) state.plans.clear()
        state.plans.set(id, { created: now, candidates })
        return {
            id,
            scanned_at: new Date(now).toISOString(),
            groups: [...groups.values()].filter((entry) => entry.files > 0).sort(sortById),
            warnings: refs.warnings.slice(0, 100),
        }
    }

    function removeCandidate(candidate: Candidate): boolean {
        const meta = state.meta
        const ref = candidate.ref
        if (ref.kind === 'version-file') {
            const version = meta.versions.find((entry) => entry.id === ref.version)
            const index = version?.files.findIndex((entry) => entry.name === ref.name) ?? -1
            if (!version || index === -1) return false
            version.files.splice(index, 1)
            // `prune_empty_parents`.
            if (!version.files.length) meta.versions.splice(meta.versions.indexOf(version), 1)
            return true
        }
        if (ref.kind === 'natives') {
            const index = meta.natives.findIndex((entry) => entry.id === ref.id)
            if (index === -1) return false
            meta.natives.splice(index, 1)
            return true
        }
        const index = meta.shared.findIndex((entry) => entry.key === ref.key)
        if (index === -1) return false
        meta.shared.splice(index, 1)
        return true
    }

    /** `plugin:storage|storage_meta_clean`. */
    function cleanMeta(id: string, selected: string[]): Promise<MetaCleanupResult> {
        return withScanLock(() =>
            withExclusive(async () => {
                if (preview.processes.processes.value.length) {
                    throw new Error('Stop running games before cleaning metadata')
                }
                await prepare()
                const refs = references()
                if (refs.blocked || refs.busy) {
                    throw new Error(
                        'Metadata references could not be verified or installation recovery is pending. Refresh the analysis after resolving it.',
                    )
                }
                const plan = state.plans.get(id)
                if (!plan || Date.now() - plan.created >= PLAN_AGE_MS) {
                    throw new Error('Metadata analysis expired. Analyze again.')
                }
                const selection = new Set(selected)
                if (
                    !selection.size ||
                    [...selection].some(
                        (name) => !plan.candidates.some((candidate) => candidate.group === name),
                    )
                ) {
                    throw new Error('Select unused metadata from the current analysis')
                }
                state.plans.delete(id)
                const chosen = plan.candidates.filter((candidate) => selection.has(candidate.group))
                await wait(
                    Math.min(
                        2_400,
                        260 + chosen.reduce((sum, candidate) => sum + candidate.files, 0) * 3.2,
                    ),
                )
                const result: MetaCleanupResult = {
                    removed_files: 0,
                    removed_bytes: 0,
                    removed_groups: {},
                    skipped_files: 0,
                    failures: [],
                }
                for (const candidate of chosen) {
                    if (usage(candidate.ref, refs) !== false || !removeCandidate(candidate)) {
                        result.skipped_files += candidate.files
                        continue
                    }
                    result.removed_files += candidate.files
                    result.removed_bytes += candidate.size
                    const removed = (result.removed_groups[candidate.group] ??= {
                        files: 0,
                        bytes: 0,
                    })
                    removed.files += candidate.files
                    removed.bytes += candidate.size
                }
                // The cached scan no longer matches the disk.
                state.snapshot = null
                return result
            }),
        )
    }

    /* ---- Cache purge (storage/cache.rs) --------------------------------- */

    function cacheBytes() {
        const caches = findEntry(currentDisk(), `${ROOT}/caches`)
        return caches ? measure(caches).bytes : 0
    }

    /** Only known download folders are disposable; icons, native archives and staging are kept. */
    function disposable(path: string) {
        const first = path.split('/')[0]
        return first === 'content' || first === 'curseforge' || first === 'modpacks'
    }

    /** `plugin:storage|storage_cache_clear`. */
    function purgeLauncherCache(): Promise<CacheCleanupResult> {
        return withScanLock(() =>
            withExclusive(async () => {
                await prepare()
                if (preview.downloads.jobs.value.length) {
                    throw new Error(
                        'Finish pending installations and recovery before clearing downloaded caches.',
                    )
                }
                const before = cacheBytes()
                // Purge the API cache tables, then clear and compact each resource cache.
                await wait(520)
                const now = Date.now()
                for (const database of state.cache.resourceDbs) {
                    database.bytes = database.channel === 'beta' ? 61_440 : 57_344
                    database.wal = 0
                    database.modified = now
                }
                await wait(420)
                const removedFiles =
                    state.cache.files.filter((entry) => disposable(entry.path)).length +
                    state.cache.objects.filter((object) => !objectReferenced(object.source)).length
                await wait(Math.min(1_200, 120 + removedFiles * 9))
                state.cache.files = state.cache.files.filter((entry) => !disposable(entry.path))
                state.cache.objects = state.cache.objects.filter((object) =>
                    objectReferenced(object.source),
                )
                // `launcher-storage.cbor` is disposable too.
                state.snapshot = null
                const remaining = cacheBytes()
                return {
                    removed_bytes: Math.max(0, before - remaining),
                    remaining_bytes: remaining,
                    failures: [],
                }
            }),
        )
    }

    /* ---- Opening locations ------------------------------------------------ */

    /** `plugin:storage|storage_open_paths`: folders open, files are revealed in their folder. */
    async function openStoragePaths(paths: StoragePath[]): Promise<StorageOpenResult> {
        await wait(30)
        const disk = currentDisk()
        const opened: string[] = []
        const failed: { path: string; reason: string }[] = []
        for (const entry of paths) {
            if (findEntry(disk, entry.path)) opened.push(entry.path)
            else failed.push({ path: entry.path, reason: 'No such file or directory (os error 2)' })
        }
        return { opened, failed }
    }

    /* ---- Backup repository (api/instance/backup.rs) --------------------- */

    function emit(event: BackupProgressEvent) {
        for (const listener of [...state.listeners]) listener(event)
    }

    /** `instance_get_backup_repository_status`. */
    async function getBackupRepositoryStatus(): Promise<BackupRepositoryStatus> {
        await wait(110)
        const path = repositoryPath()
        // No instance has backups enabled, so the repository has no database yet.
        const initialized = findEntry(currentDisk(), `${path}/backup.db`)?.kind === 'file'
        return {
            path,
            initialized,
            available: true,
            stored_size: 0,
            logical_size: 0,
            snapshot_count: 0,
            error: null,
        }
    }

    /** `move_repository_inner`. */
    async function moveRepositoryInner(destination: string) {
        await wait(260)
        const source = repositoryPath()
        if (!destination.startsWith('/')) {
            throw new Error('The backup repository destination must be an absolute path')
        }
        if (source === destination) return
        const disk = currentDisk()
        const existing = findEntry(disk, destination)
        if (existing) {
            if (existing.kind !== 'dir') throw new Error('Not a directory (os error 20)')
            if ((existing.children?.length ?? existing.summary?.files ?? 0) > 0) {
                throw new Error('The new backup repository directory must be empty')
            }
        } else {
            state.userFolders.push(destination)
        }
        // Without a repository database there is nothing to copy.
        state.repositoryPath = destination === DEFAULT_REPOSITORY ? null : destination
        // The scan reads a different set of roots now.
        state.snapshot = null
    }

    /**
     * `instance_move_backup_repository`: starts the move and returns its operation
     * id; progress arrives through `instance_backup_progress`. `onFinished` stands
     * in for the action bar's result notification.
     */
    async function moveBackupRepository(
        destination: string,
        onFinished?: (event: BackupProgressEvent) => void,
    ): Promise<string> {
        await wait(40)
        const status = await getBackupRepositoryStatus()
        const operationId = newId()
        const total = status.stored_size
        state.move = { id: operationId, processed: 0, total, created: Date.now() }
        void (async () => {
            emit({
                operationId,
                operationType: 'repository_move',
                stage: 'copying',
                processedBytes: 0,
                totalBytes: total,
            })
            let message: string | undefined
            try {
                await moveRepositoryInner(destination)
            } catch (error) {
                message = error instanceof Error ? error.message : String(error)
            }
            state.move = null
            const event: BackupProgressEvent = {
                operationId,
                operationType: 'repository_move',
                stage: message ? 'failed' : 'completed',
                processedBytes: message ? 0 : total,
                totalBytes: total,
                finalState: message ? 'failed' : 'completed',
                message,
            }
            emit(event)
            onFinished?.(event)
        })()
        return operationId
    }

    /** `instance_list_backup_operations`. */
    async function listBackupOperations(
        _instanceId?: string,
        _activeOnly = false,
    ): Promise<BackupOperation[]> {
        await wait(20)
        const move = state.move
        if (!move) return []
        return [
            {
                id: move.id,
                operation_type: 'repository_move',
                instance_id: REPOSITORY_OPERATION_KEY,
                state: 'materializing',
                processed_bytes: move.processed,
                total_bytes: move.total,
                cancellable: false,
                cancel_requested: false,
                created_at: move.created,
                updated_at: Date.now(),
            },
        ]
    }

    /** `listen('instance_backup_progress')`. */
    async function listenBackupProgress(handler: (event: BackupProgressEvent) => void) {
        state.listeners.add(handler)
        return () => {
            state.listeners.delete(handler)
        }
    }

    /** Creates a folder from the open panel. */
    function createFolder(path: string) {
        if (!state.userFolders.includes(path)) state.userFolders.push(path)
    }

    return {
        scanStorage,
        previewCacheUsage,
        analyzeMeta,
        cleanMeta,
        purgeLauncherCache,
        openStoragePaths,
        getBackupRepositoryStatus,
        moveBackupRepository,
        listBackupOperations,
        listenBackupProgress,
        createFolder,
        currentDisk,
        repositoryPath,
    }
}

export type StorageBackend = ReturnType<typeof createStorageBackend>
