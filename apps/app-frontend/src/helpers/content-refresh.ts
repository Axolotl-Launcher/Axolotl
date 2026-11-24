import type { ContentItem } from '@modrinth/ui'
import { replaceEqualDeep } from '@tanstack/vue-query'
import { listen } from '@tauri-apps/api/event'

import { invoke } from '@/helpers/invoke'

import type { InstanceContentSnapshot, InstanceContentSnapshotItem } from './instance'
import type { ResourceArtifact, SourceReleaseRef } from './resources'

export type ContentRefreshMode = 'normal' | 'force_remote' | 'retry_failed'
export interface ContentRefreshTask {
    id: string
    mode: ContentRefreshMode
    running: boolean
    scanned: number
    checked: number
    failed: number
    unresolved: number
    errors: string[]
}
export interface ContentRefreshState {
    snapshot: InstanceContentSnapshot
    task: ContentRefreshTask | null
    sequence: number
}
export type ContentRefreshPatch =
    | { kind: 'pack'; pack: InstanceContentSnapshot['pack'] }
    | { kind: 'content'; snapshot: InstanceContentSnapshot }
    | {
          kind: 'entry'
          entryId: string
          fileId: string
          sha1: string
          artifactRef?: ResourceArtifact['reference']
          presentation?: ContentItem
          update?: {
              candidate: SourceReleaseRef | null
              failed: boolean
              unresolved: boolean
              error?: string | null
          }
      }
    | { kind: 'progress' }
export type ContentRefreshEvent =
    | { kind: 'resync' }
    | {
          instanceId: string
          taskId: string
          sequence: number
          patch: ContentRefreshPatch
          task: ContentRefreshTask
      }
type EntryRefreshPatch = Extract<ContentRefreshPatch, { kind: 'entry' }>

export function resourceStartContentRefresh(instanceId: string, mode: ContentRefreshMode) {
    return invoke<ContentRefreshTask>('plugin:axolotl-resources|resource_start_content_refresh', {
        instanceId,
        mode,
    })
}

export function resourceGetContentRefreshState(instanceId: string) {
    return invoke<ContentRefreshState>(
        'plugin:axolotl-resources|resource_get_content_refresh_state',
        {
            instanceId,
        },
    )
}

export interface ContentRefreshTransport {
    listen: (callback: (event: ContentRefreshEvent) => void) => Promise<() => void>
    read: (instanceId: string) => Promise<ContentRefreshState>
    start: (instanceId: string, mode: ContentRefreshMode) => Promise<ContentRefreshTask>
    schedule: (callback: () => void) => () => void
}

export function listenContentRefresh(callback: (event: ContentRefreshEvent) => void) {
    return listen<ContentRefreshEvent>('resource_content_refresh', (event) =>
        callback(event.payload),
    )
}

const nativeTransport: ContentRefreshTransport = {
    listen: listenContentRefresh,
    read: resourceGetContentRefreshState,
    start: resourceStartContentRefresh,
    schedule: (callback) => {
        const frame = requestAnimationFrame(callback)
        return () => cancelAnimationFrame(frame)
    },
}

/** A subscription owns presentation only; disposing never cancels the backend task. */
export function createContentRefreshSubscription(
    instanceId: string,
    onChange: (state: ContentRefreshState, source: 'read' | 'stream') => void,
    onError: (error: unknown) => void,
    transport: ContentRefreshTransport = nativeTransport,
) {
    let state: ContentRefreshState | undefined
    let disposed = false
    let initializing = true
    let unlisten: (() => void) | undefined
    let cancelFrame: (() => void) | undefined
    let pending: ContentRefreshEvent[] = []
    let reading: Promise<ContentRefreshState | undefined> | undefined
    let flushing = false

    async function reload() {
        if (reading) return reading
        reading = (async () => {
            const next = await transport.read(instanceId)
            if (!disposed) {
                state = { ...next, snapshot: shareContentSnapshot(state?.snapshot, next.snapshot) }
                onChange(state, 'read')
                return state
            }
        })().finally(() => {
            reading = undefined
        })
        return reading
    }

    function schedule() {
        if (disposed || initializing || flushing || cancelFrame || !pending.length) return
        cancelFrame = transport.schedule(() => {
            cancelFrame = undefined
            if (flushing) return
            flushing = true
            void flush()
                .catch(onError)
                .finally(() => {
                    flushing = false
                    schedule()
                })
        })
    }

    async function flush() {
        if (disposed || initializing) return
        if (reading) {
            await reading
            schedule()
            return
        }
        const events = pending
        pending = []
        const resync = events.some((event) => 'kind' in event && event.kind === 'resync')
        const ordered = events
            .filter(
                (event): event is Exclude<ContentRefreshEvent, { kind: 'resync' }> =>
                    'sequence' in event && event.instanceId === instanceId,
            )
            .sort((left, right) => left.sequence - right.sequence)
        let changed = false
        const entryPatches: EntryRefreshPatch[] = []
        if (resync || !state) await reload()
        for (let index = 0; index < ordered.length; index++) {
            const event = ordered[index]
            if (!state || disposed) break
            if (event.sequence <= state.sequence) continue
            if (event.sequence !== state.sequence + 1) {
                const snapshot = applyEntryPatches(state.snapshot, entryPatches)
                state = { ...state, snapshot }
                entryPatches.length = 0
                await reload()
                if (!state || disposed || event.sequence <= state.sequence) continue
                if (event.sequence !== state.sequence + 1) {
                    // A fresh state is authoritative; keep later events until it catches up.
                    pending.push(...ordered.slice(index))
                    break
                }
            }
            const patch = event.patch
            if (patch.kind === 'entry' || patch.kind === 'progress') {
                if (patch.kind === 'entry') entryPatches.push(patch)
                state = { ...state, sequence: event.sequence, task: event.task }
            } else {
                const snapshot = applyEntryPatches(state.snapshot, entryPatches)
                entryPatches.length = 0
                state = applyContentRefreshEvent({ ...state, snapshot }, event)
            }
            changed = true
        }
        if (!disposed && state && changed) {
            const snapshot = applyEntryPatches(state.snapshot, entryPatches)
            state = { ...state, snapshot }
            onChange(state, 'stream')
        }
        schedule()
    }

    async function connect(initialState?: ContentRefreshState) {
        state = initialState
        unlisten = await transport.listen((event) => {
            if ('instanceId' in event && event.instanceId !== instanceId) return
            pending.push(event)
            schedule()
        })
        if (disposed) {
            unlisten()
            return
        }
        if (!state) await reload()
        initializing = false
        schedule()
    }

    async function start(mode: ContentRefreshMode = 'normal') {
        const task = await transport.start(instanceId, mode)
        if (!disposed && state && state.task?.id !== task.id) {
            state = { ...state, task }
            onChange(state, 'stream')
        }
        return task
    }

    function dispose() {
        disposed = true
        pending = []
        cancelFrame?.()
        unlisten?.()
    }

    return { connect, start, reload, dispose }
}

function contentSnapshotItemKey(item: InstanceContentSnapshotItem) {
    if (item.memberId) return `member:${item.memberId}`
    if (item.entryId) return `entry:${item.entryId}`
    if (item.fileId) return `file:${item.fileId}`
    return `path:${item.expectedRelativePath}`
}

export function shareContentSnapshot(
    previous: InstanceContentSnapshot | undefined,
    incoming: InstanceContentSnapshot,
): InstanceContentSnapshot {
    if (!previous || previous.instanceId !== incoming.instanceId || previous === incoming)
        return incoming

    const { items: previousItems, ...previousMetadata } = previous
    const { items: incomingItems, ...incomingMetadata } = incoming
    const byId = new Map(previousItems.map((item) => [contentSnapshotItemKey(item), item]))
    const items = incomingItems.map((item) =>
        replaceEqualDeep(byId.get(contentSnapshotItemKey(item)), item),
    )
    const sameItems =
        items.length === previousItems.length &&
        items.every((item, index) => item === previousItems[index])
    const metadata = replaceEqualDeep(previousMetadata, incomingMetadata)
    return sameItems && metadata === previousMetadata
        ? previous
        : { ...metadata, items: sameItems ? previousItems : items }
}

export function applyContentRefreshEvent(
    state: ContentRefreshState,
    event: Exclude<ContentRefreshEvent, { kind: 'resync' }>,
): ContentRefreshState {
    let snapshot = state.snapshot
    const patch = event.patch
    if (patch.kind === 'pack') {
        snapshot = { ...snapshot, pack: patch.pack }
    } else if (patch.kind === 'content') {
        if (patch.snapshot.revision >= snapshot.revision) {
            snapshot = shareContentSnapshot(snapshot, patch.snapshot)
        }
    } else if (patch.kind === 'entry') {
        snapshot = applyEntryPatches(snapshot, [patch])
    }
    return { snapshot, sequence: event.sequence, task: event.task }
}

function applyEntryPatches(
    snapshot: InstanceContentSnapshot,
    patches: readonly EntryRefreshPatch[],
): InstanceContentSnapshot {
    if (!patches.length) return snapshot

    const byEntry = new Map<string, EntryRefreshPatch[]>()
    for (const patch of patches) {
        const existing = byEntry.get(patch.entryId)
        if (existing) existing.push(patch)
        else byEntry.set(patch.entryId, [patch])
    }

    const matched = new Set<EntryRefreshPatch>()
    const items = snapshot.items.map((item) => {
        const entryId = item.entryId
        const matchingEntry = entryId == null ? undefined : byEntry.get(entryId)
        if (!matchingEntry) return item
        let next = item
        for (const patch of matchingEntry) {
            if (item.fileId !== patch.fileId || item.content?.id !== patch.sha1) continue
            matched.add(patch)
            next = applyEntryPatch(next, patch)
        }
        return next === item ? item : replaceEqualDeep(item, next)
    })
    if (!matched.size) return snapshot

    const warningUpdates = new Map<string, string | undefined>()
    for (const patch of patches) {
        const update = patch.update
        if (!matched.has(patch) || !update || (update.failed && update.error === undefined))
            continue
        warningUpdates.delete(patch.entryId)
        warningUpdates.set(patch.entryId, update.failed ? (update.error ?? undefined) : undefined)
    }
    let warnings = snapshot.warnings
    if (warningUpdates.size) {
        warnings = warnings.filter(
            (warning) =>
                warning.code !== 'resource_update_unavailable' ||
                warning.entryId == null ||
                !warningUpdates.has(warning.entryId),
        )
        for (const [entryId, message] of warningUpdates) {
            if (message) {
                warnings.push({
                    entryId,
                    code: 'resource_update_unavailable',
                    message,
                    provider: null,
                })
            }
        }
    }
    return { ...snapshot, items, warnings }
}

function applyEntryPatch(item: InstanceContentSnapshotItem, patch: EntryRefreshPatch) {
    const next = { ...item, capabilities: { ...item.capabilities } }
    if (patch.artifactRef) next.artifactRef = patch.artifactRef
    if (patch.presentation && next.content) {
        next.content = {
            ...next.content,
            project: patch.presentation.project,
            version: patch.presentation.version,
            owner: patch.presentation.owner ?? next.content.owner,
            environment: patch.presentation.environment,
            loader: patch.presentation.loader,
            external: patch.presentation.external,
        }
    }
    if (patch.update) {
        next.resourceUpdate = patch.update.candidate
        next.capabilities.canUpdate =
            !!patch.update.candidate && next.materializationState === 'present'
    }
    return next
}
