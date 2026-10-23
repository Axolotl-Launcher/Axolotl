import { usePreview } from '../../../lib/state'
import type { ContentItem } from '../../../lib/state/downloads'
import { contentFolderFor } from '../../../lib/state/downloads'
import { useHomeState } from '../../home/state'
import { type FsDir, useInstanceFiles } from '../files/instance-files'

/*
 * The app's `resource_export_*` commands (`packages/app-lib/src/resources/export`)
 * over the preview's instance folder: the scan sorts the files the Files tab
 * shows into the app's categories, and the check matches them against the
 * Modrinth releases the instance's content came from.
 */

export type SourceId = 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'

export interface ResourceArtifact {
    reference: {
        release: { project: { source: SourceId; projectId: string }; releaseId: string }
        artifactId: string
    }
    fileName: string | null
    role: string
    integrity: { size: number | null; hashes: { algorithm: string; value: string }[] }
}

export interface ExportFileSnapshot {
    relativePath: string
    integrity: ResourceArtifact['integrity']
    revision: number
    identifiable: boolean
    modifiedAt: number | null
    categoryId: string
    categorySelected: boolean
}

export interface ExportCategory {
    id: string
    defaultSelected: boolean
    paths: string[]
    children: ExportCategory[]
    title: string | null
    modifiedAt: number | null
}

export interface ResourceExportScan {
    sessionId: string
    instanceId: string
    instanceRevision: number
    files: ExportFileSnapshot[]
    directories: { relativePath: string; modifiedAt: number | null }[]
    categories: ExportCategory[]
    excludedPaths: string[]
    requirements: { gameVersion: string; loader: string | null; loaderVersion: string | null }
}

export type ExportSourceResolution =
    | { kind: 'matched'; source: SourceId; artifacts: ResourceArtifact[] }
    | { kind: 'noMatch'; source: SourceId }
    | { kind: 'failed'; source: SourceId; code: string; message: string }

export interface ExportResolvedFile {
    snapshot: ExportFileSnapshot
    resolutions: ExportSourceResolution[]
    localError: string | null
}

export interface ExportResolutionSession {
    id: string
    requestId: string
    selectionRevision: number
    instanceRevision: number
    state: 'refreshing' | 'ready' | 'needsDecision' | 'canceled'
    files: ExportResolvedFile[]
}

export interface ExportRefreshProgress {
    sessionId: string
    requestId: string
    selectionRevision: number
    completed: number
    total: number
    phase: string
    overallCompleted?: number
    overallTotal?: number
    currentFileFraction?: number
}

export interface ExportEmbeddedFailureChoice {
    snapshot: ExportFileSnapshot
    source: SourceId
}

export type ResourceExportFormat = 'modrinth' | 'genericZip' | 'curseForge'

/** `resourceSourceName` of `helpers/resource-operation-logic`. */
export function resourceSourceName(source: string): string {
    switch (source) {
        case 'modrinth':
            return 'Modrinth'
        case 'curseforge':
            return 'CurseForge'
        case 'mcarchive':
            return 'MCArchive'
        case 'planet_minecraft':
            return 'Planet Minecraft'
        default:
            return source
    }
}

/* ------------------------------------------------------------ categories.rs */

type Rule = {
    id: string
    selected: boolean
    paths: string[]
    excluded?: string[]
    included?: string[]
}

const RULES: Rule[] = [
    { id: 'gameSettings', selected: true, paths: ['options.txt', 'configureddefaults/'] },
    { id: 'personalRecords', selected: false, paths: ['hotbar.nbt', 'command_history.txt'] },
    { id: 'optifine', selected: true, paths: ['optionsof.txt', 'optionsshaders.txt'] },
    {
        id: 'oldResources',
        selected: false,
        paths: [
            'mods/**/*.disabled',
            'mods/**/*.old',
            'coremods/**/*.disabled',
            'coremods/**/*.old',
            'lib/**/*.disabled',
            'lib/**/*.old',
            'plugins/**/*.disabled',
            'plugins/**/*.old',
        ],
    },
    {
        id: 'mapRecords',
        selected: false,
        paths: ['journeymap/data/', 'xaero/', 'XaeroWaypoints/', 'XaeroWorldMap/'],
    },
    { id: 'jeiRecords', selected: false, paths: ['config/jei/world/'] },
    { id: 'emiRecords', selected: false, paths: ['emi.json'] },
    { id: 'patchouliRecords', selected: false, paths: ['patchouli_data.json'] },
    {
        id: 'mods',
        selected: true,
        paths: ['mods/', 'coremods/', 'lib/', 'plugins/'],
        excluded: ['mods/.connector/', 'mods/mcef-libraries/', 'mods/mcef-cache/'],
    },
    {
        id: 'modConfig',
        selected: true,
        paths: [
            'config/',
            'journeymap/config/',
            'journeymap/server/',
            'TrashSlotSaveState.json',
            'customfov.txt',
            'gg.essential.mod/',
            'essential/',
            'paragliderSettings.nbt',
            'local/client_config.json',
            'local/ftbl.json',
            'local/client/sidebar_buttons.json',
            'local/client/ftbutilities.cfg',
            'local/client/ftblib.cfg',
            'local/client/xencraft.cfg',
            'liteloader.properties',
            'default_reference.xml',
            'CustomSkinLoader/CustomSkinLoader.json',
        ],
        excluded: [
            'config/accountsx/',
            'config/jei/world/',
            'config/worldedit/',
            'config/spark/',
            'essential/*/',
            'essential/*.jar*',
            'essential/screenshot-checksum-caches.json',
        ],
        included: ['config/worldedit/worldedit.properties', 'config/spark/config.json'],
    },
    { id: 'defaultConfig', selected: true, paths: ['defaultconfigs/'] },
    {
        id: 'importantData',
        selected: true,
        paths: [
            'addons/',
            'multiblocked/',
            'modpack-update-checker/',
            'global_packs/',
            'global_resource_packs/',
            'global_data_packs/',
            'optional_data_packs/',
            'maps/',
            'icon.png',
            'mods-resourcepacks/',
            'matmos/',
            'resource_assorts/',
            'resource_assorts.json',
            'patchouli_books/',
            'datapacks/',
            'kubejs*/',
            'openloader/',
            'worldshape/',
            'resources/',
            'scripts/',
            'structures/',
            'fontfiles/',
            'oresources/',
            'packmenu/',
            'craftpresence/',
            'pointblanks/',
            'template*/',
        ],
        excluded: [
            'kubejs*/probe/',
            'kubejs*/exported/',
            'kubejs*/jsconfig.json',
            'kubejs*/README.txt',
            'template*/playerdata/',
            'template*/stats/',
        ],
    },
    { id: 'resourcePacks', selected: true, paths: ['resourcepacks/', 'texturepacks/'] },
    { id: 'shaders', selected: true, paths: ['shaderpacks/'] },
    { id: 'worlds', selected: false, paths: ['saves/'] },
    { id: 'screenshots', selected: false, paths: ['screenshots/'] },
    { id: 'schematics', selected: false, paths: ['schematics/'] },
    { id: 'recordings', selected: false, paths: ['replay_recordings/', 'replay_videos/'] },
    { id: 'servers', selected: false, paths: ['servers.dat'] },
]

const FORBIDDEN = [
    '**/.axolotl-export-*.tmp',
    'assets/',
    'versions/',
    'libraries/',
    '*-natives/',
    'structureCacheV1/',
    '.fabric/',
    '.git/',
    'avatar-cache/',
    'cosmetic-cache/',
    'PCL/',
    '__MACOSX/',
    'modrinth_logs/',
    'launcher_logs/',
    '**/*.log',
    '**/*.dat_old',
    '**/*.BakaCoreInfo',
    '**/.DS_Store*',
    'hmclversion.cfg',
    'log4j2.xml',
    'profile.json',
    'essential/microsoft_accounts.json',
    'essential/microsoft_accounts.json*',
    'config/accountsx/',
    'accounts.json',
    'launcher_accounts.json',
    'launcher_profiles.json',
    '.axolotl/',
    '.theseus/',
]

const escapeRegex = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** `compile`: a `/`-separated glob where `*` stays inside a segment and `**` spans folders. */
function compile(pattern: string): string {
    const directory = pattern.endsWith('/')
    const segments = pattern.replace(/\/+$/, '').split('/')
    let expression = '^'
    segments.forEach((segment, index) => {
        if (segment === '**') {
            expression += '(?:[^/]+/)*'
            return
        }
        expression += segment.split('*').map(escapeRegex).join('[^/]*')
        if (index + 1 < segments.length) expression += '/'
    })
    if (directory) expression += '(?:/.*)?'
    return `${expression}$`
}

/** `file_pattern`: a folder pattern matches the files below it. */
function filePattern(pattern: string): RegExp {
    const expression = compile(pattern)
    return new RegExp(pattern.endsWith('/') ? expression.replace('(?:/.*)?$', '/.+$') : expression)
}

const COMPILED = RULES.map((rule) => ({
    rule,
    paths: rule.paths.map(filePattern),
    excluded: (rule.excluded ?? []).map(filePattern),
    included: (rule.included ?? []).map(filePattern),
}))
const BLOCKED = FORBIDDEN.map((pattern) => new RegExp(compile(pattern), 'i'))

function forbidden(path: string) {
    return BLOCKED.some((pattern) => pattern.test(path))
}

function classify(path: string): [string, boolean] {
    const match = COMPILED.find((rule) => rule.paths.some((pattern) => pattern.test(path)))
    if (!match) return ['other', true]
    return [
        match.rule.id,
        !match.excluded.some((pattern) => pattern.test(path)) ||
            match.included.some((pattern) => pattern.test(path)),
    ]
}

function categories(): ExportCategory[] {
    return [
        ...RULES.map((rule) => ({
            id: rule.id,
            defaultSelected: rule.selected,
            paths: [...rule.paths],
            children: [],
            title: null,
            modifiedAt: null,
        })),
        {
            id: 'other',
            defaultSelected: false,
            paths: [],
            children: [],
            title: null,
            modifiedAt: null,
        },
    ]
}

/* -------------------------------------------------------------------- scan */

const IDENTIFIABLE = new Set(['jar', 'zip', 'rar', 'disabled', 'old'])

function canceled() {
    return new Error('resource request canceled')
}

function compare(a: string, b: string) {
    return a < b ? -1 : a > b ? 1 : 0
}

type ExportSession = {
    scan: ResourceExportScan
    /** Modrinth release of each installed file, as the instance's content records it. */
    known: Map<string, { projectId: string; versionId: string; fileName: string; size: number }>
    results: Map<string, ExportResolvedFile>
    selected: string[]
    selectionRevision: number
}

/** The export sessions of this window (`ResourceService` drafts in the app). */
export function useExportBackend() {
    const preview = usePreview()
    const home = useHomeState()
    const { treeFor } = useInstanceFiles()
    const sessions = new Map<string, ExportSession>()

    function wait(ms: number, signal?: AbortSignal) {
        return new Promise<void>((resolve, reject) => {
            if (signal?.aborted) {
                reject(canceled())
                return
            }
            const cancel = preview.clock.after(ms, () => {
                signal?.removeEventListener('abort', onAbort)
                resolve()
            })
            function onAbort() {
                cancel()
                reject(canceled())
            }
            signal?.addEventListener('abort', onAbort, { once: true })
        })
    }

    function knownReferences(instanceId: string) {
        const known: ExportSession['known'] = new Map()
        for (const item of (preview.downloads.contents.value[instanceId] ?? []) as ContentItem[]) {
            if (
                !item.project_id ||
                !item.version_id ||
                item.project_id.startsWith('local:') ||
                item.removed
            )
                continue
            const folder = item.folder ?? contentFolderFor(item.project_type)
            const reference = {
                projectId: item.project_id,
                versionId: item.version_id,
                fileName: item.file_name,
                size: item.size,
            }
            known.set(`${folder}/${item.file_name}`, reference)
            known.set(`${folder}/${item.file_name}.disabled`, reference)
        }
        return known
    }

    /** `resource_export_scan`: the instance folder, sorted into the export categories. */
    async function resourceExportScan(
        instanceId: string,
        signal?: AbortSignal,
    ): Promise<ResourceExportScan> {
        const instance = preview.instances.get(instanceId)
        if (!instance) throw new Error('unknown instance')
        await preview.downloads.ensureContents()
        const worlds = await home.getInstanceWorlds(instanceId, true)
        const root: FsDir = treeFor(instance)

        const files: ExportFileSnapshot[] = []
        const directories: ResourceExportScan['directories'] = []
        const excluded: string[] = []
        const pending: [FsDir, string][] = [[root, '']]
        let visited = 0
        while (pending.length) {
            const [directory, relative] = pending.pop()!
            for (const entry of directory.children ?? []) {
                visited += 1
                const path = relative ? `${relative}/${entry.name}` : entry.name
                if (forbidden(path)) {
                    excluded.push(path)
                    continue
                }
                if (entry.kind === 'dir') {
                    directories.push({ relativePath: path, modifiedAt: entry.modified })
                    pending.push([entry, path])
                    continue
                }
                const [categoryId, categorySelected] = classify(path)
                files.push({
                    relativePath: path,
                    integrity: { size: entry.size, hashes: [] },
                    revision: 1,
                    identifiable: IDENTIFIABLE.has(path.split('.').pop()?.toLowerCase() ?? ''),
                    modifiedAt: entry.modified,
                    categoryId,
                    categorySelected,
                })
            }
        }
        // Reading the folder takes longer the more entries it has.
        await wait(260 + Math.min(1600, visited * 0.6), signal)
        files.sort((a, b) => compare(a.relativePath, b.relativePath))
        directories.sort((a, b) => compare(a.relativePath, b.relativePath))
        excluded.sort(compare)

        const list = categories()
        for (const category of list) {
            if (!['resourcePacks', 'shaders', 'worlds'].includes(category.id)) continue
            const objects = new Set<string>()
            for (const path of [
                ...directories.map((directory) => directory.relativePath),
                ...files.map((file) => file.relativePath),
            ]) {
                const parts = path.split('/')
                if (parts.length >= 2 && category.paths.some((prefix) => path.startsWith(prefix))) {
                    objects.add(`${parts[0]}/${parts[1]}`)
                }
            }
            for (const object of [...objects].sort(compare)) {
                const id = `${category.id}:${object}`
                let modifiedAt: number | null = null
                for (const file of files) {
                    if (
                        file.categoryId === category.id &&
                        (file.relativePath === object || file.relativePath.startsWith(`${object}/`))
                    ) {
                        file.categoryId = id
                        if (file.modifiedAt !== null)
                            modifiedAt = Math.max(modifiedAt ?? 0, file.modifiedAt)
                    }
                }
                // The world's name comes from its `level.dat`.
                const title =
                    category.id === 'worlds'
                        ? (worlds.find(
                              (world) =>
                                  world.type === 'singleplayer' && `saves/${world.path}` === object,
                          )?.name ?? null)
                        : null
                category.children.push({
                    id,
                    defaultSelected: category.defaultSelected,
                    paths: [object],
                    children: [],
                    title,
                    modifiedAt,
                })
            }
        }

        const scan: ResourceExportScan = {
            sessionId: crypto.randomUUID(),
            instanceId,
            instanceRevision: 1,
            files,
            directories,
            categories: list,
            excludedPaths: excluded,
            requirements: {
                gameVersion: instance.game_version,
                loader: instance.loader !== 'vanilla' ? instance.loader : null,
                loaderVersion: instance.loader_version ?? null,
            },
        }
        sessions.set(scan.sessionId, {
            scan,
            known: knownReferences(instanceId),
            results: new Map(),
            selected: [],
            selectionRevision: 0,
        })
        return scan
    }

    function sessionFor(sessionId: string) {
        const session = sessions.get(sessionId)
        if (!session) throw new Error('export session expired')
        return session
    }

    /** `ExportDraft::response`: the selected files' results. */
    function response(session: ExportSession, paths: string[]): ExportResolutionSession {
        const files = paths.flatMap((path) => session.results.get(path) ?? [])
        const failures = files.some(
            (file) =>
                file.localError ||
                file.resolutions.some((resolution) => resolution.kind === 'failed'),
        )
        return {
            id: session.scan.sessionId,
            requestId: crypto.randomUUID(),
            selectionRevision: session.selectionRevision,
            instanceRevision: session.scan.instanceRevision,
            state: failures ? 'needsDecision' : 'ready',
            files,
        }
    }

    /** `resolve_source`: a file matches the release the instance installed it from. */
    function resolve(
        session: ExportSession,
        file: ExportFileSnapshot,
        source: SourceId,
    ): ExportSourceResolution {
        const known = session.known.get(file.relativePath)
        if (source !== 'modrinth' || !known || known.size !== file.integrity.size)
            return { kind: 'noMatch', source }
        return {
            kind: 'matched',
            source,
            artifacts: [
                {
                    reference: {
                        release: {
                            project: { source, projectId: known.projectId },
                            releaseId: known.versionId,
                        },
                        artifactId: known.fileName,
                    },
                    fileName: known.fileName,
                    role: 'primary',
                    integrity: file.integrity,
                },
            ],
        }
    }

    async function check(
        session: ExportSession,
        candidates: ExportFileSnapshot[],
        sources: SourceId[],
        revision: number,
        onProgress: (progress: ExportRefreshProgress) => void,
        signal?: AbortSignal,
    ) {
        const sessionId = session.scan.sessionId
        const requestId = crypto.randomUUID()
        const identifiable = candidates.filter((file) => file.identifiable)
        const overallTotal = candidates.length + candidates.length * sources.length
        const totalBytes = Math.max(
            1,
            candidates.reduce((sum, file) => sum + Math.max(1, file.integrity.size ?? 0), 0),
        )
        // Hashing reads every candidate once.
        const hashMs = 180 + (totalBytes / (420 * 1024 * 1024)) * 1000
        let elapsed = 0
        while (elapsed < hashMs) {
            const fraction = Math.min(1, elapsed / hashMs)
            const local = fraction * candidates.length
            onProgress({
                sessionId,
                requestId,
                selectionRevision: revision,
                completed: Math.floor(local),
                total: candidates.length,
                phase: 'hashing',
                overallCompleted: Math.floor(local),
                overallTotal,
                currentFileFraction: local % 1,
            })
            await wait(250, signal)
            elapsed += 250
        }
        for (const file of candidates) {
            const previous = session.results.get(file.relativePath)
            session.results.set(file.relativePath, {
                snapshot: file,
                resolutions: previous?.resolutions ?? [],
                localError: null,
            })
        }
        // Files no source can identify count as checked for every source.
        let completed =
            candidates.length + (candidates.length - identifiable.length) * sources.length
        for (const source of sources) {
            for (let start = 0; start < identifiable.length; start += 100) {
                const batch = identifiable.slice(start, start + 100)
                onProgress({
                    sessionId,
                    requestId,
                    selectionRevision: revision,
                    completed: start,
                    total: identifiable.length,
                    phase: source,
                    overallCompleted: completed,
                    overallTotal,
                    currentFileFraction: 0,
                })
                await wait(source === 'modrinth' ? 520 : 680, signal)
                for (const file of batch) {
                    const result = session.results.get(file.relativePath)!
                    result.resolutions = [
                        ...result.resolutions.filter((resolution) => resolution.source !== source),
                        resolve(session, file, source),
                    ]
                }
                completed += batch.length
            }
        }
        onProgress({
            sessionId,
            requestId,
            selectionRevision: revision,
            completed: candidates.length,
            total: candidates.length,
            phase: 'complete',
            overallCompleted: completed,
            overallTotal,
            currentFileFraction: 0,
        })
        await wait(120, signal)
    }

    /** `resource_export_refresh`: hashes the files and looks them up on each source. */
    async function resourceExportRefresh(
        instanceId: string,
        sessionId: string,
        selectedFiles: ExportFileSnapshot[],
        selectionRevision: number,
        onProgress: (progress: ExportRefreshProgress) => void,
        signal?: AbortSignal,
    ): Promise<ExportResolutionSession> {
        const session = sessionFor(sessionId)
        if (session.scan.instanceId !== instanceId)
            throw new Error('export session instance mismatch')
        if (selectionRevision <= session.selectionRevision)
            throw new Error('stale export selection revision')
        const allowed = new Set(session.scan.files.map((file) => file.relativePath))
        if (selectedFiles.some((file) => !allowed.has(file.relativePath))) {
            throw new Error('selected file is not part of the export snapshot')
        }
        session.selected = selectedFiles.map((file) => file.relativePath)
        session.selectionRevision = selectionRevision
        await check(
            session,
            selectedFiles,
            ['modrinth', 'curseforge'],
            selectionRevision,
            onProgress,
            signal,
        )
        return response(session, session.selected)
    }

    /** `resource_export_refresh_retry`: asks the sources that failed again. */
    async function resourceExportRefreshRetry(
        sessionId: string,
        failedRefs: ExportEmbeddedFailureChoice[],
        selectionRevision: number,
        onProgress: (progress: ExportRefreshProgress) => void,
        signal?: AbortSignal,
    ): Promise<ExportResolutionSession> {
        const session = sessionFor(sessionId)
        session.selectionRevision = selectionRevision
        const sources = [...new Set(failedRefs.map((ref) => ref.source))]
        const files = [
            ...new Map(failedRefs.map((ref) => [ref.snapshot.relativePath, ref.snapshot])).values(),
        ]
        await check(session, files, sources, selectionRevision, onProgress, signal)
        return response(session, session.selected)
    }

    /** `resource_export_close`. */
    async function resourceExportClose(sessionId: string) {
        sessions.delete(sessionId)
    }

    return {
        resourceExportScan,
        resourceExportRefresh,
        resourceExportRefreshRetry,
        resourceExportClose,
    }
}
