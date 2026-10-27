import { invoke } from '@tauri-apps/api/core'

/**
 * Typed wrappers for the `schematic-browser` plugin.
 *
 * Nothing outside this module calls `invoke` directly, so a command rename only
 * touches one file. Names use the `plugin:<id>|<command>` form; the id is
 * kebab-case because Tauri compares it character for character with the
 * `Builder::new()` argument.
 */

const PLUGIN = 'schematic-browser'

export type SchematicSourceId = 'reden' | 'buildschematics' | 'mc_schematics'

export type SchematicFormat = 'litematic' | 'schem' | 'schematic' | 'nbt' | 'unknown'

export type SchematicContentKind = 'schematic' | 'world'

export type SchematicDelivery = 'direct' | 'quark' | 'external'

export type SchematicTotalPrecision = 'exact' | 'estimated' | 'unknown'

export type SchematicSourceState = 'ok' | 'degraded' | 'offline' | 'unknown'

export interface SchematicDownloadPlan {
    delivery: SchematicDelivery
    url: string
    referer?: string
    fileNameHint?: string
    requiresLogin: boolean
}

export interface SchematicItem {
    id: string
    sourceId: SchematicSourceId
    title: string
    description?: string
    format: SchematicFormat
    contentKind: SchematicContentKind
    versions: string[]
    downloads?: number
    updatedAt?: number
    images: string[]
    detailUrl: string
    download: SchematicDownloadPlan
    degraded: boolean
    degradedReason?: string
}

export interface SchematicResourceTotal {
    value?: number
    precision: SchematicTotalPrecision
}

export interface SchematicSourceStatus {
    sourceId: SchematicSourceId
    state: SchematicSourceState
    message?: string
    cached: boolean
    fetchedAt?: number
}

export interface SchematicSourceFailure {
    sourceId: SchematicSourceId
    code: string
    message: string
    retryable: boolean
}

export interface SchematicListPayload {
    items: SchematicItem[]
    page: number
    hasMore: boolean
    total: SchematicResourceTotal
    sourceStatus: SchematicSourceStatus
    sourceFailures: SchematicSourceFailure[]
}

export interface SchematicMaterialEntry {
    name: string
    count: number
}

export interface SchematicDetail {
    item: SchematicItem
    images: string[]
    materials: SchematicMaterialEntry[]
    description?: string
    sourceStatus: SchematicSourceStatus[]
}

export interface SchematicSourceCapabilities {
    supportsSearch: boolean
    supportsVersionFilter: boolean
    supportsServerSidePaging: boolean
    supportsThumbnail: boolean
    supportsQuark: boolean
    supportsGenerator: boolean
}

export interface SchematicSourceCapabilityEntry {
    sourceId: SchematicSourceId
    displayName: string
    capabilities: SchematicSourceCapabilities
}

export interface SchematicGenerateSize {
    x?: number
    y?: number
    z?: number
}

export interface SchematicListArgs {
    sourceId: SchematicSourceId
    page?: number
    pageSize?: number
    query?: string
    versions?: string[]
}

let requestCounter = 0

/**
 * Request ids have to be unique per call so a cancel targets the right one.
 * A monotonic counter is enough; the value never leaves the process.
 */
function nextRequestId(): string {
    requestCounter += 1
    return `schematic-${Date.now()}-${requestCounter}`
}

export async function listSchematics(args: SchematicListArgs): Promise<SchematicListPayload> {
    return invoke<SchematicListPayload>(`plugin:${PLUGIN}|schematic_browser_list`, {
        sourceId: args.sourceId,
        page: args.page,
        pageSize: args.pageSize,
        query: args.query,
        versions: args.versions,
        requestId: nextRequestId(),
    })
}

export async function getSchematicDetail(
    sourceId: SchematicSourceId,
    id: string,
): Promise<SchematicDetail> {
    return invoke<SchematicDetail>(`plugin:${PLUGIN}|schematic_browser_detail`, {
        sourceId,
        id,
        requestId: nextRequestId(),
    })
}

export async function getSchematicDownloadPlan(
    sourceId: SchematicSourceId,
    id: string,
    size?: SchematicGenerateSize,
): Promise<SchematicDownloadPlan> {
    return invoke<SchematicDownloadPlan>(`plugin:${PLUGIN}|schematic_browser_download_plan`, {
        sourceId,
        id,
        size,
        requestId: nextRequestId(),
    })
}

export async function cancelSchematicRequest(requestId: string): Promise<boolean> {
    return invoke<boolean>(`plugin:${PLUGIN}|schematic_browser_cancel`, { requestId })
}

/**
 * Fetch one item straight into an instance and return the written path.
 *
 * Only `direct` deliveries can be applied; the source-page kinds (`quark`,
 * `external`) are refused by the backend and the UI sends those to the browser
 * instead, so callers should branch on `item.download.delivery` first.
 */
export async function applySchematicDownload(
    instanceId: string,
    sourceId: SchematicSourceId,
    id: string,
    size?: SchematicGenerateSize,
): Promise<string> {
    return invoke<string>(`plugin:${PLUGIN}|schematic_browser_download_apply`, {
        instanceId,
        sourceId,
        id,
        size,
        requestId: nextRequestId(),
    })
}

export async function getSchematicCapabilities(): Promise<SchematicSourceCapabilityEntry[]> {
    return invoke<SchematicSourceCapabilityEntry[]>(
        `plugin:${PLUGIN}|schematic_browser_capabilities`,
    )
}

export async function getSchematicHealth(): Promise<SchematicSourceStatus[]> {
    return invoke<SchematicSourceStatus[]>(`plugin:${PLUGIN}|schematic_browser_health`)
}

/** Display label for a source, used when the capability payload is not loaded. */
export const SCHEMATIC_SOURCE_LABELS: Record<SchematicSourceId, string> = {
    reden: 'Reden',
    buildschematics: 'BuildSchematics',
    mc_schematics: 'mc-schematics',
}

/** Every source, in the order the backend registry exposes them. */
export const SCHEMATIC_SOURCE_IDS: SchematicSourceId[] = [
    'reden',
    'buildschematics',
    'mc_schematics',
]
