import { invoke } from '@tauri-apps/api/core'
import { beforeEach, expect, it, vi } from 'vitest'

import { resourceCheckUpdates, type ResourceUpdateProgress } from './resources'

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
    Channel: class {
        onmessage?: (progress: ResourceUpdateProgress) => void
    },
}))

beforeEach(() => vi.resetAllMocks())

const emptyResult = () => ({
    entries: [],
    releases: {},
    unresolved: [],
    failures: [],
    failedEntries: [],
})

it('delivers visible-entry updates before the visible batch finishes', async () => {
    let complete!: (value: ReturnType<typeof emptyResult>) => void
    vi.mocked(invoke).mockImplementation(
        () =>
            new Promise((resolve) => {
                complete = resolve
            }),
    )
    const onProgress = vi.fn()
    let finished = false
    const pending = resourceCheckUpdates('instance', undefined, {
        entryIds: ['visible-entry'],
        onProgress,
    }).then(() => {
        finished = true
    })
    const args = vi.mocked(invoke).mock.calls[0][1] as {
        entryIds: string[]
        onProgress: { onmessage: (progress: ResourceUpdateProgress) => void }
    }
    expect(args.entryIds).toEqual(['visible-entry'])
    const progress: ResourceUpdateProgress = {
        entryId: 'visible-entry',
        release: { project: { source: 'modrinth', projectId: 'project' }, releaseId: 'new' },
        failed: false,
        unresolved: false,
    }
    args.onProgress.onmessage(progress)
    expect(onProgress).toHaveBeenCalledWith(progress)
    expect(finished).toBe(false)
    complete(emptyResult())
    await pending
})

it('ignores late progress when the instance request has been canceled', async () => {
    let complete!: (value: ReturnType<typeof emptyResult>) => void
    vi.mocked(invoke).mockImplementation((command) =>
        command.endsWith('|resource_cancel')
            ? Promise.resolve(undefined)
            : new Promise((resolve) => {
                  complete = resolve
              }),
    )
    const controller = new AbortController()
    const onProgress = vi.fn()
    const pending = resourceCheckUpdates('instance', controller.signal, { onProgress })
    const args = vi.mocked(invoke).mock.calls[0][1] as {
        onProgress: { onmessage: (progress: ResourceUpdateProgress) => void }
    }
    controller.abort()
    args.onProgress.onmessage({ entryId: 'entry', release: null, failed: false, unresolved: false })
    expect(onProgress).not.toHaveBeenCalled()
    complete(emptyResult())
    await expect(pending).rejects.toMatchObject({ name: 'AbortError' })
})

it('reserves full instance checks for callers without an entry scope', async () => {
    vi.mocked(invoke).mockResolvedValue(emptyResult())
    await resourceCheckUpdates('instance')
    expect(vi.mocked(invoke).mock.calls[0][1]).toMatchObject({ entryIds: null })
    await resourceCheckUpdates('instance', undefined, { entryIds: [] })
    expect(vi.mocked(invoke).mock.calls[1][1]).toMatchObject({ entryIds: [] })
})
