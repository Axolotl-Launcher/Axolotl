import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { invoke } from '@/helpers/invoke'

export type InstanceBackupEligibility =
    | 'eligible'
    | 'not_installed'
    | 'symlink_instance'
    | 'direct_linked_instance'
    | 'external_game_directory'
    | 'root_missing'
    | 'root_is_link'

export interface BackupRepositoryStatus {
    path: string
    initialized: boolean
    available: boolean
    stored_size: number
    logical_size: number
    snapshot_count: number
    error: string | null
}

export type BackupExclusionKind = 'file' | 'directory'

export interface BackupExclusion {
    path: string
    kind: BackupExclusionKind
}

export function formatBackupExclusionPath(exclusion: BackupExclusion): string {
    return exclusion.kind === 'directory' ? `${exclusion.path}/` : exclusion.path
}

export interface InstanceBackupConfig {
    instance_id: string
    enabled: boolean
    eligibility: InstanceBackupEligibility
    excluded_paths: BackupExclusion[]
    snapshot_count: number
}

export interface BackupSnapshot {
    id: string
    instance_id: string
    instance_name: string
    created_at: number
    file_count: number
    symlink_count: number
    logical_size: number
    added_size: number
}

export interface BackupDeleteSummary {
    snapshot_count: number
    logical_size: number
}

export interface BackupRestorePreview {
    added_files: number
    modified_files: number
    deleted_files: number
    plan_token: string
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
    restore_preview?: BackupRestorePreview
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

export function canonicalizeBackupExclusions(values: BackupExclusion[]): BackupExclusion[] {
    const normalized = values
        .map((value) => ({ ...value, path: value.path.replaceAll('\\', '/') }))
        .sort(
            (left, right) =>
                left.path.split('/').length - right.path.split('/').length ||
                left.path.localeCompare(right.path),
        )
    const result: BackupExclusion[] = []
    for (const value of normalized) {
        const duplicate = result.find((existing) => existing.path === value.path)
        if (duplicate) {
            if (value.kind === 'directory') duplicate.kind = 'directory'
            continue
        }
        if (
            result.some(
                (parent) => parent.kind === 'directory' && value.path.startsWith(`${parent.path}/`),
            )
        ) {
            continue
        }
        result.push(value)
    }
    return result
}

export function getBackupRepositoryStatus(): Promise<BackupRepositoryStatus> {
    return invoke('plugin:instance|instance_get_backup_repository_status')
}

export function moveBackupRepository(destination: string): Promise<string> {
    return invoke('plugin:instance|instance_move_backup_repository', { destination })
}

export function getBackupConfig(instanceId: string): Promise<InstanceBackupConfig> {
    return invoke('plugin:instance|instance_get_backup_config', { instanceId })
}

export function normalizeBackupExclusion(
    instanceId: string,
    selectedPath: string,
    kind: BackupExclusionKind,
): Promise<BackupExclusion> {
    return invoke('plugin:instance|instance_normalize_backup_exclusion', {
        instanceId,
        selectedPath,
        kind,
    })
}

export function enableBackups(
    instanceId: string,
    excludedPaths: BackupExclusion[],
): Promise<InstanceBackupConfig> {
    return invoke('plugin:instance|instance_enable_backups', { instanceId, excludedPaths })
}

export function updateBackupExclusions(
    instanceId: string,
    excludedPaths: BackupExclusion[],
): Promise<InstanceBackupConfig> {
    return invoke('plugin:instance|instance_update_backup_exclusions', {
        instanceId,
        excludedPaths,
    })
}

export function disableBackups(instanceId: string): Promise<void> {
    return invoke('plugin:instance|instance_disable_backups', { instanceId })
}

export function startBackup(instanceId: string): Promise<string> {
    return invoke('plugin:instance|instance_start_backup', { instanceId })
}

export function cancelBackup(operationId: string): Promise<boolean> {
    return invoke('plugin:instance|instance_cancel_backup', { operationId })
}

export function listBackupOperations(
    instanceId?: string,
    activeOnly = false,
): Promise<BackupOperation[]> {
    return invoke('plugin:instance|instance_list_backup_operations', {
        instanceId,
        activeOnly,
    })
}

export function listBackups(instanceId: string): Promise<BackupSnapshot[]> {
    return invoke('plugin:instance|instance_list_backups', { instanceId })
}

export function deleteBackup(snapshotId: string): Promise<void> {
    return invoke('plugin:instance|instance_delete_backup', { snapshotId })
}

export function startBackupRestorePreview(snapshotId: string): Promise<string> {
    return invoke('plugin:instance|instance_get_backup_restore_preview', { snapshotId })
}

export function restoreBackup(snapshotId: string, planToken: string): Promise<string> {
    return invoke('plugin:instance|instance_restore_backup', { snapshotId, planToken })
}

export function getBackupDeleteSummary(instanceId: string): Promise<BackupDeleteSummary> {
    return invoke('plugin:instance|instance_get_backup_delete_summary', { instanceId })
}

export function listenBackupProgress(
    handler: (event: BackupProgressEvent) => void,
): Promise<UnlistenFn> {
    return listen<BackupProgressEvent>('instance_backup_progress', (event) =>
        handler(event.payload),
    )
}
