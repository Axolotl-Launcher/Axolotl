import { Channel, invoke } from '@tauri-apps/api/core'

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

export function scanStorage(
    force: boolean,
    progress?: (event: StorageScanEvent) => void,
): Promise<StorageTree> {
    const onProgress = new Channel<StorageScanEvent>()
    onProgress.onmessage = progress ?? (() => {})
    return invoke('plugin:storage|storage_scan', { force, onProgress })
}

export function analyzeMeta(): Promise<MetaAnalysis> {
    return invoke('plugin:storage|storage_meta_analyze')
}

export function cleanMeta(id: string, groups: string[]): Promise<MetaCleanupResult> {
    return invoke('plugin:storage|storage_meta_clean', { id, groups })
}

export function openStoragePaths(paths: StoragePath[]): Promise<StorageOpenResult> {
    return invoke('plugin:storage|storage_open_paths', { paths })
}
