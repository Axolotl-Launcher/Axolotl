import type { ContentItem } from '@modrinth/ui'
import { expect, it, vi } from 'vitest'

import { type InstallJobSnapshot, wait_for_install_job } from './install.ts'
import {
    confirmedContentGroupKey,
    duplicateGroupUpdateEntries,
    queueResourceEntryUpdates,
    type ResourceEntryReview,
    reviewResourceEntryChanges,
} from './resource-update-selection.ts'
import { resourcePrepareEntryOperation, resourceQueuePlan } from './resources'

vi.mock('./resources', () => ({
    resourcePrepareEntryOperation: vi.fn(),
    resourceQueuePlan: vi.fn(),
}))
vi.mock('./install.ts', () => ({ wait_for_install_job: vi.fn().mockResolvedValue({}) }))

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: unknown) => void
    const promise = new Promise<T>((success, failure) => {
        resolve = success
        reject = failure
    })
    return { promise, resolve, reject }
}

function readyUpdates() {
    vi.clearAllMocks()
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        id: 'plan',
        revision: 1,
        issues: [],
        actions: [{ entryId: 'entry' }],
    } as never)
    vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'job' } as never)
    vi.mocked(wait_for_install_job).mockResolvedValue({} as never)
}

it('prepares the next update only after the previous native job finishes in the same instance', async () => {
    readyUpdates()
    const firstFinished = deferred<InstallJobSnapshot>()
    vi.mocked(wait_for_install_job).mockReturnValueOnce(firstFinished.promise)
    const first = queueResourceEntryUpdates('serial', ['a'], vi.fn(), vi.fn())
    const second = queueResourceEntryUpdates('serial', ['b'], vi.fn(), vi.fn())
    await vi.waitFor(() => expect(wait_for_install_job).toHaveBeenCalledOnce())
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(1)
    firstFinished.resolve({} as InstallJobSnapshot)
    expect(await first).toBe(true)
    expect(await second).toBe(true)
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(2)
})

it('allows different instances in parallel and deduplicates entries until their native job finishes', async () => {
    readyUpdates()
    const firstFinished = deferred<InstallJobSnapshot>()
    vi.mocked(wait_for_install_job).mockReturnValueOnce(firstFinished.promise)
    const tracked = vi.fn()
    const first = queueResourceEntryUpdates('one', ['a', 'b'], tracked, vi.fn())
    const duplicate = queueResourceEntryUpdates('one', ['b'], tracked, vi.fn())
    const second = queueResourceEntryUpdates('two', ['a'], tracked, vi.fn())
    await expect(second).resolves.toBe(true)
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(2)
    expect(tracked).toHaveBeenCalledTimes(2)
    firstFinished.resolve({} as InstallJobSnapshot)
    await expect(first).resolves.toBe(true)
    await expect(duplicate).resolves.toBe(true)
    expect(resourceQueuePlan).toHaveBeenCalledTimes(2)
})

it('releases the instance sequence after failed preparation and failed native jobs', async () => {
    readyUpdates()
    vi.mocked(resourcePrepareEntryOperation).mockRejectedValueOnce(new Error('offline'))
    const failedPrepare = queueResourceEntryUpdates('failure', ['a'], vi.fn(), vi.fn())
    const next = queueResourceEntryUpdates('failure', ['b'], vi.fn(), vi.fn())
    await expect(failedPrepare).rejects.toThrow('offline')
    await expect(next).resolves.toBe(true)
    const finished = deferred<InstallJobSnapshot>()
    vi.mocked(wait_for_install_job).mockReturnValueOnce(finished.promise)
    const failedJob = queueResourceEntryUpdates('failure', ['c'], vi.fn(), vi.fn())
    const later = queueResourceEntryUpdates('failure', ['d'], vi.fn(), vi.fn())
    const expectedFailure = expect(failedJob).rejects.toThrow('job failed')
    await vi.waitFor(() => expect(wait_for_install_job).toHaveBeenCalledTimes(2))
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(3)
    finished.reject(new Error('job failed'))
    await expectedFailure
    await expect(later).resolves.toBe(true)
    expect(resourceQueuePlan).toHaveBeenCalledTimes(3)
})

it('rebuilds stale preparation with a bounded retry and does not retry uncertain queue failures', async () => {
    readyUpdates()
    vi.mocked(resourcePrepareEntryOperation)
        .mockRejectedValueOnce({
            error: {
                message: 'invalid resource request: instance changed while preparing the plan',
            },
        })
        .mockRejectedValueOnce(
            new Error(
                'invalid resource request: instance changed while verifying installed resources',
            ),
        )
    await expect(queueResourceEntryUpdates('retry', ['a'], vi.fn(), vi.fn())).resolves.toBe(true)
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(3)
    vi.mocked(resourcePrepareEntryOperation).mockRejectedValue(
        new Error('invalid resource request: instance changed while preparing the plan'),
    )
    await expect(queueResourceEntryUpdates('retry', ['b'], vi.fn(), vi.fn())).rejects.toThrow(
        'instance changed',
    )
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(6)
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        id: 'plan',
        revision: 1,
        issues: [],
        actions: [{}],
    } as never)
    vi.mocked(resourceQueuePlan).mockRejectedValueOnce(new Error('unknown queue response'))
    await expect(queueResourceEntryUpdates('retry', ['c'], vi.fn(), vi.fn())).rejects.toThrow(
        'unknown queue response',
    )
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(7)
    expect(resourceQueuePlan).toHaveBeenCalledTimes(2)
})

it('rebuilds a definitively unclaimed stale queue plan but never retries an already queued job', async () => {
    readyUpdates()
    const track = vi.fn()
    vi.mocked(resourceQueuePlan).mockRejectedValueOnce({
        error: {
            message: 'invalid resource request: Invalid input: instance changed after review',
        },
    })
    await expect(queueResourceEntryUpdates('queue-retry', ['a'], track, vi.fn())).resolves.toBe(
        true,
    )
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(2)
    expect(resourceQueuePlan).toHaveBeenCalledTimes(2)
    expect(track).toHaveBeenCalledOnce()
    vi.mocked(wait_for_install_job).mockRejectedValueOnce(
        new Error('instance changed after review'),
    )
    await expect(queueResourceEntryUpdates('queue-retry', ['b'], track, vi.fn())).rejects.toThrow(
        'after review',
    )
    expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(3)
    expect(resourceQueuePlan).toHaveBeenCalledTimes(3)
    expect(track).toHaveBeenCalledTimes(2)
})

it('single, selected, all and duplicate updates queue exact entries without dependency analysis or review', async () => {
    for (const entryIds of [['single'], ['a', 'b'], ['a', 'b', 'c'], ['copy', 'copy']]) {
        vi.clearAllMocks()
        vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
            id: 'direct-plan',
            revision: 3,
            issues: [],
            actions: [{ entryId: entryIds[0] }],
        } as never)
        vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'direct-job' } as never)
        const track = vi.fn()
        expect(await queueResourceEntryUpdates('instance', entryIds, track, vi.fn())).toBe(true)
        expect(resourcePrepareEntryOperation).toHaveBeenCalledExactlyOnceWith(
            'instance',
            'update',
            [...new Set(entryIds)],
            null,
            false,
            undefined,
            undefined,
            false,
        )
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('direct-plan', 3)
        expect(track).toHaveBeenCalledExactlyOnceWith({ job_id: 'direct-job' })
    }
})

it('does not queue empty or blocked updates and propagates prepare and queue failures', async () => {
    vi.clearAllMocks()
    const track = vi.fn()
    const format = vi.fn((message: { defaultMessage: string }) => message.defaultMessage)
    expect(await queueResourceEntryUpdates('instance', [], track, format)).toBe(false)
    expect(resourcePrepareEntryOperation).not.toHaveBeenCalled()
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        id: 'plan',
        revision: 1,
        actions: [{ entryId: 'entry' }],
        issues: [{ code: 'fileNotWritable', blocking: true, message: 'Access is denied' }],
    } as never)
    await expect(queueResourceEntryUpdates('instance', ['entry'], track, format)).rejects.toThrow(
        'cannot write',
    )
    expect(resourceQueuePlan).not.toHaveBeenCalled()
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        id: 'plan',
        revision: 1,
        actions: [],
        issues: [],
    } as never)
    expect(await queueResourceEntryUpdates('instance', ['entry'], track, format)).toBe(false)
    vi.mocked(resourcePrepareEntryOperation).mockRejectedValueOnce(new Error('source unavailable'))
    await expect(queueResourceEntryUpdates('instance', ['entry'], track, format)).rejects.toThrow(
        'source unavailable',
    )
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        id: 'plan',
        revision: 1,
        actions: [{ entryId: 'entry' }],
        issues: [],
    } as never)
    vi.mocked(resourceQueuePlan).mockRejectedValueOnce(new Error('stale revision'))
    await expect(queueResourceEntryUpdates('instance', ['entry'], track, format)).rejects.toThrow(
        'stale revision',
    )
    expect(track).not.toHaveBeenCalled()
})

function item(entry: string, overrides: Partial<ContentItem> = {}): ContentItem {
    return {
        id: entry,
        instanceEntryId: entry,
        instanceResourceId: 'confirmed',
        project_type: 'mod',
        file_path: `mods/${entry}.jar`,
        instanceMaterializationState: 'present',
        resourceUpdate: { project: { source: 'modrinth', projectId: 'project' }, releaseId: 'new' },
        ...overrides,
    } as ContentItem
}

it('duplicate group update includes every actual updatable entry across sources and versions', () => {
    const anchor = item('a')
    const copies = [
        anchor,
        item('b', { origin_provider: 'curseforge' }),
        item('b'),
        item('c', { resourceUpdate: null }),
        item('missing', { instanceMaterializationState: 'missing' }),
        item('other', { instanceResourceId: 'another' }),
    ]
    expect(duplicateGroupUpdateEntries(copies, anchor)).toEqual(['a', 'b'])
    expect(confirmedContentGroupKey(item('unverified', { instanceResourceId: undefined }))).toBe(
        null,
    )
})

it('world scopes and content types stay independent', () => {
    const a = item('a', { project_type: 'datapack', file_path: 'saves/one/datapacks/a.zip' })
    const b = item('b', { project_type: 'datapack', file_path: 'saves/two/datapacks/b.zip' })
    const c = item('c', {
        project_type: 'datapack',
        file_path: 'saves/one/datapacks/c.zip.disabled',
    })
    expect(duplicateGroupUpdateEntries([a, b, c, item('mod')], a)).toEqual(['a', 'c'])
    expect(confirmedContentGroupKey(item('invalid', { project_type: 'datapack' }))).toBe(null)
})

it('single, selected, all and duplicate updates retain exact entries and dependency preferences for review', async () => {
    for (const entryIds of [
        ['single'],
        ['a', 'b'],
        ['a', 'b', 'c'],
        ['copy-a', 'copy-b', 'copy-a'],
    ]) {
        for (const installDependencies of [false, true]) {
            let reviewed: ResourceEntryReview | undefined
            const tracked: string[] = []
            const result = await reviewResourceEntryChanges(
                {
                    instanceId: 'instance',
                    instanceName: 'Example',
                    entryIds,
                    release: null,
                    installDependencies,
                },
                async (input) => {
                    reviewed = input
                    expect(tracked).toEqual([])
                    return { job_id: 'reviewed' } as InstallJobSnapshot
                },
                (job) => {
                    tracked.push(job.job_id)
                },
            )
            expect(result).toBe(true)
            expect(reviewed?.entryIds).toEqual([...new Set(entryIds)])
            expect(reviewed?.installDependencies).toBe(installDependencies)
            expect(reviewed?.intent).toBe('update')
            expect(tracked).toEqual(['reviewed'])
        }
    }
})

it('cancelled and failed previews do not track a job or alter selected entries', async () => {
    const input = {
        instanceId: 'instance',
        instanceName: 'Example',
        entryIds: ['entry'],
        release: null,
        installDependencies: false,
    }
    let tracked = false
    const track = () => {
        tracked = true
    }
    expect(await reviewResourceEntryChanges(input, async () => null, track)).toBe(false)
    await expect(
        reviewResourceEntryChanges(
            input,
            async () => {
                throw new Error('source unavailable')
            },
            track,
        ),
    ).rejects.toThrow()
    expect(tracked).toBe(false)
    expect(input.entryIds).toEqual(['entry'])
})
