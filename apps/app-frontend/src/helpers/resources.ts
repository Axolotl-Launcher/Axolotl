import { Channel, invoke } from '@tauri-apps/api/core'

import type { InstallJobSnapshot } from './install'

export type SourceId = 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
export type ResourceType =
    'mod' | 'plugin' | 'dataPack' | 'resourcePack' | 'shader' | 'modPack' | 'world'
export interface SourceProjectRef {
    source: SourceId
    projectId: string
}
export interface SourceReleaseRef {
    project: SourceProjectRef
    releaseId: string
}
export type ResourceRef =
    { kind: 'resource'; id: string } | { kind: 'source'; project: SourceProjectRef }
export interface ResourceTarget {
    gameVersions: string[]
    loaders: string[]
    channels: string[]
}
export interface ResourceText {
    source: SourceId
    format: 'markdown' | 'html' | 'plain'
    body: string
}
export interface SourceProjectMetadata {
    environments: string[]
    project: SourceProjectRef
    slug: string | null
    title: string
    summary: string
    iconUrl: string | null
    types: ResourceType[]
    authors: string[]
    license: string | null
    downloads: number | null
    followers: number | null
    createdAt: string | null
    projectPublishedAt?: string | null
    updatedAt: string | null
    firstPublishedAt: string | null
    latestPublishedAt: string | null
    categories: string[]
    gameVersions: string[]
    loaders: string[]
    gallery: string[]
}
export interface Resource {
    confirmedProjects: SourceProjectRef[]
    sourceFailures: { source: SourceId; code: string; message: string; retryable: boolean }[]
    id: string | null
    defaultProject: SourceProjectRef
    projects: SourceProjectMetadata[]
}
export interface ResourceArtifact {
    reference: { release: SourceReleaseRef; artifactId: string }
    fileName: string | null
    role: string
    available: boolean | null
    integrity: { size: number | null; hashes: { algorithm: string; value: string }[] }
    acquisition:
        | { kind: 'publicDownload'; urls: string[] }
        | { kind: 'manualDownload'; pageUrl: string }
        | { kind: 'unavailable' }
    dependencies: { target: unknown; kind: string; versionConstraint: string | null }[]
}
export interface ResourceRelease {
    reference: SourceReleaseRef
    version: string
    channel: string
    publishedAt: string | null
    compatibility: { gameVersions: string[]; loaders: string[] }
    artifacts: ResourceArtifact[]
    equivalentReleases: SourceReleaseRef[]
}
export interface ProjectDocument<T = unknown> {
    metadata: SourceProjectMetadata
    description: ResourceText
    protocol: T | null
}
export interface ReleaseDocument<T = unknown> {
    release: ResourceRelease
    changelog: ResourceText | null
    protocol: T | null
}
export interface ResourceSearchQuery {
    filter?: ResourceSearchExpression | null
    text: string
    types: ResourceType[]
    sources: SourceId[]
    target: ResourceTarget
    categories: string[]
    sort:
        | 'projectPublished'
        | 'relevance'
        | 'downloads'
        | 'followers'
        | 'created'
        | 'projectUpdated'
        | 'firstPublished'
        | 'latestPublished'
    pageSize: number
}

export type ResourceSearchExpression =
    | { kind: 'all' | 'any'; terms: ResourceSearchExpression[] }
    | { kind: 'predicate'; field: string; values: string[]; excluded: boolean }
export interface ResourceSearchResult {
    entry: {
        queryEntryId: string
        resourceId: string | null
        defaultProject: SourceProjectRef
        candidates: SourceProjectRef[]
        status: 'confirmed' | 'unconfirmed' | 'rejected'
        basis: string[]
    }
    projects: SourceProjectMetadata[]
}
export interface ResourcePage<T> {
    requestId: string
    entries: T[]
    cursor: string | null
    hasMore: boolean
    total: { value: number | null; precision: 'exact' | 'estimate' | 'unknown' }
    sourceFailures: { source: SourceId; code: string; message: string; retryable: boolean }[]
    exhaustedSources: SourceId[]
}
export const emptyResourceTarget = (): ResourceTarget => ({
    gameVersions: [],
    loaders: [],
    channels: [],
})
export const sourceResource = (source: SourceId, projectId: string): ResourceRef => ({
    kind: 'source',
    project: { source, projectId },
})

export async function resourceRequest<T>(
    command: string,
    args: Record<string, unknown>,
    signal?: AbortSignal,
): Promise<T> {
    signal?.throwIfAborted()
    const requestId = crypto.randomUUID()
    const cancel = () => {
        void invoke('plugin:axolotl-resources|resource_cancel', { requestId }).catch(() => {})
    }
    signal?.addEventListener('abort', cancel, { once: true })
    try {
        const result = await invoke<T>(`plugin:axolotl-resources|${command}`, {
            ...args,
            requestId,
        })
        if (signal?.aborted && command === 'resource_export_scan') {
            const sessionId = (result as { sessionId?: string })?.sessionId
            if (sessionId) await resourceExportClose(sessionId).catch(() => {})
        }
        signal?.throwIfAborted()
        return result
    } finally {
        signal?.removeEventListener('abort', cancel)
    }
}
export function resourceSearch(
    query: ResourceSearchQuery,
    cursor: string | null,
    signal?: AbortSignal,
) {
    return resourceRequest<ResourcePage<ResourceSearchResult>>(
        'resource_search',
        { query, cursor },
        signal,
    )
}
export function resourceGet(
    reference: ResourceRef,
    source?: SourceId,
    locator?: string,
    signal?: AbortSignal,
) {
    return resourceRequest<Resource>(
        'resource_get',
        { reference, source: source ?? null, locator: locator ?? null },
        signal,
    )
}
export function resourceDescription(
    reference: ResourceRef,
    source: SourceId,
    locator?: string,
    signal?: AbortSignal,
) {
    return resourceRequest<ResourceText>(
        'resource_get_description',
        { reference, source, locator: locator ?? null },
        signal,
    )
}
export function resourceLocate<T>(
    source: SourceId,
    locator: string,
    signal?: AbortSignal,
    cachePolicy: ResourceCachePolicy = 'useFresh',
) {
    return resourceRequest<ProjectDocument<T>>(
        'resource_locate',
        { source, locator, cachePolicy },
        signal,
    )
}
export async function resourceProjectProtocol<T>(
    source: SourceId,
    locator: string,
    signal?: AbortSignal,
    cachePolicy: ResourceCachePolicy = 'useFresh',
): Promise<T> {
    const document = await resourceLocate<T>(source, locator, signal, cachePolicy)
    if (!document.protocol) throw new Error('Source presentation is unavailable')
    return document.protocol
}
export function resourceReleaseDocuments<T>(
    source: SourceId,
    releaseIds: string[],
    projectId?: string,
    signal?: AbortSignal,
    cachePolicy: ResourceCachePolicy = 'useFresh',
) {
    return resourceRequest<ReleaseDocument<T>[]>(
        'resource_release_documents',
        { source, releaseIds, projectId: projectId ?? null, cachePolicy },
        signal,
    )
}

export function resourceProjectDocuments<T>(
    source: SourceId,
    projectIds: string[],
    signal?: AbortSignal,
    cachePolicy: ResourceCachePolicy = 'useFresh',
) {
    return resourceRequest<ProjectDocument<T>[]>(
        'resource_project_documents',
        { source, projectIds, cachePolicy },
        signal,
    )
}
export function resourceReleases(
    reference: ResourceRef,
    target = emptyResourceTarget(),
    cursor: string | null = null,
    locator?: string,
    signal?: AbortSignal,
) {
    return resourceRequest<ResourcePage<ResourceRelease>>(
        'resource_list_releases',
        { reference, target, cursor, locator: locator ?? null },
        signal,
    )
}
export async function resourceChangelog(reference: SourceReleaseRef, signal?: AbortSignal) {
    const result = await resourceRequest<{
        release: SourceReleaseRef
        publishedAt: string | null
        text: ResourceText
    }>('resource_get_changelog', { releaseRef: reference, locator: null }, signal)
    return result.text
}
export function resourcePreferredSource() {
    return invoke<SourceId>('plugin:axolotl-resources|resource_preferred_source')
}
export function resourceSetPreferredSource(source: SourceId) {
    return invoke<void>('plugin:axolotl-resources|resource_set_preferred_source', { source })
}

export type ResourceCachePolicy =
    'useFresh' | 'revalidate' | 'forceRefresh' | 'onlyCached' | 'staleWhileRevalidate'

export type ResourceOperationIntent = 'install' | 'update' | 'switchVersion'
export type ResourceSelection = {
    id: string
    resource: ResourceRef
    release:
        | { kind: 'automatic' }
        | { kind: 'release'; reference: SourceReleaseRef }
        | { kind: 'artifact'; reference: ResourceArtifact['reference'] }
    resourceType: ResourceType
    scope: { kind: 'instance' } | { kind: 'world'; relativePath: string }
    entryId?: string | null
}
export type InstalledResource = {
    entryId: string
    instanceId: string
    resourceId: string | null
    artifact: ResourceArtifact['reference'] | null
    knownArtifacts: ResourceArtifact['reference'][]
    integrity: ResourceArtifact['integrity']
    relativePath: string
    originalFileName: string | null
    resourceType: ResourceType
    scope: ResourceSelection['scope']
    enabled: boolean
    missing: boolean
    ownership: { kind: 'userAdded' | 'packManaged' | 'localDiscovered'; packMemberId?: string }
}
export type ResourceDependencyNode = {
    id: string
    rootSelectionIds?: string[]
    resourceType?: ResourceType | null
    resource: ResourceRef
    scope: ResourceSelection['scope']
    state: string
    artifact: ResourceArtifact | null
    installedEntries: string[]
    compatibility: {
        gameVersion: 'compatible' | 'incompatible' | 'unknown'
        loader: 'compatible' | 'incompatible' | 'unknown'
        reasons: string[]
    }
    declaredCompatibility?: { gameVersions: string[]; loaders: string[] } | null
}
export function resourceListInstalled(instanceId: string, signal?: AbortSignal) {
    return resourceRequest<InstalledResource[]>('resource_list_installed', { instanceId }, signal)
}
export interface ResourceUpdateProgress {
    entryId: string
    release: SourceReleaseRef | null
    failed: boolean
    unresolved: boolean
}
export interface ResourceUpdateCheckOptions {
    entryIds?: string[]
    onProgress?: (progress: ResourceUpdateProgress) => void
}
export function resourceCheckUpdates(
    instanceId: string,
    signal?: AbortSignal,
    options?: ResourceUpdateCheckOptions,
) {
    let onProgress: Channel<ResourceUpdateProgress> | null = null
    if (options?.onProgress) {
        onProgress = new Channel<ResourceUpdateProgress>()
        onProgress.onmessage = (progress) => {
            if (!signal?.aborted) options.onProgress?.(progress)
        }
    }
    return resourceRequest<{
        entries: string[]
        releases: Record<string, SourceReleaseRef>
        unresolved: string[]
        failures: string[]
        failedEntries: string[]
    }>(
        'resource_check_updates',
        { instanceId, entryIds: options?.entryIds ?? null, onProgress },
        signal,
    )
}
export function resourcePrepareEntryOperation(
    instanceId: string,
    intent: ResourceOperationIntent,
    entryIds: string[],
    release: SourceReleaseRef | null,
    installDependencies: boolean,
    signal?: AbortSignal,
    onProgress?: (progress: ResourceOperationProgress) => void,
    resolveDependencies = true,
) {
    return resourceRequest<ResourceOperationPlan>(
        'resource_prepare_entry_operation',
        {
            instanceId,
            intent,
            entryIds,
            release,
            installDependencies,
            onProgress: operationProgressChannel(onProgress, signal),
            resolveDependencies,
        },
        signal,
    )
}
export type ResourceOperationPlan = {
    schemaVersion: number
    id: string
    revision: number
    instanceId: string
    instanceRevision: number
    intent: ResourceOperationIntent
    selections: ResourceSelection[]
    activeRoots: string[]
    nodes: ResourceDependencyNode[]
    edges: {
        parent: string
        child: string
        kind: string
        declaredBy: ResourceArtifact['reference']
        versionConstraint: string | null
        releaseSelection?: ResourceSelection['release'] | null
    }[]
    actions: {
        entryId: string
        nodeId: string
        kind: string
        targetRelativePath: string
        current: InstalledResource | null
        target: ResourceArtifact
    }[]
    issues: { blocking: boolean; code: string; message: string; selectionId: string | null }[]
    exceptions: {
        selectionId: string
        release: SourceReleaseRef
        dimensions: ('gameVersion' | 'loader')[]
    }[]
    selectionReasons: unknown[]
}
export type ResourceOperationProgress = {
    planId: string
    revision: number
    node: ResourceDependencyNode
    metadata: SourceProjectMetadata | null
}
function operationProgressChannel(
    onProgress?: (progress: ResourceOperationProgress) => void,
    signal?: AbortSignal,
) {
    if (!onProgress) return null
    const channel = new Channel<ResourceOperationProgress>()
    channel.onmessage = (progress) => {
        if (!signal?.aborted) onProgress(progress)
    }
    return channel
}
export type ResourcePlanDecision =
    | { kind: 'ignoreDependencies' }
    | { kind: 'skipUnwritableFiles' }
    | { kind: 'addRequired'; nodeId: string }
    | { kind: 'addOptional'; nodeId: string }
    | { kind: 'removeOptional'; nodeId: string }
    | { kind: 'removeRoot'; selectionId: string }
    | { kind: 'acceptCompatibility'; exception: unknown }
    | { kind: 'selectRelease'; nodeId: string; selection: ResourceSelection['release'] }

export function resourcePrepareOperation(
    instanceId: string,
    intent: ResourceOperationIntent,
    selections: ResourceSelection[],
    installDependencies = true,
    signal?: AbortSignal,
    importFiles?: Record<string, string>,
    locators?: Record<string, string>,
    onProgress?: (progress: ResourceOperationProgress) => void,
    resolveDependencies = true,
    analysis?: { sessionId: string; selectionRevision: number },
) {
    return resourceRequest<ResourceOperationPlan>(
        'resource_prepare_operation',
        {
            instanceId,
            intent,
            selections,
            installDependencies,
            importFiles: importFiles ?? null,
            locators: locators ?? null,
            onProgress: operationProgressChannel(onProgress, signal),
            resolveDependencies,
            analysisSessionId: analysis?.sessionId ?? null,
            selectionRevision: analysis?.selectionRevision ?? null,
        },
        signal,
    )
}
export function resourceAnalysisOpen(instanceId: string) {
    return invoke<string>('plugin:axolotl-resources|resource_analysis_open', { instanceId })
}
export function resourceAnalysisClose(sessionId: string) {
    return invoke<void>('plugin:axolotl-resources|resource_analysis_close', { sessionId })
}
export function resourceAnalysisUpdate(
    sessionId: string,
    selectionRevision: number,
    selections: ResourceSelection[],
    installDependencies: boolean,
    signal: AbortSignal,
    onProgress: (progress: ResourceOperationProgress) => void,
) {
    return resourceRequest<ResourceOperationPlan>(
        'resource_analysis_update',
        {
            sessionId,
            selectionRevision,
            selections,
            installDependencies,
            onProgress: operationProgressChannel(onProgress, signal),
        },
        signal,
    )
}
export function resourceRevisePlan(
    planId: string,
    revision: number,
    decisions: ResourcePlanDecision[],
    signal?: AbortSignal,
    onProgress?: (progress: ResourceOperationProgress) => void,
) {
    return resourceRequest<ResourceOperationPlan>(
        'resource_revise_plan',
        { planId, revision, decisions, onProgress: operationProgressChannel(onProgress, signal) },
        signal,
    )
}
export function resourceQueuePlan(planId: string, revision: number) {
    return invoke<InstallJobSnapshot>('plugin:axolotl-resources|resource_queue_plan', {
        planId,
        revision,
    })
}

export type ResourceHistoryEntry = {
    id: string
    entryId: string
    instanceId: string
    recordedAt: string
    previous: InstalledResource
    target: InstalledResource
    backupRelativePath: string | null
    backupAvailable: boolean
    backupRetired?: boolean
}

export function resourceListHistory(instanceId: string, entryId?: string | null) {
    return invoke<ResourceHistoryEntry[]>('plugin:axolotl-resources|resource_list_history', {
        instanceId,
        entryId: entryId ?? null,
    })
}

export function resourceRollback(instanceId: string, entryId: string, historyId: string) {
    return invoke<InstallJobSnapshot>('plugin:axolotl-resources|resource_rollback', {
        instanceId,
        entryId,
        historyId,
    })
}
export type ResourceBackupFile = {
    id: string
    historyId: string | null
    relativePath: string
    originalFileName?: string | null
    integrity: ResourceArtifact['integrity']
    rollbackAvailable: boolean
    recognized: boolean
    exists: boolean
}
export type ResourceHistoryCleanupPlan = {
    id: string
    revision: number
    instanceId: string
    files: ResourceBackupFile[]
}
export function resourceScanBackups(instanceId: string) {
    return invoke<ResourceBackupFile[]>('plugin:axolotl-resources|resource_scan_backups', {
        instanceId,
    })
}
export function resourcePrepareHistoryCleanup(instanceId: string, historyIds: string[]) {
    return invoke<ResourceHistoryCleanupPlan>(
        'plugin:axolotl-resources|resource_prepare_history_cleanup',
        { instanceId, historyIds },
    )
}
export function resourceExecuteHistoryCleanup(cleanupPlanId: string, revision: number) {
    return invoke<{ removed: string[]; failures: [string, string][] }>(
        'plugin:axolotl-resources|resource_execute_history_cleanup',
        { cleanupPlanId, revision },
    )
}
export function resourceCachePolicy(behavior?: string): ResourceCachePolicy {
    if (behavior === 'revalidate') return 'revalidate'
    return behavior === 'bypass'
        ? 'forceRefresh'
        : behavior === 'cache_only'
          ? 'onlyCached'
          : behavior === 'stale_while_revalidate' ||
              behavior === 'stale_while_revalidate_skip_offline'
            ? 'staleWhileRevalidate'
            : 'useFresh'
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
export function resourceExportScan(instanceId: string, signal?: AbortSignal) {
    return resourceRequest<ResourceExportScan>('resource_export_scan', { instanceId }, signal)
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
export function resourceExportRefresh(
    instanceId: string,
    sessionId: string,
    selectedFiles: ExportFileSnapshot[],
    selectionRevision: number,
    progress: (value: ExportRefreshProgress) => void,
    signal?: AbortSignal,
) {
    const onProgress = new Channel<ExportRefreshProgress>()
    onProgress.onmessage = progress
    return resourceRequest<ExportResolutionSession>(
        'resource_export_refresh',
        { instanceId, sessionId, selectedFiles, selectionRevision, onProgress },
        signal,
    )
}
export function resourceExportRefreshRetry(
    sessionId: string,
    failedRefs: ExportEmbeddedFailureChoice[],
    selectionRevision: number,
    progress: (value: ExportRefreshProgress) => void,
    signal?: AbortSignal,
) {
    const onProgress = new Channel<ExportRefreshProgress>()
    onProgress.onmessage = progress
    return resourceRequest<ExportResolutionSession>(
        'resource_export_refresh_retry',
        { sessionId, failedRefs, selectionRevision, onProgress },
        signal,
    )
}
export function resourceExportClose(sessionId: string) {
    return invoke<void>('plugin:axolotl-resources|resource_export_close', { sessionId })
}
export type ResourceExportFormat = 'modrinth' | 'genericZip' | 'curseForge'
export interface ResourceExportPlan {
    id: string
    revision: number
    sessionId: string
    selectionRevision: number
    format: ResourceExportFormat
}
export function resourceExportPlan(
    sessionId: string,
    selectionRevision: number,
    format: ResourceExportFormat,
    packageInfo: { name: string; version: string; summary: string; author: string | null },
    embeddedFailureChoices: ExportEmbeddedFailureChoice[],
) {
    return invoke<ResourceExportPlan>('plugin:axolotl-resources|resource_export_plan', {
        sessionId,
        selectionRevision,
        format,
        packageInfo,
        embeddedFailureChoices,
    })
}
export function resourceExportExecute(planId: string, revision: number, outputPath: string) {
    return invoke<import('./install').InstallJobSnapshot>(
        'plugin:axolotl-resources|resource_export_execute',
        { planId, revision, outputPath },
    )
}

export function resourceExportQueue(
    sessionId: string,
    selectedFiles: ExportFileSnapshot[],
    format: ResourceExportFormat,
    packageInfo: { name: string; version: string; summary: string; author: string | null },
    embeddedFailureChoices: ExportEmbeddedFailureChoice[],
    outputPath: string,
) {
    return invoke<import('./install').InstallJobSnapshot>(
        'plugin:axolotl-resources|resource_export_queue',
        { sessionId, selectedFiles, format, packageInfo, embeddedFailureChoices, outputPath },
    )
}
export function resourceExportCancel(requestIdOrJobId: string) {
    return invoke<void>('plugin:axolotl-resources|resource_export_cancel', { requestIdOrJobId })
}
