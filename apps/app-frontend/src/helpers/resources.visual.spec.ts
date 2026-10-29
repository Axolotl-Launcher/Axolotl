import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'

import {
    resourceExportScan,
    resourceGet,
    type ResourceOperationProgress,
    resourceRevisePlan,
    sourceResource,
} from './resources'

vi.mock('@tauri-apps/api/core', () => ({ Channel: vi.fn(), invoke: vi.fn() }))

function pending<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

it('rejects as soon as the request is aborted and asks the native side to stop it', async () => {
    vi.mocked(invoke).mockReset()
    const request = pending<unknown>()
    vi.mocked(invoke).mockImplementation((command) =>
        command === 'plugin:axolotl-resources|resource_get'
            ? request.promise
            : Promise.resolve(undefined),
    )
    const controller = new AbortController()
    const result = resourceGet(
        sourceResource('modrinth', 'project'),
        undefined,
        undefined,
        controller.signal,
    )
    controller.abort()
    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
    const requestId = vi.mocked(invoke).mock.calls[0][1]?.requestId
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('plugin:axolotl-resources|resource_cancel', {
        requestId,
    })
})

it('passes on only newer progress of its own request', async () => {
    vi.mocked(invoke).mockReset()
    vi.mocked(invoke).mockReturnValue(new Promise(() => {}))
    const seen: number[] = []
    void resourceRevisePlan('plan', 1, [], undefined, (progress) => seen.push(progress.sequence))
    const args = vi.mocked(invoke).mock.calls[0][1] as {
        requestId: string
        onProgress: { onmessage: (value: ResourceOperationProgress) => void }
    }
    const event = (requestId: string, sequence: number) =>
        ({ requestId, sequence, kind: 'phase', phase: 'validating' }) as ResourceOperationProgress
    args.onProgress.onmessage(event(args.requestId, 1))
    args.onProgress.onmessage(event(args.requestId, 1))
    args.onProgress.onmessage(event('another-request', 2))
    args.onProgress.onmessage(event(args.requestId, 3))
    args.onProgress.onmessage(event(args.requestId, 2))
    expect(seen).toEqual([1, 3])
})

it('closes the session of an export scan that finishes after it was aborted', async () => {
    vi.mocked(invoke).mockReset()
    const scan = pending<unknown>()
    vi.mocked(invoke).mockImplementation((command) =>
        command === 'plugin:axolotl-resources|resource_export_scan'
            ? scan.promise
            : Promise.resolve(undefined),
    )
    const controller = new AbortController()
    const result = resourceExportScan('instance', controller.signal)
    controller.abort()
    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
        'plugin:axolotl-resources|resource_export_close',
        expect.anything(),
    )
    scan.resolve({ sessionId: 'late-session' })
    await vi.waitFor(() =>
        expect(vi.mocked(invoke)).toHaveBeenCalledWith(
            'plugin:axolotl-resources|resource_export_close',
            { sessionId: 'late-session' },
        ),
    )
})
