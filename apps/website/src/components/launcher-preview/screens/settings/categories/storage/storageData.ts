/**
 * Storage analysis data shapes (apps/app-frontend/src/helpers/storage.ts and
 * helpers/instance-backup.ts), plus `sortStorageChildren` of
 * components/ui/settings/storage/storageData.ts.
 */

export type StorageNodeType =
    | 'instances'
    | 'backups'
    | 'servers'
    | 'synced_options'
    | 'components'
    | 'launcher_logs'
    | 'launcher_data'
    | 'icons'
    | 'install_rollbacks'
    | 'content_store'
    | 'resource_backups'
    | 'resource_cache'
    | 'staging'
    | 'folder'
    | 'java_versions'
    | 'versions'
    | 'libraries'
    | 'assets'
    | 'natives'
    | 'log_configs'
    | 'game_locales'
    | 'resources'
    | 'logs'
    | 'crash_reports'
    | 'cache'
    | 'meta'
    | 'database'
    | 'other'
    | 'instance'
    | 'mods'
    | 'replay'
    | 'resourcepacks'
    | 'saves'
    | 'world'
    | 'schematics'
    | 'screenshots'
    | 'shaderpacks'
    | 'minimap'
    | 'distant-horizons'
    | 'db-file'
    | 'db-backup'

export interface StoragePath {
    path: string
    kind: 'file' | 'directory'
}

export interface StorageSize {
    actual: number
    symlink: number
}

export interface StorageNode {
    id: string
    type: StorageNodeType
    name?: string
    instance_id?: string
    size: StorageSize
    count?: number
    paths: StoragePath[]
    children?: StorageNode[]
}

export interface StorageTree {
    version?: number
    scannedAt?: string
    total: StorageSize
    categories: StorageNode[]
    rootOther: StorageNode | null
    external: StorageNode[]
    warnings: string[]
}

export interface StorageOpenResult {
    opened: string[]
    failed: { path: string; reason: string }[]
}

export interface MetaGroup {
    id: string
    category: string
    name: string
    used: number
    unused: number
    unknown: number
    files: number
}

export interface MetaWarning {
    kind:
        | 'version_unavailable'
        | 'processor_unresolved'
        | 'database_unavailable'
        | 'active_operation'
        | 'custom_arguments'
        | 'external_metadata'
        | 'unreadable_path'
    subject: string
    detail: string
}

export interface MetaAnalysis {
    id: string
    scanned_at: string
    groups: MetaGroup[]
    warnings: MetaWarning[]
}

export interface MetaCleanupResult {
    removed_files: number
    removed_bytes: number
    removed_groups: Record<string, { files: number; bytes: number }>
    skipped_files: number
    failures: string[]
}

export interface CacheCleanupResult {
    removed_bytes: number
    remaining_bytes: number
    failures: string[]
}

export type StorageScanEvent =
    | { kind: 'cached'; tree: StorageTree }
    | { kind: 'progress'; completed: number; total: number; category: StorageNodeType }

/* ---- helpers/instance-backup.ts ------------------------------------------- */

export interface BackupRepositoryStatus {
    path: string
    initialized: boolean
    available: boolean
    stored_size: number
    logical_size: number
    snapshot_count: number
    error: string | null
}

export type BackupOperationType =
    'create' | 'restore' | 'restore_preview' | 'delete' | 'repository_move'

export type BackupOperationState =
    | 'queued'
    | 'scanning'
    | 'hashing'
    | 'saving'
    | 'validating'
    | 'materializing'
    | 'applying'
    | 'completed'
    | 'cancelled'
    | 'failed'
    | 'interrupted'

export interface BackupOperation {
    id: string
    operation_type: BackupOperationType
    instance_id: string
    snapshot_id?: string
    state: BackupOperationState
    processed_bytes: number
    total_bytes: number
    cancellable: boolean
    cancel_requested: boolean
    error?: string
    created_at: number
    updated_at: number
    finished_at?: number
}

export type BackupProgressStage =
    | 'scanning'
    | 'hashing'
    | 'saving'
    | 'validating'
    | 'copying'
    | 'restoring'
    | 'deleting'
    | 'completed'
    | 'cancelled'
    | 'failed'

export type BackupOperationFinalState = 'completed' | 'cancelled' | 'failed'

export interface BackupProgressEvent {
    operationId: string
    instanceId?: string
    operationType: BackupOperationType
    stage: BackupProgressStage
    processedBytes: number
    totalBytes: number
    finalState?: BackupOperationFinalState
    snapshotId?: string
    message?: string
}

/* ---- components/ui/settings/storage/storageData.ts ------------------------ */

export function sortStorageChildren(children: StorageNode[] | undefined): StorageNode[] {
    if (!children) return []
    return [...children]
        .filter((child) => child.size.actual + child.size.symlink > 0)
        .sort((a, b) => {
            const aIsOther = a.type === 'other' ? 1 : 0
            const bIsOther = b.type === 'other' ? 1 : 0
            if (aIsOther !== bIsOther) return aIsOther - bIsOther
            const aTotal = a.size.actual + a.size.symlink
            const bTotal = b.size.actual + b.size.symlink
            return bTotal - aTotal
        })
}
