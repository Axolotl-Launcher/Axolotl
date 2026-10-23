import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'
import { type Ref, ref, shallowRef } from 'vue'
import type { Router } from 'vue-router'

import { loadInstanceContent, projectIcon } from '../../data'
import modpackSnapshot from '../../data/modpacks.json'
import type { PreviewState } from '../../lib/state'
import type { ContentItem } from '../../lib/state/downloads'
import type { GameInstance, InstanceLoader } from '../../lib/types'
import {
    type ClassificationResult,
    classifyArchiveNode,
    classifyFolderNode,
    classifyPickedFile,
    classifyZipWithExtraction,
} from './classify'
import { getLoaderInstanceIconUrl } from './icons'
import { GAME_FILES, runInstallJob } from './jobs'
import {
    baseName,
    childNamed,
    findNode,
    type FsNode,
    HOME,
    joinPath,
    loadMacFileSystem,
    measure,
} from './mac-fs'
import { dropNotificationMessages as messages } from './messages'
import { type GameVersionTag, loaderVersionsForGameVersion, loadLoaderManifests } from './metadata'
import { type LocalModpackInfo, readLocalModpack } from './modpack'
import type { ConfiguredMinecraftDirectory } from './state'
import { openZip, readZipJson, type ZipArchive } from './zip'

// ── Shapes shared with the drop modals (`packages/ui/src/providers/instance-import.ts`) ──

export interface ScanInstance {
    name: string
    path: string
    version: string
    loader: string
    compatibleMode?: boolean
    versionPath?: string
}

export interface ScanResult {
    launcherName: string
    launcherType: string
    instances: ScanInstance[]
}

export interface LauncherSelection {
    launcherType: string
    launcherName: string
    instances: { name: string; path: string; compatibleMode?: boolean; versionPath?: string }[]
}

export interface SymlinkMethodInstance {
    name: string
    path?: string
    launcherType?: string
    basePath?: string
    versionPath?: string
    compatibleMode?: boolean
}

export interface SymlinkMethodChoice {
    instanceName: string
    instancePath?: string
    symlink: boolean
    gameVersion?: string | null
    loader?: string | null
    loaderVersion?: string | null
    gameDirOverride?: string | null
}

export interface ImportPlanCounts {
    files: number
    bytes: number
}

export type ImportPlanStage = 'resolving' | 'scanning' | 'done' | 'error'

export interface ImportPlanSnapshot {
    requestId: string
    stage: ImportPlanStage
    gameVersion: string | null
    loader: string | null
    loaderVersion: string | null
    importPath: string
    minecraftRoot: string
    modCount: number
    cache: ImportPlanCounts
    local: ImportPlanCounts
    network: ImportPlanCounts
    migrate: ImportPlanCounts
    error?: string | null
}

export interface ImportPlanRequest {
    requestId: string
    launcherType: string
    basePath: string
    instanceFolder: string
    instancePath?: string | null
    gameVersion?: string | null
    loader?: string | null
    loaderVersion?: string | null
}

export interface GenericInstallTarget {
    id: string
    name: string
    iconUrl?: string | null
    gameVersion?: string | null
    loader?: string | null
}

export type SymlinkCapability = 'supported' | 'requires_admin' | 'unsupported'

type FormatMessage = (descriptor: MessageDescriptor, values?: Record<string, unknown>) => string

/** What the hosting page binds: its modals, router and formatter. */
export interface ImportFlowBindings {
    formatMessage: FormatMessage
    router: Router
    confirmDrop: { show: () => void; hide: () => void } | null
    launcherImport: { show: (results: ScanResult[]) => void; hide: () => void } | null
    symlinkCards: {
        show: (options: {
            instances: SymlinkMethodInstance[]
            symlinkCapable: SymlinkCapability
        }) => void
        hide: () => void
    } | null
    unknownPack: { show: (onProceed: () => Promise<void>, fileName: string) => void } | null
    genericInstall: {
        show: (options: {
            contentType: string
            fileName: string
            instances: GenericInstallTarget[]
        }) => void
        hide: () => void
    } | null
    folderPanel: { pick: () => Promise<string | null>; browse: (path: string) => void } | null
    filePicker: { pick: () => Promise<{ file: File; path: string } | null> } | null
}

interface ImportFlowDeps {
    ensureData: () => Promise<void>
    gameVersions: Ref<GameVersionTag[]>
    addMinecraftDirectories: (roots: ConfiguredMinecraftDirectory[]) => void
    hideCreationModal: () => void
}

interface InstanceDetails {
    gameVersion: string | null
    loader: string | null
    loaderVersion: string | null
    minecraftRoot: string
    mods: {
        projectId: string | null
        versionId: string | null
        title: string
        fileName: string
        size: number
        versionNumber: string | null
    }[]
    migrate: ImportPlanCounts
}

const LOADER_FILES: ImportPlanCounts = { files: 9, bytes: 6 * 1024 * 1024 }
/** macOS `$TMPDIR` of the preview Mac, where launcher archives are extracted. */
const TEMP_DIR = '/var/folders/7k/axl9d0bs2xq1/T'

const contentFolders: Record<string, string> = {
    mod: 'mods',
    resource_pack: 'resourcepacks',
    data_pack: 'datapacks',
    shader_pack: 'shaderpacks',
    litematic: 'schematics',
    schematic: 'schematics',
}

const contentProjectTypes: Record<string, string> = {
    mod: 'mod',
    resource_pack: 'resourcepack',
    data_pack: 'datapack',
    shader_pack: 'shaderpack',
    litematic: 'schematic',
    schematic: 'schematic',
}

export function createImportFlow(preview: PreviewState, deps: ImportFlowDeps) {
    const { notifications, popups, clock } = preview
    let bindings: ImportFlowBindings | null = null
    let formatMessage: FormatMessage = (descriptor) =>
        String(descriptor.defaultMessage ?? descriptor.id)
    let router: Router | null = null

    // ── Single-file drop state (`useDropImport`) ──────────────────────────
    const dropClassification = ref<ClassificationResult | null>(null)
    const dropFileName = ref('')
    const dropFilePath = ref('')
    const scanningInstances = ref(false)
    const processingNotificationId = ref<string | number | null>(null)
    const selectedInstances = shallowRef<
        {
            launcherType: string
            basePath: string
            name: string
            path: string
            compatibleMode?: boolean
            versionPath?: string
        }[]
    >([])
    let currentImportContext: { launcherType: string; basePath: string } | null = null
    let launcherZipTempDir: string | null = null
    let pendingInstall: { type: string; filePath: string; innerBase?: string } | null = null

    /** Files the visitor picked, by the path the flow uses for them. */
    const pickedFiles = new Map<string, File>()
    /** Launcher archives "extracted" to the temp dir: path → archive + inner prefix. */
    const zipMounts = new Map<string, ZipArchive>()

    function bind(next: ImportFlowBindings) {
        bindings = next
        formatMessage = next.formatMessage
        router = next.router
    }

    function unbind(owner: ImportFlowBindings) {
        if (bindings === owner) bindings = null
    }

    function addNotification(notification: Parameters<typeof notifications.addNotification>[0]) {
        return notifications.addNotification(notification)
    }

    function wait(ms: number) {
        return new Promise<void>((resolve) => clock.after(ms, resolve))
    }

    function clearDropProcessingNotification() {
        if (processingNotificationId.value !== null) {
            notifications.removeNotification(processingNotificationId.value)
            processingNotificationId.value = null
        }
    }

    function isZipPath(path: string) {
        return /\.zip$/i.test(path)
    }

    function unknownReasonMessage(reason: string | undefined): string {
        const normalized = reason?.toLowerCase() ?? ''
        if (normalized.includes('too deep') || normalized.includes('nesting')) {
            return formatMessage(messages.unknownDepthText)
        }
        if (normalized.includes('encrypted')) return formatMessage(messages.unknownEncryptedText)
        return reason ? reason : formatMessage(messages.unknownText)
    }

    // ── Sources: the simulated disk, picked files and extracted archives ──

    function mountFor(path: string): { archive: ZipArchive; prefix: string } | null {
        for (const [mount, archive] of zipMounts) {
            if (path === mount || path.startsWith(`${mount}/`)) {
                const rest = path.slice(mount.length).replace(/^\//, '')
                return { archive, prefix: rest ? `${rest}/` : '' }
            }
        }
        return null
    }

    async function classifyPath(
        path: string,
        allowNestedUnpack = false,
    ): Promise<ClassificationResult> {
        const picked = pickedFiles.get(path)
        if (picked) return classifyPickedFile(picked, path, allowNestedUnpack)
        const root = await loadMacFileSystem()
        const node = findNode(root, path)
        if (!node) return { item_type: 'unknown', reason: 'Path does not exist' }
        if (node.kind === 'dir') return classifyFolderNode(node, path)
        if (/\.(zip|mrpack)$/i.test(node.name)) return classifyArchiveNode(node, path)
        return { item_type: 'unknown', reason: `Unrecognised file type: ${path}` }
    }

    /** `scanLauncherInstances` (`drop_scan_launcher_instances`). */
    async function scanLauncherInstances(
        launcherType: string,
        basePath: string,
    ): Promise<ScanResult[]> {
        const instances: ScanInstance[] = []
        const mount = mountFor(basePath)
        if (mount) {
            const names = mount.archive.entries.map((entry) => entry.name)
            const seen = new Set<string>()
            for (const name of names) {
                if (!name.startsWith(mount.prefix)) continue
                const parts = name.slice(mount.prefix.length).split('/')
                let folder: string | null = null
                if (
                    (launcherType === 'MultiMC' || launcherType === 'PrismLauncher') &&
                    parts.length === 3 &&
                    parts[0] === 'instances' &&
                    parts[2] === 'instance.cfg'
                ) {
                    folder = `instances/${parts[1]}`
                } else if (
                    launcherType === 'Axolotl' &&
                    parts.length === 2 &&
                    parts[1] === 'axolotl_config.json'
                ) {
                    folder = parts[0]
                }
                if (!folder || seen.has(folder)) continue
                seen.add(folder)
                instances.push({
                    name: baseName(folder),
                    path: joinPath(basePath, folder),
                    version: '',
                    loader: '',
                })
            }
        } else {
            const root = await loadMacFileSystem()
            const base = findNode(root, basePath)
            if (!base || base.kind !== 'dir')
                throw new Error(`${basePath}: No such file or directory`)
            if (launcherType === 'MultiMC' || launcherType === 'PrismLauncher') {
                const folder = childNamed(base, 'instances')
                for (const child of folder?.children ?? []) {
                    if (child.kind === 'dir' && childNamed(child, 'instance.cfg')) {
                        instances.push({
                            name: child.name,
                            path: joinPath(basePath, folder!.name, child.name),
                            version: '',
                            loader: '',
                        })
                    }
                }
            } else if (launcherType === 'Axolotl') {
                for (const child of base.children ?? []) {
                    if (child.kind === 'dir' && childNamed(child, 'axolotl_config.json')) {
                        instances.push({
                            name: child.name,
                            path: joinPath(basePath, child.name),
                            version: '',
                            loader: '',
                        })
                    }
                }
            }
        }
        await wait(500 + instances.length * 120)
        return [{ launcherName: launcherType, launcherType, instances }]
    }

    /** `resolveGamedirs` (`gamedir_resolve.rs`). */
    async function resolveGamedirs(launcherType: string, basePath: string) {
        await wait(260)
        if (launcherType === 'Generic') {
            const root = await loadMacFileSystem()
            const node = pickedFiles.has(basePath) ? null : findNode(root, basePath)
            const versions = childNamed(node, 'versions')
            const hasManifest = (versions?.children ?? []).some(
                (child) =>
                    child.kind === 'dir' &&
                    (child.children ?? []).some((file) => /\.json$/i.test(file.name)),
            )
            if (!node || node.kind !== 'dir' || !hasManifest) {
                throw new Error('Invalid input: Selected folder is not a Minecraft game directory')
            }
            return [{ path: basePath, dialect: 'generic' }]
        }
        // PCL / PCL-CE / HMCL keep their registries on Windows; the preview Mac has none,
        // and its data dir has no `.minecraft` either.
        return []
    }

    /**
     * `sync_direct_links`: every `versions/<id>` with a version manifest under a
     * configured root becomes a direct-link record named after the folder and
     * grouped by its `.minecraft` path.
     */
    async function syncDirectLinks(roots: ConfiguredMinecraftDirectory[]) {
        const root = await loadMacFileSystem()
        const report = { imported: 0, updated: 0, removed: 0, missing: 0, errors: [] as string[] }
        for (const entry of roots) {
            const node = findNode(root, entry.path)
            if (!node || node.kind !== 'dir') {
                report.missing++
                report.errors.push(`${entry.path} is not a directory`)
                continue
            }
            const versions = childNamed(node, 'versions')
            if (!versions) {
                report.errors.push(
                    `${joinPath(entry.path, 'versions')}: No such file or directory (os error 2)`,
                )
                continue
            }
            for (const folder of versions.children ?? []) {
                if (folder.kind !== 'dir') continue
                const manifest = (folder.children ?? []).find(
                    (child) => child.meta?.type === 'version-json',
                )
                if (!manifest || manifest.meta?.type !== 'version-json') continue
                const folderPath = joinPath(entry.path, 'versions', folder.name)
                const existing = preview.instances.instances.value.find(
                    (instance) => instance.path === folderPath,
                )
                if (existing) {
                    report.updated++
                    continue
                }
                const created = preview.instances.create({
                    name: folder.name,
                    gameVersion: manifest.meta.gameVersion,
                    loader: manifest.meta.loader as InstanceLoader,
                    loaderVersion: manifest.meta.loaderVersion,
                    iconPath: null,
                })
                const instance = preview.instances.get(created.id) ?? created
                instance.path = folderPath
                preview.instances.createGroup(entry.path)
                preview.instances.setGroups(instance.id, [entry.path])
                preview.downloads.contents.value = {
                    ...preview.downloads.contents.value,
                    [instance.id]:
                        manifest.meta.loader === 'vanilla'
                            ? []
                            : contentItemsFor(modsOf(childNamed(node, 'mods'))),
                }
                report.imported++
            }
        }
        return report
    }

    async function routeToGamedirFlow(paths: string[]) {
        const roots = paths
            .filter((path) => path.trim())
            .map((path) => ({ path: path.trim(), mode: 'automatic' as const }))
        if (roots.length === 0) return false
        try {
            deps.addMinecraftDirectories(roots)
            const report = await syncDirectLinks(roots)
            if (report.errors.length > 0) throw new Error(report.errors.join('\n'))
            addNotification({
                title: formatMessage(messages.instanceImportedTitle),
                text: formatMessage(messages.instanceImportedText, { name: paths.join(', ') }),
                type: 'success',
            })
            return true
        } catch (error) {
            addNotification({
                title: formatMessage(messages.scanFailed),
                text: error instanceof Error ? error.message : String(error),
                type: 'error',
            })
            return false
        }
    }

    // ── Instance details and the import plan (`start_import_plan`) ────────

    function modsOf(node: FsNode | null) {
        return (node?.children ?? [])
            .filter((child) => child.kind === 'file' && /\.jar$/i.test(child.name))
            .map((child) => ({
                projectId: child.meta?.type === 'mod' ? child.meta.projectId : null,
                versionId: child.meta?.type === 'mod' ? child.meta.versionId : null,
                title:
                    child.meta?.type === 'mod'
                        ? child.meta.title
                        : child.name.replace(/\.jar$/i, ''),
                versionNumber: child.meta?.type === 'mod' ? child.meta.versionNumber : null,
                fileName: child.name,
                size: child.size,
            }))
    }

    function contentItemsFor(mods: InstanceDetails['mods']): ContentItem[] {
        const now = new Date().toISOString()
        return mods.map((mod) => ({
            id: `mods/${mod.fileName}`,
            project_id: mod.projectId,
            version_id: mod.versionId,
            name: mod.title,
            file_name: mod.fileName,
            enabled: true,
            icon: mod.projectId ? projectIcon(mod.projectId) : null,
            version_number: mod.versionNumber,
            project_type: 'mod',
            size: mod.size,
            installed_at: now,
        }))
    }

    async function describeInstance(path: string): Promise<InstanceDetails> {
        const mount = mountFor(path)
        if (mount) {
            const pack = await readZipJson<{ components?: { uid: string; version?: string }[] }>(
                mount.archive,
                `${mount.prefix}mmc-pack.json`,
            ).catch(() => null)
            const components = pack?.components ?? []
            const loaderUids: Record<string, string> = {
                'net.fabricmc.fabric-loader': 'fabric',
                'org.quiltmc.quilt-loader': 'quilt',
                'net.minecraftforge': 'forge',
                'net.neoforged': 'neoforge',
            }
            const loader = components.find((component) => loaderUids[component.uid])
            const runtime = mount.archive.entries.some((entry) =>
                entry.name.startsWith(`${mount.prefix}.minecraft/`),
            )
                ? '.minecraft'
                : 'minecraft'
            const runtimePrefix = `${mount.prefix}${runtime}/`
            const mods = mount.archive.entries
                .filter(
                    (entry) =>
                        !entry.directory &&
                        entry.name.startsWith(`${runtimePrefix}mods/`) &&
                        /\.jar$/i.test(entry.name),
                )
                .map((entry) => ({
                    projectId: null,
                    versionId: null,
                    title: baseName(entry.name).replace(/\.jar$/i, ''),
                    versionNumber: null,
                    fileName: baseName(entry.name),
                    size: entry.uncompressedSize,
                }))
            const migrateEntries = mount.archive.entries.filter(
                (entry) =>
                    !entry.directory &&
                    entry.name.startsWith(runtimePrefix) &&
                    !entry.name.startsWith(`${runtimePrefix}mods/`),
            )
            return {
                gameVersion:
                    components.find((component) => component.uid === 'net.minecraft')?.version ??
                    null,
                loader: loader ? loaderUids[loader.uid] : null,
                loaderVersion: loader?.version ?? null,
                minecraftRoot: joinPath(path, runtime),
                mods,
                migrate: {
                    files: migrateEntries.length,
                    bytes: migrateEntries.reduce(
                        (total, entry) => total + entry.uncompressedSize,
                        0,
                    ),
                },
            }
        }
        const root = await loadMacFileSystem()
        const node = findNode(root, path)
        const pack = (node?.children ?? []).find((child) => child.meta?.type === 'mmc-pack')
        const runtime = childNamed(node, 'minecraft') ?? childNamed(node, '.minecraft') ?? node
        const mods = modsOf(childNamed(runtime, 'mods'))
        let migrate: ImportPlanCounts = { files: 0, bytes: 0 }
        for (const child of runtime?.children ?? []) {
            if (child.name === 'mods') continue
            const counts = measure(child)
            migrate = { files: migrate.files + counts.files, bytes: migrate.bytes + counts.bytes }
        }
        const meta = pack?.meta?.type === 'mmc-pack' ? pack.meta : null
        return {
            gameVersion: meta?.gameVersion ?? null,
            loader: meta ? meta.loader : null,
            loaderVersion: meta?.loaderVersion ?? null,
            minecraftRoot: runtime && runtime !== node ? joinPath(path, runtime.name) : path,
            mods,
            migrate,
        }
    }

    function hasCachedGameFiles(gameVersion: string | null) {
        return (
            !!gameVersion &&
            preview.instances.instances.value.some(
                (instance) =>
                    instance.game_version === gameVersion && instance.install_stage === 'installed',
            )
        )
    }

    function planCounts(
        details: InstanceDetails,
        gameVersion: string | null,
        loader: string | null,
    ) {
        const cached = hasCachedGameFiles(gameVersion)
        const loaderFiles = loader && loader !== 'vanilla' ? LOADER_FILES : { files: 0, bytes: 0 }
        return {
            cache: cached ? GAME_FILES : { files: 0, bytes: 0 },
            network: cached
                ? loaderFiles
                : {
                      files: GAME_FILES.files + loaderFiles.files,
                      bytes: GAME_FILES.bytes + loaderFiles.bytes,
                  },
            local: {
                files: details.mods.length,
                bytes: details.mods.reduce((total, mod) => total + mod.size, 0),
            },
            migrate: details.migrate,
        }
    }

    const planListeners = new Set<(snapshot: ImportPlanSnapshot) => void>()
    const activePlans = new Map<string, () => void>()

    function listenImportPlan(callback: (snapshot: ImportPlanSnapshot) => void) {
        planListeners.add(callback)
        return () => planListeners.delete(callback)
    }

    function emitPlan(snapshot: ImportPlanSnapshot) {
        for (const listener of [...planListeners]) listener(snapshot)
    }

    /** `start_import_plan`: resolves the instance, then scans its files in stages. */
    function startImportPlan(request: ImportPlanRequest): string {
        let cancelled = false
        activePlans.set(request.requestId, () => {
            cancelled = true
        })
        void (async () => {
            const path = request.instancePath ?? ''
            const details = await describeInstance(path).catch(() => null)
            if (cancelled) return
            if (!details) {
                emitPlan({
                    requestId: request.requestId,
                    stage: 'error',
                    gameVersion: null,
                    loader: null,
                    loaderVersion: null,
                    importPath: path,
                    minecraftRoot: '',
                    modCount: 0,
                    cache: { files: 0, bytes: 0 },
                    local: { files: 0, bytes: 0 },
                    network: { files: 0, bytes: 0 },
                    migrate: { files: 0, bytes: 0 },
                    error: `${path}: No such file or directory (os error 2)`,
                })
                return
            }
            const gameVersion = request.gameVersion ?? details.gameVersion
            const loader = request.loader ?? details.loader
            const loaderVersion = request.loaderVersion ?? details.loaderVersion
            const zero = { files: 0, bytes: 0 }
            const base = {
                requestId: request.requestId,
                gameVersion,
                loader,
                loaderVersion,
                importPath: path,
                minecraftRoot: details.minecraftRoot,
            }
            await wait(220)
            if (cancelled) return
            emitPlan({
                ...base,
                stage: 'resolving',
                modCount: 0,
                cache: zero,
                local: zero,
                network: zero,
                migrate: zero,
            })
            const counts = planCounts(details, gameVersion, loader)
            await wait(420)
            if (cancelled) return
            emitPlan({
                ...base,
                stage: 'scanning',
                modCount: Math.ceil(details.mods.length / 2),
                cache: counts.cache,
                local: {
                    files: Math.ceil(counts.local.files / 2),
                    bytes: Math.round(counts.local.bytes / 2),
                },
                network: counts.network,
                migrate: zero,
            })
            await wait(560)
            if (cancelled) return
            activePlans.delete(request.requestId)
            emitPlan({ ...base, stage: 'done', modCount: details.mods.length, ...counts })
        })()
        return request.requestId
    }

    function cancelImportPlan(requestId: string) {
        activePlans.get(requestId)?.()
        activePlans.delete(requestId)
    }

    /** `getLoaderVersions` of the instance-import provider (launcher-meta builds). */
    async function getLoaderVersions(loader: string, gameVersion: string): Promise<string[]> {
        const manifests = await loadLoaderManifests()
        const versions = loaderVersionsForGameVersion(manifests[loader], gameVersion).map(
            (entry) => entry.id,
        )
        return [...new Set(versions)]
    }

    function openPath(path: string) {
        bindings?.folderPanel?.browse(path)
    }

    /** `import_instance` + `wait_for_install_job`. */
    async function importInstance(params: {
        launcherType: string
        basePath: string
        instanceFolder: string
        symlink: boolean
        instancePath?: string
        gameVersion?: string | null
        loader?: string | null
        loaderVersion?: string | null
        gameDirOverride?: string | null
    }): Promise<GameInstance> {
        const path = params.instancePath ?? joinPath(params.basePath, params.instanceFolder)
        const details = await describeInstance(path)
        await deps.ensureData()
        const gameVersion =
            params.gameVersion ||
            details.gameVersion ||
            deps.gameVersions.value.find((entry) => entry.version_type === 'release')?.version ||
            ''
        const loader = (params.loader || details.loader || 'vanilla') as InstanceLoader
        const rawLoaderVersion = params.loaderVersion ?? details.loaderVersion
        const loaderVersion =
            loader === 'vanilla' || !rawLoaderVersion || rawLoaderVersion === 'latest'
                ? null
                : rawLoaderVersion
        const created = preview.instances.create({
            name: params.instanceFolder,
            gameVersion,
            loader,
            loaderVersion,
            iconPath: null,
        })
        const instance = preview.instances.get(created.id) ?? created
        instance.install_stage = 'minecraft_installing'
        if (params.symlink) instance.symlink_target = path
        if (params.gameDirOverride) instance.path = params.gameDirOverride
        const counts = planCounts(details, gameVersion, loader)
        const copyBytes = params.symlink ? 0 : counts.local.bytes + counts.migrate.bytes
        const result = await runInstallJob(preview, {
            title: params.instanceFolder,
            icon: getLoaderInstanceIconUrl(loader),
            projectType: 'minecraft',
            instanceId: instance.id,
            instanceName: params.instanceFolder,
            versionName: gameVersion,
            fileName: `${gameVersion}.jar`,
            downloadBytes: counts.network.bytes,
            installMs: Math.min(2400, copyBytes / (64 * 1024)),
        })
        const current = preview.instances.get(instance.id)
        if (result !== 'completed') {
            if (current) current.install_stage = 'not_installed'
            throw new Error(`Install job ${instance.id} canceled`)
        }
        if (current) {
            current.install_stage = 'installed'
            preview.downloads.contents.value = {
                ...preview.downloads.contents.value,
                [instance.id]: contentItemsFor(details.mods),
            }
        }
        return instance
    }

    // ── Modpack files (`installModpackFromPath` of creation-modal.ts) ─────

    async function doInstallModpackFile(
        filePath: string,
        options: { existingNotify?: { id: string | number } } = {},
    ) {
        const installingNotify =
            options.existingNotify ??
            addNotification({
                title: formatMessage(messages.modpackInstalling),
                type: 'info',
                autoCloseMs: 1000 * 10,
            })
        let pack: ResolvedPack
        try {
            pack = await resolvePack(filePath)
        } catch (error) {
            notifications.removeNotification(installingNotify.id)
            notifications.handleError(error)
            return
        }
        const { info } = pack
        await deps.ensureData()
        // The shared content list is filled once for the existing instances; load it
        // first so the new instance's files are not added twice.
        await preview.downloads.ensureContents()
        const gameVersion =
            info.gameVersion ||
            deps.gameVersions.value.find((entry) => entry.version_type === 'release')?.version ||
            ''
        const loader = (info.loader || 'vanilla') as InstanceLoader
        const created = preview.instances.create({
            name: info.name,
            gameVersion,
            loader,
            loaderVersion: info.loaderVersion,
            iconPath: pack.icon,
            link: pack.link,
        })
        const instance = preview.instances.get(created.id) ?? created
        instance.install_stage = 'pack_installing'
        const downloadBytes =
            info.downloads.bytes + (hasCachedGameFiles(gameVersion) ? 0 : GAME_FILES.bytes)
        const result = await runInstallJob(preview, {
            title: info.name,
            icon: pack.icon ?? getLoaderInstanceIconUrl(loader),
            projectType: 'modpack',
            instanceId: instance.id,
            instanceName: info.name,
            versionName: info.versionNumber ?? gameVersion,
            fileName: baseName(filePath),
            downloadBytes,
            installMs: Math.min(2400, info.overridesBytes / (64 * 1024)),
        })
        const current = preview.instances.get(instance.id)
        notifications.removeNotification(installingNotify.id)
        if (result === 'completed') {
            if (current) {
                current.install_stage = 'installed'
                preview.downloads.contents.value = {
                    ...preview.downloads.contents.value,
                    [instance.id]: pack.contents,
                }
            }
            addNotification({ title: formatMessage(messages.modpackInstalled), type: 'success' })
        } else if (current) {
            current.install_stage = 'not_installed'
        }
    }

    interface ResolvedPack {
        info: LocalModpackInfo
        icon: string | null
        link: GameInstance['link']
        contents: ContentItem[]
    }

    /**
     * What the install reads from a pack file: a file the visitor picked is parsed in
     * the page; the Modrinth pack on the simulated disk is the snapshot's version
     * (`data/modpacks.json`) with its resolved files (`data/instance-content.json`).
     */
    async function resolvePack(filePath: string): Promise<ResolvedPack> {
        const file = pickedFiles.get(filePath)
        if (file) {
            const info = await readLocalModpack(file, baseName(filePath))
            return {
                info,
                icon: null,
                link: {
                    type: 'imported_modpack',
                    name: info.name,
                    version_number: info.versionNumber,
                    filename: baseName(filePath),
                },
                contents: [],
            }
        }
        const root = await loadMacFileSystem()
        const node = findNode(root, filePath)
        if (!node) throw new Error(`${filePath}: No such file or directory (os error 2)`)
        if (node.meta?.type !== 'mrpack') throw new Error('File is not a valid ZIP archive')
        const { projectId, versionId } = node.meta
        const version = modpackSnapshot.packs.find(
            (entry) => entry.project_id === projectId && entry.version_id === versionId,
        )
        if (!version) throw new Error(`${filePath}: No such file or directory (os error 2)`)
        const content = await loadInstanceContent()
        const withoutIcon = new Set(content.withoutIcon)
        const rows = content.instances[projectId] ?? []
        const now = new Date().toISOString()
        return {
            info: {
                name: version.pack_name,
                versionNumber: version.version_number,
                gameVersion: version.game_version,
                loader: version.loader,
                loaderVersion: version.loader_version,
                downloads: {
                    files: rows.filter(([, , , rowProject]) => rowProject).length,
                    bytes: rows
                        .filter(([, , , rowProject]) => rowProject)
                        .reduce((total, [, , size]) => total + size, 0),
                },
                overridesBytes: rows
                    .filter(([, , , rowProject]) => !rowProject)
                    .reduce((total, [, , size]) => total + size, 0),
            },
            icon: projectIcon(projectId),
            link: { type: 'modrinth_modpack', project_id: projectId, version_id: versionId },
            contents: rows.map(
                ([
                    fileName,
                    folder,
                    size,
                    rowProject,
                    rowVersion,
                    versionNumber,
                    title,
                    projectType,
                ]) => ({
                    id: `${folder}/${fileName}`,
                    project_id: rowProject,
                    version_id: rowVersion,
                    name: title ?? fileName,
                    file_name: fileName,
                    enabled: true,
                    icon:
                        rowProject && !withoutIcon.has(rowProject) ? projectIcon(rowProject) : null,
                    version_number: versionNumber,
                    project_type: projectType,
                    size,
                    installed_at: now,
                }),
            ),
        }
    }

    async function installModpackFromPath(
        filePath: string,
        fileName: string,
        options: { persistUntilDone?: boolean } = {},
    ) {
        const persistUntilDone = options.persistUntilDone === true
        const currentNotify = addNotification({
            title: formatMessage(messages.installingFile, { name: fileName }),
            type: 'info',
            autoCloseMs: persistUntilDone ? null : 1000 * 10,
        })
        try {
            const isMrpack = fileName.toLowerCase().endsWith('.mrpack')
            if (!isMrpack) {
                // `install_get_modpack_preview` looks the file up on Modrinth; a local
                // archive the visitor picked is not a reviewed Modrinth file.
                const file = pickedFiles.get(filePath)
                if (!file) throw new Error(`${filePath}: No such file or directory (os error 2)`)
                await openZip(file)
                notifications.removeNotification(currentNotify.id)
                await new Promise<void>((resolve) => {
                    if (!bindings?.unknownPack) {
                        resolve()
                        return
                    }
                    bindings.unknownPack.show(async () => {
                        resolve()
                        if (persistUntilDone) {
                            const installingNotify = addNotification({
                                title: formatMessage(messages.modpackInstalling),
                                type: 'info',
                                autoCloseMs: null,
                            })
                            await doInstallModpackFile(filePath, {
                                existingNotify: installingNotify,
                            })
                        } else {
                            await doInstallModpackFile(filePath)
                        }
                    }, fileName)
                    unknownPackDismissed = resolve
                })
                unknownPackDismissed = null
                return
            }
            if (persistUntilDone)
                await doInstallModpackFile(filePath, { existingNotify: currentNotify })
            else await doInstallModpackFile(filePath)
        } catch (error) {
            notifications.removeNotification(currentNotify.id)
            notifications.handleError(error)
        }
    }

    /** Resolves the pending install-anyway prompt when the warning closes. */
    let unknownPackDismissed: (() => void) | null = null
    function onUnknownPackClosed() {
        unknownPackDismissed?.()
    }

    // ── Entry points: the picker (`onImportFileReceived` of App.vue) ──────

    async function onImportFileReceived(payload: {
        filePath: string | null
        source: 'file-picker' | 'drag-drop'
    }) {
        const filePath = payload.filePath
        if (!filePath) return
        const fileName = filePath.split(/[/\\]/).pop() || 'file'

        deps.hideCreationModal()

        const processingNotify = addNotification({
            title: formatMessage(messages.processing, { name: fileName }),
            type: 'info',
            autoCloseMs: null,
        })

        try {
            await wait(380)
            const classification = await classifyPath(filePath)
            clearDropProcessingNotification()
            notifications.removeNotification(processingNotify.id)

            dropClassification.value = classification
            dropFilePath.value = classification.file_path ?? classification.base_path ?? filePath
            dropFileName.value = fileName

            if (
                classification.item_type === 'unknown' &&
                classification.reason?.toLowerCase().includes('nested')
            ) {
                showNestedUnpackPrompt(classification)
                return
            }
            if (
                classification.item_type === 'unknown' &&
                classification.reason?.toLowerCase().includes('extraction')
            ) {
                showForceAnalysisPrompt(classification)
                return
            }
            if (classification.item_type === 'unknown') {
                addNotification({
                    title: formatMessage(messages.unknownTitle),
                    text: unknownReasonMessage(classification.reason),
                    type: 'error',
                })
                return
            }
            bindings?.confirmDrop?.show()
        } catch (error) {
            notifications.removeNotification(processingNotify.id)
            addNotification({
                title: formatMessage(messages.processFailedTitle),
                text: error instanceof Error ? error.message : String(error),
                type: 'error',
            })
        }
    }

    /** "Select file to import": the browser's file picker stands in for the native one. */
    async function pickFile() {
        const picked = await bindings?.filePicker?.pick()
        if (!picked?.path) return
        pickedFiles.set(picked.path, picked.file)
        await onImportFileReceived({ filePath: picked.path, source: 'file-picker' })
    }

    /** "Select folder to import": the simulated macOS open panel. */
    async function pickFolder() {
        const path = await bindings?.folderPanel?.pick()
        if (!path) return
        await onImportFileReceived({ filePath: path, source: 'file-picker' })
    }

    function showForceAnalysisPrompt(classification: ClassificationResult) {
        const filePath = dropFilePath.value
        if (!filePath || !classification) return
        popups.addPopupNotification({
            title: formatMessage(messages.forceAnalysisTitle),
            text: formatMessage(messages.forceAnalysisText),
            type: 'info',
            autoCloseMs: null,
            buttons: [
                {
                    label: formatMessage(messages.forceAnalysisButton),
                    action: async () => {
                        const analyzing = addNotification({
                            title: formatMessage(messages.forceAnalyzing),
                            type: 'info',
                            autoCloseMs: null,
                        })
                        try {
                            await wait(900)
                            const file = pickedFiles.get(filePath)
                            const result = file
                                ? await classifyZipWithExtraction(file, filePath)
                                : ({
                                      item_type: 'unknown',
                                      reason: 'Cannot open ZIP file',
                                  } as ClassificationResult)
                            notifications.removeNotification(analyzing.id)
                            if (result.item_type === 'unknown') {
                                addNotification({
                                    title: formatMessage(messages.forceAnalysisFailedTitle),
                                    text: formatMessage(messages.forceAnalysisFailedText),
                                    type: 'error',
                                })
                                return
                            }
                            await continueWithClassification(result, filePath)
                        } catch (error) {
                            notifications.removeNotification(analyzing.id)
                            addNotification({
                                title: formatMessage(messages.forceAnalysisFailedTitle),
                                text: error instanceof Error ? error.message : String(error),
                                type: 'error',
                            })
                        }
                    },
                    color: 'brand',
                },
            ],
        })
    }

    function showNestedUnpackPrompt(classification: ClassificationResult) {
        const filePath = dropFilePath.value
        if (!filePath) return
        const sizeBytes = Number(classification.reason?.match(/total (\d+) bytes/i)?.[1] ?? 0)
        const sizeLabel =
            sizeBytes > 0
                ? sizeBytes >= 1024 * 1024
                    ? `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`
                    : `${Math.max(1, Math.round(sizeBytes / 1024))} KB`
                : '?'
        popups.addPopupNotification({
            title: formatMessage(messages.nestedUnpackTitle),
            text: formatMessage(messages.nestedUnpackText, { size: sizeLabel }),
            type: 'info',
            autoCloseMs: null,
            buttons: [
                {
                    label: formatMessage(messages.nestedUnpackButton),
                    action: async () => {
                        try {
                            const result = await classifyPath(filePath, true)
                            await continueWithClassification(result, dropFileName.value || 'file')
                        } catch (error) {
                            addNotification({
                                title: formatMessage(messages.processFailedTitle),
                                text: error instanceof Error ? error.message : String(error),
                                type: 'error',
                            })
                        }
                    },
                },
            ],
        })
    }

    async function continueWithClassification(
        result: ClassificationResult,
        fallbackFileName: string,
    ) {
        if (result.item_type === 'unknown') {
            addNotification({
                title: formatMessage(messages.unknownTitle),
                text: unknownReasonMessage(result.reason),
                type: 'error',
            })
            return
        }
        dropClassification.value = result
        dropFilePath.value = result.file_path ?? result.base_path ?? ''
        dropFileName.value =
            result.file_path?.split(/[/\\]/).pop() ??
            result.base_path?.split(/[/\\]/).pop() ??
            fallbackFileName
        switch (result.item_type) {
            case 'modpack':
                await handleDropConfirm('modpack')
                break
            case 'world_save':
                await handleDropConfirm('world_save')
                break
            case 'launcher':
            case 'hmcl_launcher':
                await handleDropConfirm('instance')
                break
            default:
                if (result.item_type === 'resource_pack' || result.item_type === 'multiple') {
                    bindings?.confirmDrop?.show()
                    return
                }
                await handleDropConfirm(result.item_type)
        }
    }

    // ── Confirm modal handlers ────────────────────────────────────────────

    function handleDropCancel() {
        clearDropProcessingNotification()
        dropClassification.value = null
    }

    async function handleConfirmDropConfirm(type: string, innerBase?: string) {
        await handleDropConfirm(type, innerBase)
    }

    function handleConfirmDropCancel() {
        handleDropCancel()
    }

    async function handleConfirmDropHelp() {
        await router?.push('/help/drop')
        bindings?.confirmDrop?.hide()
    }

    async function handleDropConfirm(type: string, innerBase?: string) {
        const classification = dropClassification.value
        dropClassification.value = null
        bindings?.confirmDrop?.hide()

        const isLauncherImport =
            classification?.item_type === 'launcher' ||
            classification?.item_type === 'hmcl_launcher'
        if (!isLauncherImport && !classification?.file_path && !dropFilePath.value) return

        const filePath = classification?.file_path ?? dropFilePath.value
        const fileName =
            filePath?.split(/[/\\]/).pop() ??
            classification?.base_path?.split(/[/\\]/).pop() ??
            'file'

        if (type === 'dot_minecraft') {
            await routeToGamedirFlow([dropFilePath.value ?? ''])
            return
        }

        if (isLauncherImport && type === 'instance') {
            const launcherType =
                classification!.item_type === 'hmcl_launcher'
                    ? 'HMCL'
                    : classification!.launcher_type!
            const basePath =
                classification!.item_type === 'hmcl_launcher'
                    ? classification!.launcher_dir!
                    : classification!.base_path!
            if (['Generic', 'PCL2', 'PCL2CE', 'HMCL'].includes(launcherType)) {
                try {
                    const resolved = await resolveGamedirs(
                        launcherType,
                        classification!.base_path ?? classification!.launcher_dir ?? filePath!,
                    )
                    if (resolved.length === 0) throw new Error(formatMessage(messages.noInstances))
                    await routeToGamedirFlow(resolved.map((entry) => entry.path))
                } catch (error) {
                    addNotification({
                        title: formatMessage(messages.scanFailed),
                        text: error instanceof Error ? error.message : String(error),
                        type: 'error',
                    })
                }
                return
            }

            let scanBasePath = basePath
            if (isZipPath(basePath)) {
                scanningInstances.value = true
                try {
                    const file = pickedFiles.get(basePath)
                    if (!file)
                        throw new Error(`${basePath}: No such file or directory (os error 2)`)
                    const archive = await openZip(file)
                    await wait(700)
                    const tempDir = joinPath(
                        TEMP_DIR,
                        `axolotl-drop-${Math.random().toString(36).slice(2, 10)}`,
                    )
                    zipMounts.set(tempDir, archive)
                    launcherZipTempDir = tempDir
                    scanBasePath = classification!.innerBase
                        ? joinPath(tempDir, classification!.innerBase)
                        : tempDir
                } catch (error) {
                    launcherZipTempDir = null
                    addNotification({
                        title: formatMessage(messages.extractFailed),
                        text: error instanceof Error ? error.message : String(error),
                        type: 'error',
                    })
                    return
                } finally {
                    scanningInstances.value = false
                }
            }

            currentImportContext = { launcherType, basePath: scanBasePath }
            scanningInstances.value = true
            let results: ScanResult[]
            try {
                results = await scanLauncherInstances(launcherType, scanBasePath)
            } catch {
                currentImportContext = null
                addNotification({ title: formatMessage(messages.scanFailed), type: 'error' })
                cleanupLauncherZipTemp()
                return
            } finally {
                scanningInstances.value = false
            }
            const totalInstances = results.reduce((sum, result) => sum + result.instances.length, 0)
            if (totalInstances === 0) {
                currentImportContext = null
                addNotification({ title: formatMessage(messages.noInstances), type: 'warning' })
                cleanupLauncherZipTemp()
                return
            }
            if (totalInstances === 1 && results[0]?.instances[0]) {
                const single = results[0].instances[0]
                selectedInstances.value = [
                    {
                        launcherType,
                        basePath: single.compatibleMode ? single.path : scanBasePath,
                        name: single.name,
                        path: single.compatibleMode
                            ? (single.versionPath ?? single.path)
                            : single.path,
                        compatibleMode: single.compatibleMode,
                        versionPath: single.versionPath,
                    },
                ]
                if (launcherZipTempDir) {
                    await onSymlinkMethodConfirmed(false)
                    return
                }
                bindings?.symlinkCards?.show({
                    instances: [
                        {
                            name: single.name,
                            path: single.compatibleMode
                                ? (single.versionPath ?? single.path)
                                : single.path,
                            launcherType,
                            basePath: single.compatibleMode ? single.path : scanBasePath,
                            compatibleMode: single.compatibleMode,
                            versionPath: single.versionPath,
                        },
                    ],
                    symlinkCapable: checkSymlinkCapability(),
                })
                return
            }
            bindings?.launcherImport?.show(results)
            return
        }

        if (type === 'modpack') {
            if (!filePath) {
                addNotification({
                    title: formatMessage(messages.modpackInstallFailed),
                    type: 'error',
                })
                return
            }
            clearDropProcessingNotification()
            await installModpackFromPath(filePath, fileName, { persistUntilDone: true })
            await router?.push('/library')
            return
        }

        const contentTypes = [
            'mod',
            'resource_pack',
            'data_pack',
            'shader_pack',
            'world_save',
            'litematic',
            'schematic',
        ]
        if (!contentTypes.includes(type)) return

        pendingInstall = { type, filePath, innerBase }
        const instances: GenericInstallTarget[] = preview.instances.instances.value.map(
            (instance) => ({
                id: instance.id,
                name: instance.name,
                iconUrl: instance.icon_path || getLoaderInstanceIconUrl(instance.loader),
                gameVersion: instance.game_version || null,
                loader: instance.loader || null,
            }),
        )
        bindings?.genericInstall?.show({ contentType: type, fileName, instances })
    }

    /** `check_symlink_capability`: macOS creates symbolic links without elevation. */
    function checkSymlinkCapability(): SymlinkCapability {
        return 'supported'
    }

    function cleanupLauncherZipTemp() {
        if (!launcherZipTempDir) return
        zipMounts.delete(launcherZipTempDir)
        launcherZipTempDir = null
    }

    // ── Generic content install (`installContentDirectly`) ────────────────

    async function handleGenericInstall(targetInstanceId: string) {
        bindings?.genericInstall?.hide()
        const pending = pendingInstall
        pendingInstall = null
        if (!pending) return
        await installContentDirectly(pending.type, pending.filePath, targetInstanceId)
    }

    function handleGenericInstallCancel() {
        dropClassification.value = null
    }

    async function handleGenericInstallNavigateCreate() {
        await router?.push('/create')
    }

    async function installContentDirectly(type: string, filePath: string, instanceId: string) {
        try {
            await wait(450)
            const source = pickedFiles.get(filePath) ?? null
            let size = source?.size ?? 0
            if (!source) {
                const root = await loadMacFileSystem()
                const node = findNode(root, filePath)
                if (!node) throw new Error(`${filePath}: No such file or directory (os error 2)`)
                size = measure(node).bytes
            }
            if (type === 'world_save') {
                addNotification({
                    title: formatMessage(messages.worldImportedTitle),
                    text: formatMessage(messages.worldImportedText),
                    type: 'success',
                })
                return
            }
            const fileName = baseName(filePath)
            const items = preview.downloads.contents.value[instanceId] ?? []
            preview.downloads.contents.value = {
                ...preview.downloads.contents.value,
                [instanceId]: [
                    {
                        id: `${contentFolders[type] ?? 'files'}/${fileName}`,
                        project_id: null,
                        version_id: null,
                        name: fileName.replace(/\.(jar|zip|litematic|schematic)$/i, ''),
                        file_name: fileName,
                        enabled: true,
                        icon: null,
                        version_number: null,
                        project_type: contentProjectTypes[type] ?? type,
                        size,
                        installed_at: new Date().toISOString(),
                    },
                    ...items.filter((item) => item.file_name !== fileName),
                ],
            }
            addNotification({
                title: formatMessage(messages.contentInstalledTitle),
                text: formatMessage(messages.contentInstalledText),
                type: 'success',
            })
        } catch (error) {
            addNotification({
                title: formatMessage(messages.installFailedTitle),
                text: error instanceof Error ? error.message : String(error),
                type: 'error',
            })
        }
    }

    // ── Launcher import handlers ──────────────────────────────────────────

    function onLauncherImportCancelled() {
        bindings?.launcherImport?.hide()
        cleanupLauncherZipTemp()
    }

    async function onImportSelected(selections: LauncherSelection[]) {
        const allSelected: typeof selectedInstances.value = []
        for (const selection of selections) {
            for (const instance of selection.instances) {
                allSelected.push({
                    launcherType: selection.launcherType,
                    basePath: instance.compatibleMode ? instance.path : '',
                    name: instance.name,
                    path: instance.compatibleMode
                        ? (instance.versionPath ?? instance.path)
                        : instance.path,
                    compatibleMode: instance.compatibleMode,
                    versionPath: instance.versionPath,
                })
            }
        }
        if (allSelected.length === 0) return
        selectedInstances.value = allSelected
        if (launcherZipTempDir) {
            await onSymlinkMethodConfirmed(false)
            return
        }
        bindings?.symlinkCards?.show({
            instances: allSelected.map((instance) => ({
                name: instance.name,
                path: instance.path,
                launcherType: instance.launcherType,
                basePath: instance.basePath || instance.path,
                compatibleMode: instance.compatibleMode,
                versionPath: instance.versionPath,
            })),
            symlinkCapable: checkSymlinkCapability(),
        })
    }

    function onSymlinkMethodCancelled() {
        bindings?.symlinkCards?.hide()
        cleanupLauncherZipTemp()
    }

    async function onSymlinkMethodConfirmed(choices: SymlinkMethodChoice[] | boolean) {
        const instances = selectedInstances.value
        selectedInstances.value = []
        const context = currentImportContext
        currentImportContext = null
        if (instances.length === 0) {
            cleanupLauncherZipTemp()
            return
        }

        const choiceFor = (instance: (typeof instances)[number]) =>
            Array.isArray(choices)
                ? choices.find(
                      (choice) =>
                          choice.instanceName === instance.name &&
                          (choice.instancePath ?? undefined) === (instance.path ?? undefined),
                  )
                : undefined

        const runImport = (instance: (typeof instances)[number]) => {
            const choice = choiceFor(instance)
            return importInstance({
                launcherType: context?.launcherType ?? instance.launcherType,
                basePath: instance.compatibleMode
                    ? instance.basePath
                    : (context?.basePath ?? instance.path),
                instanceFolder: instance.name,
                symlink: choice?.symlink ?? (Array.isArray(choices) ? false : choices),
                instancePath: instance.compatibleMode ? instance.versionPath : instance.path,
                gameVersion: instance.compatibleMode
                    ? undefined
                    : (choice?.gameVersion ?? undefined),
                loader: instance.compatibleMode ? undefined : (choice?.loader ?? undefined),
                loaderVersion: instance.compatibleMode
                    ? undefined
                    : (choice?.loaderVersion ?? undefined),
                gameDirOverride: choice?.gameDirOverride ?? null,
            })
        }

        if (instances.length === 1) {
            const instance = instances[0]
            try {
                await runImport(instance)
                addNotification({
                    title: formatMessage(messages.instanceImportedTitle),
                    text: formatMessage(messages.instanceImportedText, { name: instance.name }),
                    type: 'success',
                })
            } catch (error) {
                addNotification({
                    title: formatMessage(messages.importFailedTitle),
                    text: formatMessage(messages.importFailedText, {
                        name: instance.name,
                        error: String(error),
                    }),
                    type: 'error',
                })
            } finally {
                cleanupLauncherZipTemp()
            }
            return
        }

        const total = instances.length
        let completed = 0
        let failedCount = 0
        let progress = addNotification({
            title: formatMessage(messages.importProgressTitle),
            text: formatMessage(messages.importProgressText, { current: 0, total }),
            type: 'info',
            autoCloseMs: null,
        })
        for (let index = 0; index < instances.length; index++) {
            const instance = instances[index]
            notifications.removeNotification(progress.id)
            progress = addNotification({
                title: formatMessage(messages.importProgressTitle),
                text: formatMessage(messages.importProgressText, { current: index + 1, total }),
                type: 'info',
                autoCloseMs: null,
            })
            try {
                await runImport(instance)
                completed++
            } catch (error) {
                failedCount++
                addNotification({
                    title: formatMessage(messages.importFailedTitle),
                    text: formatMessage(messages.importFailedText, {
                        name: instance.name,
                        error: String(error),
                    }),
                    type: 'error',
                })
            }
        }
        cleanupLauncherZipTemp()
        notifications.removeNotification(progress.id)
        if (failedCount === 0) {
            addNotification({
                title: formatMessage(messages.importCompletedTitle),
                text: formatMessage(messages.importCompletedText, { count: total }),
                type: 'success',
            })
        } else {
            addNotification({
                title: formatMessage(messages.importCompletedTitle),
                text: formatMessage(messages.importCompletedPartialText, {
                    completed,
                    failed: failedCount,
                    total,
                }),
                type: 'warning',
            })
        }
    }

    function dispose() {
        for (const cancel of activePlans.values()) cancel()
        activePlans.clear()
        planListeners.clear()
        pickedFiles.clear()
        zipMounts.clear()
    }

    return {
        dropClassification,
        dropFileName,
        scanningInstances,
        bind,
        unbind,
        pickFile,
        pickFolder,
        onImportFileReceived,
        handleConfirmDropConfirm,
        handleConfirmDropCancel,
        handleConfirmDropHelp,
        onLauncherImportCancelled,
        onImportSelected,
        onSymlinkMethodConfirmed,
        onSymlinkMethodCancelled,
        handleGenericInstall,
        handleGenericInstallCancel,
        handleGenericInstallNavigateCreate,
        onUnknownPackClosed,
        startImportPlan,
        cancelImportPlan,
        listenImportPlan,
        getLoaderVersions,
        openPath,
        gameVersions: deps.gameVersions,
        ensureData: deps.ensureData,
        homePath: HOME,
        dispose,
    }
}

export type ImportFlow = ReturnType<typeof createImportFlow>
