import type { ContentItem } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import type { InstanceContentSnapshot } from './instance'
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

const nativeTransport: ContentRefreshTransport = {
    listen: (callback) =>
        listen<ContentRefreshEvent>('resource_content_refresh', (event) => callback(event.payload)),
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
    onChange: (state: ContentRefreshState) => void,
    onError: (error: unknown) => void,
    transport: ContentRefreshTransport = nativeTransport,
) {
    let state: ContentRefreshState | undefined
    let disposed = false
    let initializing = true
    let unlisten: (() => void) | undefined
    let cancelFrame: (() => void) | undefined
    let pending: ContentRefreshEvent[] = []
    let reading: Promise<void> | undefined
    let flushing = false

    async function reload() {
        if (reading) return reading
        reading = (async () => {
            const next = await transport.read(instanceId)
            if (!disposed) {
                state = next
                onChange(next)
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
        if (resync || !state) await reload()
        for (let index = 0; index < ordered.length; index++) {
            const event = ordered[index]
            if (!state || disposed) break
            if (event.sequence <= state.sequence) continue
            if (event.sequence !== state.sequence + 1) {
                await reload()
                if (!state || disposed || event.sequence <= state.sequence) continue
                if (event.sequence !== state.sequence + 1) {
                    // A fresh state is authoritative; keep later events until it catches up.
                    pending.push(...ordered.slice(index))
                    break
                }
            }
            state = applyContentRefreshEvent(state, event)
            changed = true
        }
        if (!disposed && state && changed) onChange(state)
        schedule()
    }

    async function connect() {
        unlisten = await transport.listen((event) => {
            if ('instanceId' in event && event.instanceId !== instanceId) return
            pending.push(event)
            schedule()
        })
        if (disposed) {
            unlisten()
            return
        }
        await reload()
        initializing = false
        schedule()
    }

    async function start(mode: ContentRefreshMode = 'normal') {
        const task = await transport.start(instanceId, mode)
        if (!disposed && state && state.task?.id !== task.id) {
            state = { ...state, task }
            onChange(state)
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

export function applyContentRefreshEvent(
    state: ContentRefreshState,
    event: Exclude<ContentRefreshEvent, { kind: 'resync' }>,
): ContentRefreshState {
    let snapshot = state.snapshot
    const patch = event.patch
    if (patch.kind === 'pack') {
        snapshot = { ...snapshot, pack: patch.pack }
    } else if (patch.kind === 'content') {
        if (patch.snapshot.revision >= snapshot.revision) snapshot = patch.snapshot
    } else if (patch.kind === 'entry') {
        const matches = snapshot.items.some(
            (item) =>
                item.entryId === patch.entryId &&
                item.fileId === patch.fileId &&
                item.content?.id === patch.sha1,
        )
        if (!matches) return { snapshot, sequence: event.sequence, task: event.task }
        let warnings = snapshot.warnings
        if (patch.update && (!patch.update.failed || patch.update.error !== undefined)) {
            warnings = warnings.filter(
                (warning) =>
                    warning.code !== 'resource_update_unavailable' ||
                    warning.entryId !== patch.entryId,
            )
            if (patch.update.failed && patch.update.error) {
                warnings = [
                    ...warnings,
                    {
                        entryId: patch.entryId,
                        code: 'resource_update_unavailable',
                        message: patch.update.error,
                        provider: null,
                    },
                ]
            }
        }
        snapshot = {
            ...snapshot,
            warnings,
            items: snapshot.items.map((item) => {
                if (
                    item.entryId !== patch.entryId ||
                    item.fileId !== patch.fileId ||
                    item.content?.id !== patch.sha1
                )
                    return item
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
            }),
        }
    }
    return { snapshot, sequence: event.sequence, task: event.task }
}
