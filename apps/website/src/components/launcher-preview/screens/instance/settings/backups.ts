import { effectScope, reactive, watch } from 'vue'

import { type PreviewState, usePreviewDomain } from '../../../lib/state'
import type { ContentItem } from '../../../lib/state/downloads'
import { useInstanceSettingsState } from './state'

/*
 * The instance backup service (`packages/app-lib/src/api/instance/backup.rs`),
 * simulated over the preview's instance state: a deduplicating snapshot store per
 * window, operations that report the backend's progress stages through the clock,
 * and restore plans computed from the instance's files.
 *
 * An instance's files are its installed content (the shared content list, so
 * enabling, removing or installing content changes what a snapshot holds) plus
 * the files every played instance has. The sizes of those other files are
 * example values of the preview's instances.
 */

export type InstanceBackupEligibility =
    | 'eligible'
    | 'not_installed'
    | 'symlink_instance'
    | 'direct_linked_instance'
    | 'external_game_directory'
    | 'root_missing'
    | 'root_is_link'

export type BackupExclusionKind = 'file' | 'directory'

export interface BackupExclusion {
    path: string
    kind: BackupExclusionKind
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

export interface BackupProgressEvent {
    operationId: string
    instanceId?: string
    operationType: BackupOperationType
    stage: BackupProgressStage
    processedBytes: number
    totalBytes: number
    snapshotId?: string
    message?: string
}

/** `formatBackupExclusionPath` of `helpers/instance-backup.ts`. */
export function formatBackupExclusionPath(exclusion: BackupExclusion): string {
    return exclusion.kind === 'directory' ? `${exclusion.path}/` : exclusion.path
}

/** `canonicalizeBackupExclusions` of `helpers/instance-backup.ts`. */
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

interface FileEntry {
    path: string
    size: number
    /** Content identity: equal keys are stored once in the repository. */
    object: string
}

interface SnapshotRecord {
    snapshot: BackupSnapshot
    files: FileEntry[]
    content: ContentItem[]
}

const OPERATION_DELAY = 150

/** Throughput of the simulated hashing pass (bytes per second). */
const HASH_RATE = 220 * 1024 * 1024
/** Throughput of the simulated restore copy (bytes per second). */
const RESTORE_RATE = 320 * 1024 * 1024

/** Files besides the installed content (example values). */
const COMMON_FILES: [string, number][] = [
    ['options.txt', 7_412],
    ['servers.dat', 1_204],
    ['usercache.json', 318],
    ['command_history.txt', 2_131],
    ['logs/latest.log', 96_533],
    ['saves/New World/level.dat', 4_096],
    ['saves/New World/level.dat_old', 4_096],
    ['saves/New World/session.lock', 3],
    ['saves/New World/icon.png', 18_244],
    ['saves/New World/stats/8667ba71-b85a-4004-af54-457a9734eed7.json', 3_904],
    ['saves/New World/advancements/8667ba71-b85a-4004-af54-457a9734eed7.json', 12_771],
    ['saves/New World/playerdata/8667ba71-b85a-4004-af54-457a9734eed7.dat', 2_518],
]
const REGION_FILES = 24
const REGION_FILE_SIZE = 1_187_840

function folderOf(item: ContentItem): string {
    const slash = item.id.lastIndexOf('/')
    if (slash > 0 && item.id.slice(slash + 1) === item.file_name) return item.id.slice(0, slash)
    switch (item.project_type) {
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

function excluded(path: string, exclusions: BackupExclusion[]) {
    return exclusions.some((exclusion) =>
        exclusion.kind === 'directory'
            ? path === exclusion.path || path.startsWith(`${exclusion.path}/`)
            : path === exclusion.path,
    )
}

function randomId() {
    return `${Math.random().toString(16).slice(2, 10)}-${Math.random().toString(16).slice(2, 6)}-4${Math.random().toString(16).slice(2, 5)}-${(8 + Math.floor(Math.random() * 4)).toString(16)}${Math.random().toString(16).slice(2, 5)}-${Math.random().toString(16).slice(2, 14).padEnd(12, '0')}`
}

function createBackups(state: PreviewState) {
    const settings = useInstanceSettingsState()
    const configs = reactive<
        Record<string, { enabled: boolean; excluded_paths: BackupExclusion[] }>
    >({})
    const snapshots = reactive<Record<string, SnapshotRecord[]>>({})
    const operations = reactive<Record<string, BackupOperation[]>>({})
    /** Objects already in the repository (deduplicated across instances). */
    const objects = new Set<string>()
    const listeners = new Set<(event: BackupProgressEvent) => void>()
    const runners = new Map<string, () => void>()
    const plans = new Map<string, { snapshotId: string }>()

    function emit(event: BackupProgressEvent) {
        for (const listener of [...listeners]) listener(event)
    }

    function listen(handler: (event: BackupProgressEvent) => void) {
        listeners.add(handler)
        return () => listeners.delete(handler)
    }

    function eligibility(instanceId: string): InstanceBackupEligibility {
        const instance = state.instances.get(instanceId)
        if (!instance) return 'root_missing'
        if (instance.install_stage !== 'installed') return 'not_installed'
        if (instance.symlink_target) return 'symlink_instance'
        const extras = settings.extrasFor(instanceId)
        if (extras.linked_dot_minecraft) return 'direct_linked_instance'
        if (extras.game_dir_override) return 'external_game_directory'
        return 'eligible'
    }

    function configFor(instanceId: string): InstanceBackupConfig {
        const stored = configs[instanceId]
        return {
            instance_id: instanceId,
            enabled: !!stored?.enabled,
            eligibility: eligibility(instanceId),
            excluded_paths: stored?.enabled
                ? [...stored.excluded_paths].sort((a, b) => a.path.localeCompare(b.path))
                : [],
            snapshot_count: snapshots[instanceId]?.length ?? 0,
        }
    }

    function filesOf(instanceId: string, exclusions: BackupExclusion[]): FileEntry[] {
        const instance = state.instances.get(instanceId)
        const content = (state.downloads.contents.value[instanceId] ?? []).filter(
            (item) => !item.removed,
        )
        const files: FileEntry[] = content.map((item) => {
            const name = item.enabled ? item.file_name : `${item.file_name}.disabled`
            return {
                path: `${folderOf(item)}/${name}`,
                size: item.size,
                object: `${item.project_id ?? item.file_name}:${item.version_id ?? ''}:${item.size}`,
            }
        })
        const mods = content.filter((item) => folderOf(item) === 'mods').length
        for (let index = 0; index < Math.round(mods * 0.7); index++) {
            files.push({
                path: `config/${content[index]?.file_name.replace(/[-_ ].*$/, '').toLowerCase() || 'mod'}-${index}.json`,
                size: 2_560 + ((index * 977) % 6_144),
                object: `${instanceId}:config:${index}`,
            })
        }
        const played = instance?.last_played ?? instance?.created ?? ''
        for (const [path, size] of COMMON_FILES) {
            // The log and the world change whenever the instance is played.
            const changes = path === 'logs/latest.log' || path.startsWith('saves/')
            files.push({
                path,
                size,
                object: `${instanceId}:${path}:${changes ? played : ''}`,
            })
        }
        for (let index = 0; index < REGION_FILES; index++) {
            const x = (index % 6) - 3
            const z = Math.floor(index / 6) - 2
            files.push({
                path: `saves/New World/region/r.${x}.${z}.mca`,
                size: REGION_FILE_SIZE + ((index * 40_960) % 409_600),
                object: `${instanceId}:region:${index}:${index < 4 ? played : ''}`,
            })
        }
        return files.filter((file) => !excluded(file.path, exclusions))
    }

    function list(instanceId: string): BackupSnapshot[] {
        return (snapshots[instanceId] ?? []).map((record) => ({ ...record.snapshot }))
    }

    function listOperations(instanceId?: string): BackupOperation[] {
        const all = instanceId ? (operations[instanceId] ?? []) : Object.values(operations).flat()
        return [...all]
            .sort((a, b) => b.created_at - a.created_at)
            .map((operation) => ({ ...operation }))
    }

    function patchOperation(instanceId: string, id: string, patch: Partial<BackupOperation>) {
        const list = operations[instanceId]
        const index = list?.findIndex((operation) => operation.id === id) ?? -1
        if (!list || index < 0) return
        list[index] = { ...list[index], ...patch, updated_at: Date.now() }
    }

    function addOperation(
        instanceId: string,
        type: BackupOperationType,
        totalBytes: number,
        snapshotId?: string,
    ) {
        const now = Date.now()
        const operation: BackupOperation = {
            id: randomId(),
            operation_type: type,
            instance_id: instanceId,
            snapshot_id: snapshotId,
            state: 'queued',
            processed_bytes: 0,
            total_bytes: totalBytes,
            cancellable: true,
            cancel_requested: false,
            created_at: now,
            updated_at: now,
        }
        operations[instanceId] = [operation, ...(operations[instanceId] ?? [])]
        return operation
    }

    function hasActive(instanceId: string) {
        return (operations[instanceId] ?? []).some(
            (operation) =>
                !['completed', 'cancelled', 'failed', 'interrupted'].includes(operation.state),
        )
    }

    function isRunning(instanceId: string) {
        return state.processes.processes.value.some((process) => process.instance.id === instanceId)
    }

    function finish(
        instanceId: string,
        operation: BackupOperation,
        stage: 'completed' | 'cancelled' | 'failed',
        extra: {
            message?: string
            snapshotId?: string
            restorePreview?: BackupRestorePreview
        } = {},
    ) {
        runners.get(operation.id)?.()
        runners.delete(operation.id)
        const current = operations[instanceId]?.find((entry) => entry.id === operation.id)
        patchOperation(instanceId, operation.id, {
            state: stage,
            cancellable: false,
            finished_at: Date.now(),
            error: extra.message,
            restore_preview: extra.restorePreview,
            snapshot_id: extra.snapshotId ?? current?.snapshot_id,
        })
        emit({
            operationId: operation.id,
            instanceId,
            operationType: operation.operation_type,
            stage,
            processedBytes: current?.processed_bytes ?? 0,
            totalBytes: current?.total_bytes ?? 0,
            snapshotId: extra.snapshotId ?? current?.snapshot_id,
            message: extra.message,
        })
    }

    /**
     * Runs `steps` on the clock. A step reports progress over its duration when it
     * has a byte total; `onDone` runs when the last step ends.
     */
    function run(
        instanceId: string,
        operation: BackupOperation,
        steps: { stage: BackupProgressStage; duration: number; bytes: boolean }[],
        totalBytes: number,
        onDone: () => void,
    ) {
        let index = -1
        let elapsed = 0
        const report = (stage: BackupProgressStage, processed: number, total: number) => {
            const state: BackupOperationState =
                stage === 'restoring' || stage === 'copying'
                    ? 'materializing'
                    : (stage as BackupOperationState)
            patchOperation(instanceId, operation.id, {
                state,
                processed_bytes: processed,
                total_bytes: total,
                cancellable: stage === 'scanning' || stage === 'hashing',
            })
            emit({
                operationId: operation.id,
                instanceId,
                operationType: operation.operation_type,
                stage,
                processedBytes: processed,
                totalBytes: total,
                snapshotId: operation.snapshot_id,
            })
        }
        const advance = () => {
            index += 1
            elapsed = 0
            if (index >= steps.length) {
                stop()
                runners.delete(operation.id)
                onDone()
                return
            }
            const step = steps[index]
            const total = step.stage === 'scanning' ? 0 : totalBytes
            report(step.stage, step.stage === 'saving' ? totalBytes : 0, total)
        }
        const stop = state.clock.every((delta) => {
            const current = operations[instanceId]?.find((entry) => entry.id === operation.id)
            if (!current) {
                stop()
                runners.delete(operation.id)
                return
            }
            if (current.cancel_requested) {
                finish(instanceId, operation, 'cancelled')
                return
            }
            if (index < 0) {
                advance()
                return
            }
            const step = steps[index]
            elapsed += delta
            if (elapsed >= step.duration) {
                if (step.bytes) report(step.stage, totalBytes, totalBytes)
                advance()
                return
            }
            if (step.bytes) {
                const processed = Math.round(totalBytes * Math.min(1, elapsed / step.duration))
                report(step.stage, processed, totalBytes)
            }
        })
        runners.set(operation.id, stop)
    }

    function durationFor(bytes: number, rate: number, min: number, max: number) {
        return Math.min(max, Math.max(min, (bytes / rate) * 1000))
    }

    function enable(instanceId: string, excludedPaths: BackupExclusion[]): InstanceBackupConfig {
        if (eligibility(instanceId) !== 'eligible') {
            throw new Error('This instance is not eligible for backups')
        }
        configs[instanceId] = {
            enabled: true,
            excluded_paths: canonicalizeBackupExclusions(excludedPaths),
        }
        return configFor(instanceId)
    }

    function updateExclusions(
        instanceId: string,
        excludedPaths: BackupExclusion[],
    ): InstanceBackupConfig {
        const stored = configs[instanceId]
        if (!stored?.enabled) throw new Error('Backups are not enabled for this instance')
        stored.excluded_paths = canonicalizeBackupExclusions(excludedPaths)
        return configFor(instanceId)
    }

    function disable(instanceId: string) {
        if ((snapshots[instanceId]?.length ?? 0) > 0) {
            throw new Error('Delete every snapshot before disabling backups')
        }
        Reflect.deleteProperty(configs, instanceId)
    }

    function start(instanceId: string): string {
        if (hasActive(instanceId)) {
            throw new Error('Another backup operation is already running for this instance')
        }
        const operation = addOperation(instanceId, 'create', 0)
        state.clock.after(OPERATION_DELAY, () => {
            if (isRunning(instanceId)) {
                finish(instanceId, operation, 'failed', {
                    message: 'Close the instance before creating a backup',
                })
                return
            }
            const stored = configs[instanceId]
            if (!stored?.enabled) {
                finish(instanceId, operation, 'failed', {
                    message: 'Backups are not enabled for this instance',
                })
                return
            }
            const files = filesOf(instanceId, stored.excluded_paths)
            const total = files.reduce((sum, file) => sum + file.size, 0)
            run(
                instanceId,
                operation,
                [
                    { stage: 'scanning', duration: 700, bytes: false },
                    {
                        stage: 'hashing',
                        duration: durationFor(total, HASH_RATE, 1400, 4800),
                        bytes: true,
                    },
                    { stage: 'saving', duration: 600, bytes: false },
                ],
                total,
                () => {
                    let added = 0
                    for (const file of files) {
                        if (objects.has(file.object)) continue
                        objects.add(file.object)
                        added += file.size
                    }
                    const instance = state.instances.get(instanceId)
                    const snapshot: BackupSnapshot = {
                        id: randomId(),
                        instance_id: instanceId,
                        instance_name: instance?.name ?? '',
                        created_at: Date.now(),
                        file_count: files.length,
                        symlink_count: 0,
                        logical_size: total,
                        added_size: added,
                    }
                    snapshots[instanceId] = [
                        {
                            snapshot,
                            files,
                            content: (state.downloads.contents.value[instanceId] ?? []).map(
                                (item) => ({ ...item }),
                            ),
                        },
                        ...(snapshots[instanceId] ?? []),
                    ]
                    finish(instanceId, operation, 'completed', { snapshotId: snapshot.id })
                },
            )
        })
        return operation.id
    }

    function cancel(operationId: string): boolean {
        for (const [instanceId, list] of Object.entries(operations)) {
            const operation = list.find((entry) => entry.id === operationId)
            if (!operation) continue
            if (!operation.cancellable) return false
            patchOperation(instanceId, operationId, { cancel_requested: true, cancellable: false })
            return true
        }
        return false
    }

    function findSnapshot(snapshotId: string) {
        for (const [instanceId, list] of Object.entries(snapshots)) {
            const record = list.find((entry) => entry.snapshot.id === snapshotId)
            if (record) return { instanceId, record }
        }
        return null
    }

    function startRestorePreview(snapshotId: string): string {
        const found = findSnapshot(snapshotId)
        if (!found) throw new Error('Unknown backup snapshot')
        const { instanceId, record } = found
        if (hasActive(instanceId)) {
            throw new Error('Another backup operation is already running for this instance')
        }
        const operation = addOperation(
            instanceId,
            'restore_preview',
            record.snapshot.logical_size,
            snapshotId,
        )
        state.clock.after(OPERATION_DELAY, () => {
            if (isRunning(instanceId)) {
                finish(instanceId, operation, 'failed', {
                    message: 'Close the instance before restoring a backup',
                })
                return
            }
            const exclusions = configs[instanceId]?.excluded_paths ?? []
            const current = filesOf(instanceId, exclusions)
            const total = current.reduce((sum, file) => sum + file.size, 0)
            run(
                instanceId,
                operation,
                [
                    {
                        stage: 'hashing',
                        duration: durationFor(total, HASH_RATE, 1000, 3200),
                        bytes: true,
                    },
                ],
                total,
                () => {
                    const saved = new Map(record.files.map((file) => [file.path, file.object]))
                    const now = new Map(current.map((file) => [file.path, file.object]))
                    let addedFiles = 0
                    let modifiedFiles = 0
                    let deletedFiles = 0
                    for (const [path, object] of saved) {
                        if (!now.has(path)) addedFiles++
                        else if (now.get(path) !== object) modifiedFiles++
                    }
                    for (const path of now.keys()) if (!saved.has(path)) deletedFiles++
                    const planToken = randomId()
                    plans.set(planToken, { snapshotId })
                    finish(instanceId, operation, 'completed', {
                        snapshotId,
                        restorePreview: {
                            added_files: addedFiles,
                            modified_files: modifiedFiles,
                            deleted_files: deletedFiles,
                            plan_token: planToken,
                        },
                    })
                },
            )
        })
        return operation.id
    }

    function restore(snapshotId: string, planToken: string): string {
        const found = findSnapshot(snapshotId)
        if (!found) throw new Error('Unknown backup snapshot')
        if (plans.get(planToken)?.snapshotId !== snapshotId) {
            throw new Error('The restore plan is out of date. Preview the restore again.')
        }
        plans.delete(planToken)
        const { instanceId, record } = found
        const operation = addOperation(
            instanceId,
            'restore',
            record.snapshot.logical_size,
            snapshotId,
        )
        state.clock.after(OPERATION_DELAY, () => {
            if (isRunning(instanceId)) {
                finish(instanceId, operation, 'failed', {
                    message: 'Close the instance before restoring a backup',
                })
                return
            }
            run(
                instanceId,
                operation,
                [
                    { stage: 'validating', duration: 600, bytes: false },
                    {
                        stage: 'restoring',
                        duration: durationFor(
                            record.snapshot.logical_size,
                            RESTORE_RATE,
                            1200,
                            3600,
                        ),
                        bytes: true,
                    },
                ],
                record.snapshot.logical_size,
                () => {
                    state.downloads.contents.value = {
                        ...state.downloads.contents.value,
                        [instanceId]: record.content.map((item) => ({ ...item })),
                    }
                    finish(instanceId, operation, 'completed', { snapshotId })
                },
            )
        })
        return operation.id
    }

    function remove(snapshotId: string) {
        const found = findSnapshot(snapshotId)
        if (!found) throw new Error('Unknown backup snapshot')
        const { instanceId } = found
        snapshots[instanceId] = (snapshots[instanceId] ?? []).filter(
            (entry) => entry.snapshot.id !== snapshotId,
        )
        // Unused stored objects are collected with the snapshot.
        const used = new Set(
            Object.values(snapshots).flatMap((list) =>
                list.flatMap((entry) => entry.files.map((file) => file.object)),
            ),
        )
        for (const object of [...objects]) if (!used.has(object)) objects.delete(object)
    }

    function deleteSummary(instanceId: string): BackupDeleteSummary {
        const list = snapshots[instanceId] ?? []
        return {
            snapshot_count: list.length,
            logical_size: list.reduce((sum, entry) => sum + entry.snapshot.logical_size, 0),
        }
    }

    // A deleted instance's backups are deleted with it.
    const scope = effectScope(true)
    scope.run(() => {
        watch(
            () => state.instances.instances.value.map((instance) => instance.id),
            (ids) => {
                const known = new Set([
                    ...Object.keys(snapshots),
                    ...Object.keys(configs),
                    ...Object.keys(operations),
                ])
                for (const id of known) {
                    if (ids.includes(id)) continue
                    Reflect.deleteProperty(snapshots, id)
                    Reflect.deleteProperty(configs, id)
                    for (const operation of operations[id] ?? []) {
                        runners.get(operation.id)?.()
                        runners.delete(operation.id)
                    }
                    Reflect.deleteProperty(operations, id)
                }
            },
        )
    })

    function dispose() {
        scope.stop()
        for (const stop of runners.values()) stop()
        runners.clear()
        listeners.clear()
    }

    return {
        value: {
            configFor,
            enable,
            updateExclusions,
            disable,
            start,
            cancel,
            list,
            listOperations,
            startRestorePreview,
            restore,
            remove,
            deleteSummary,
            listen,
        },
        dispose,
    }
}

export type InstanceBackups = ReturnType<typeof createBackups>['value']

export function useInstanceBackups(): InstanceBackups {
    return usePreviewDomain('instance-settings:backups', createBackups)
}
