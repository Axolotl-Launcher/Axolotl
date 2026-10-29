import { expect, it, vi } from 'vitest'

import {
    applyContentRefreshEvent,
    type ContentRefreshEvent,
    type ContentRefreshState,
    type ContentRefreshTransport,
    createContentRefreshSubscription,
} from './content-refresh'
import { normalizeInstanceContentSnapshot } from './instance-content'

function state(sequence = 0): ContentRefreshState {
    return {
        sequence,
        task: null,
        snapshot: {
            instanceId: 'instance',
            revision: 1,
            pack: null,
            warnings: [],
            pendingManualDownloads: [],
            items: [
                {
                    fileId: 'file',
                    entryId: 'entry',
                    memberId: null,
                    expectedRelativePath: 'mods/a.jar',
                    ownershipKind: 'user_added',
                    materializationState: 'present',
                    overrideKind: 'none',
                    required: true,
                    projectType: 'mod',
                    provider: null,
                    providerProjectId: null,
                    providerReleaseId: null,
                    dependency: null,
                    capabilities: {
                        canToggle: true,
                        canDelete: true,
                        canUpdate: false,
                        canChangeVersion: true,
                        canRestorePackDefault: false,
                    },
                    content: {
                        id: 'hash',
                        file_name: 'a.jar',
                        file_path: 'mods/a.jar',
                        size: 1,
                        enabled: true,
                        project_type: 'mod',
                    },
                },
            ],
        },
    }
}

function event(sequence: number): Exclude<ContentRefreshEvent, { kind: 'resync' }> {
    return {
        sequence,
        instanceId: 'instance',
        taskId: 'task',
        task: {
            id: 'task',
            mode: 'normal',
            running: true,
            scanned: 1,
            checked: 1,
            failed: 0,
            unresolved: 0,
            errors: [],
        },
        patch: {
            kind: 'entry',
            entryId: 'entry',
            fileId: 'file',
            sha1: 'hash',
            update: {
                candidate: {
                    project: { source: 'curseforge', projectId: '123' },
                    releaseId: '456',
                },
                failed: false,
                unresolved: false,
            },
        },
    }
}

function fixture(initial = state()) {
    let listener!: (event: ContentRefreshEvent) => void
    let frame: (() => void) | undefined
    const unlisten = vi.fn()
    const read = vi.fn(async () => initial)
    const start = vi.fn(async () => event(1).task)
    const transport: ContentRefreshTransport = {
        listen: vi.fn(async (callback) => {
            listener = callback
            return unlisten
        }),
        read,
        start,
        schedule: (callback) => {
            frame = callback
            return () => {
                frame = undefined
            }
        },
    }
    const change = vi.fn()
    const error = vi.fn()
    const subscription = createContentRefreshSubscription('instance', change, error, transport)
    return {
        subscription,
        change,
        error,
        transport,
        read,
        unlisten,
        receive: (event: ContentRefreshEvent) => listener(event),
        async flush() {
            const callback = frame
            frame = undefined
            callback?.()
            await vi.waitFor(() => expect(error).not.toHaveBeenCalled())
            await Promise.resolve()
            await Promise.resolve()
        },
    }
}

it('reconciles only the matching entry warning as checks fail and recover', () => {
    const initial = state()
    initial.snapshot.warnings = [
        { entryId: 'entry', code: 'resource_update_unavailable', message: 'old', provider: null },
        { entryId: 'other', code: 'resource_update_unavailable', message: 'other', provider: null },
        { code: 'resource_history_unavailable', message: 'history', provider: null },
    ]
    const patch = event(1)
    const failed = {
        ...patch,
        patch: {
            ...patch.patch,
            kind: 'entry' as const,
            entryId: 'entry',
            fileId: 'file',
            sha1: 'hash',
            update: { candidate: null, failed: true, unresolved: false, error: 'offline' },
        },
    }
    const updated = applyContentRefreshEvent(initial, failed)
    expect(updated.snapshot.warnings.map((warning) => warning.message)).toEqual([
        'other',
        'history',
        'offline',
    ])
    const recovered = applyContentRefreshEvent(updated, event(2))
    expect(recovered.snapshot.warnings.map((warning) => warning.message)).toEqual([
        'other',
        'history',
    ])
    const stale = { ...failed, patch: { ...failed.patch, sha1: 'old-bytes' } }
    expect(applyContentRefreshEvent(recovered, stale).snapshot).toBe(recovered.snapshot)
})

it('subscribes before reading and replays events arriving during the state request', async () => {
    const test = fixture()
    let resolve!: (state: ContentRefreshState) => void
    test.read.mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const connecting = test.subscription.connect()
    await vi.waitFor(() => expect(test.read).toHaveBeenCalledOnce())
    expect(test.transport.listen).toHaveBeenCalledOnce()
    test.receive(event(1))
    resolve(state())
    await connecting
    await test.flush()
    expect(test.change.mock.lastCall?.[0].snapshot.items[0].resourceUpdate?.releaseId).toBe('456')
    test.subscription.dispose()
})

it('sorts and deduplicates events in one frame and reloads local state for gaps', async () => {
    const test = fixture()
    await test.subscription.connect()
    test.receive(event(2))
    test.receive(event(1))
    test.receive(event(1))
    await test.flush()
    expect(test.change.mock.lastCall?.[0].sequence).toBe(2)
    expect(test.read).toHaveBeenCalledOnce()
    test.read.mockResolvedValueOnce(state(5))
    test.receive(event(5))
    await test.flush()
    await vi.waitFor(() => expect(test.read).toHaveBeenCalledTimes(2))
    expect(test.transport.start).not.toHaveBeenCalled()
    test.subscription.dispose()
})

it('disposal only unsubscribes and a late state response cannot publish', async () => {
    const test = fixture()
    let resolve!: (state: ContentRefreshState) => void
    test.read.mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const connecting = test.subscription.connect()
    await vi.waitFor(() => expect(test.read).toHaveBeenCalledOnce())
    test.subscription.dispose()
    resolve(state())
    await connecting
    expect(test.unlisten).toHaveBeenCalledOnce()
    expect(test.change).not.toHaveBeenCalled()
})

it('metadata preserves candidates while an explicit null clears them and stale files are ignored', () => {
    const first = applyContentRefreshEvent(state(), event(1))
    const metadata = event(2)
    metadata.patch = {
        kind: 'entry',
        entryId: 'entry',
        fileId: 'file',
        sha1: 'hash',
        presentation: {
            ...first.snapshot.items[0].content!,
            project: { id: 'project', title: 'Updated name', slug: 'project' },
            owner: {
                id: 'author',
                name: 'ExampleAuthor',
                type: 'user',
                avatar_url: 'https://cdn.modrinth.com/avatars/author.png',
            },
        },
    }
    const updated = applyContentRefreshEvent(first, metadata)
    expect(updated.snapshot.items[0].resourceUpdate?.releaseId).toBe('456')
    expect(updated.snapshot.items[0].content?.project?.title).toBe('Updated name')
    expect(
        normalizeInstanceContentSnapshot('instance', updated.snapshot).contentItems[0].owner?.name,
    ).toBe('ExampleAuthor')
    expect(
        normalizeInstanceContentSnapshot('instance', updated.snapshot).contentItems[0].owner
            ?.avatar_url,
    ).toBe('https://cdn.modrinth.com/avatars/author.png')
    const partial = {
        ...metadata,
        patch: {
            ...metadata.patch,
            presentation: { ...metadata.patch.presentation!, owner: null },
        },
    }
    expect(applyContentRefreshEvent(updated, partial).snapshot.items[0].content?.owner?.name).toBe(
        'ExampleAuthor',
    )
    const stale = event(3)
    if (stale.patch.kind === 'entry') stale.patch.fileId = 'old-file'
    expect(applyContentRefreshEvent(updated, stale).snapshot.items[0]).toBe(
        updated.snapshot.items[0],
    )
    const clear = event(4)
    if (clear.patch.kind === 'entry') clear.patch.update!.candidate = null
    expect(applyContentRefreshEvent(updated, clear).snapshot.items[0].capabilities.canUpdate).toBe(
        false,
    )
})

it('renders successive update counts and restores saved candidates when a new page subscribes', async () => {
    const initial = state()
    initial.snapshot.items.push({
        ...initial.snapshot.items[0],
        entryId: 'second',
        fileId: 'second-file',
        content: { ...initial.snapshot.items[0].content!, id: 'second-hash' },
    })
    const test = fixture(initial)
    const counts: number[] = []
    test.change.mockImplementation((value: ContentRefreshState) => {
        const data = normalizeInstanceContentSnapshot('instance', value.snapshot)
        counts.push(data.contentItems.filter((item) => !!item.resourceUpdate).length)
    })
    await test.subscription.connect()
    test.receive(event(1))
    await test.flush()
    expect(counts).toEqual([0, 1])
    const saved = test.change.mock.lastCall![0]
    test.subscription.dispose()
    const returned = fixture(saved)
    await returned.subscription.connect()
    expect(returned.change.mock.lastCall?.[0].snapshot.items[0].resourceUpdate?.releaseId).toBe(
        '456',
    )
    const next = event(2)
    if (next.patch.kind === 'entry') {
        next.patch.entryId = 'second'
        next.patch.fileId = 'second-file'
        next.patch.sha1 = 'second-hash'
    }
    returned.receive(next)
    await returned.flush()
    const final = normalizeInstanceContentSnapshot(
        'instance',
        returned.change.mock.lastCall![0].snapshot,
    )
    expect(final.contentItems.filter((item) => !!item.resourceUpdate)).toHaveLength(2)
    returned.subscription.dispose()
})
