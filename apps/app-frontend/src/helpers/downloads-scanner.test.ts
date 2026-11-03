import { expect, it } from 'vitest'

import {
    createDownloadsScanLoop,
    createDownloadsScannerPresentationState,
    getMissingContentScannerSettings,
    reduceDownloadsScannerPresentation,
    setMissingContentScannerSettings,
} from './downloads-scanner.ts'

function memoryStorage(initial?: string) {
    let value = initial ?? null
    return {
        getItem: () => value,
        setItem: (_key: string, next: string) => {
            value = next
        },
    }
}

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((complete) => {
        resolve = complete
    })
    return { promise, resolve }
}

it('close and reopen ignores stale scan results', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const results: string[] = []
    let scanCount = 0
    const loop = createDownloadsScanLoop({
        scan: () => (scanCount++ === 0 ? first.promise : second.promise),
        onResult: (result) => results.push(result),
        schedule: () => undefined,
        cancelSchedule: () => undefined,
    })

    loop.start()
    const staleRun = loop.runNow()
    loop.stop()
    loop.start()
    first.resolve('stale')
    await staleRun
    expect(results).toEqual([])

    const currentRun = loop.runNow()
    second.resolve('current')
    await currentRun
    expect(results).toEqual(['current'])
    loop.stop()
})

it('one scan stays in flight at a time', async () => {
    const pending = deferred<string>()
    let calls = 0
    const loop = createDownloadsScanLoop({
        scan: () => {
            calls += 1
            return pending.promise
        },
        onResult: () => undefined,
        schedule: () => undefined,
        cancelSchedule: () => undefined,
    })

    loop.start()
    const firstRun = loop.runNow()
    await loop.runNow()
    expect(calls).toBe(1)
    pending.resolve('done')
    await firstRun
    loop.stop()
})

it('empty scan activity keeps monitoring presentation stable', async () => {
    let state = createDownloadsScannerPresentationState()
    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: [],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })
    const scan = deferred<string>()
    const observedPhases: string[] = []
    const loop = createDownloadsScanLoop({
        scan: () => scan.promise,
        onResult: () => {
            state = reduceDownloadsScannerPresentation(state, {
                type: 'scan_result',
                downloadDirectory: 'C:\\Downloads',
                importedItemIds: [],
                rejectedItemIds: [],
                pendingCandidates: 0,
                hasErrors: false,
                items: [],
            })
        },
        onScanningChange: () => observedPhases.push(state.phase),
        schedule: () => undefined,
        cancelSchedule: () => undefined,
    })

    loop.start()
    const running = loop.runNow()
    expect(state.phase).toBe('monitoring')
    scan.resolve('done')
    await running
    expect(state.phase).toBe('monitoring')
    expect(observedPhases.every((phase) => phase === 'monitoring')).toBeTruthy()
    loop.stop()
})

it('empty interval scan keeps an unchanged same-name rejection visible', () => {
    let state = createDownloadsScannerPresentationState()
    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: ['mods/example.jar'],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })
    expect(state.phase).toBe('rejected')

    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: ['mods/example.jar'],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })
    expect(state.phase).toBe('rejected')
    expect(state.rejectedItemIds).toEqual(['mods/example.jar'])
})

it('candidate progresses from stability wait through verification and import', () => {
    let state = createDownloadsScannerPresentationState()
    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: [],
        pendingCandidates: 1,
        hasErrors: false,
        items: [],
    })
    expect(state.phase).toBe('waiting_for_stability')
    expect(state.rejectedItemIds).toEqual([])

    state = reduceDownloadsScannerPresentation(state, {
        type: 'items_updated',
        items: [{ id: 'mods/example.jar', status: 'verifying' }],
    })
    expect(state.phase).toBe('verifying')
    expect(state.verifyingItemIds).toEqual(['mods/example.jar'])
    state = reduceDownloadsScannerPresentation(state, {
        type: 'items_updated',
        items: [{ id: 'mods/example.jar', status: 'writing' }],
    })
    expect(state.phase).toBe('importing')

    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: ['mods/example.jar'],
        rejectedItemIds: [],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })
    expect(state.phase).toBe('imported')
    expect(state.rejectedItemIds).toEqual([])
    expect(state.verifyingItemIds).toEqual([])
    expect(state.importedCount).toBe(1)
})

it('presentation keeps multiple concurrent candidate verifications visible', () => {
    let state = createDownloadsScannerPresentationState()
    state = reduceDownloadsScannerPresentation(state, {
        type: 'items_updated',
        items: [
            { id: 'mods/one.jar', status: 'verifying' },
            { id: 'mods/two.jar', status: 'verifying' },
        ],
    })

    expect(state.phase).toBe('verifying')
    expect(state.verifyingItemIds).toEqual(['mods/one.jar', 'mods/two.jar'])
})

it('rejected candidate disappearance returns presentation to monitoring', () => {
    let state = createDownloadsScannerPresentationState()
    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: ['mods/example.jar'],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })
    expect(state.phase).toBe('rejected')

    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: [],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })
    expect(state.phase).toBe('monitoring')
    expect(state.rejectedItemIds).toEqual([])
})

it('presentation reset drops previous modal session result', () => {
    let state = createDownloadsScannerPresentationState()
    state = reduceDownloadsScannerPresentation(state, {
        type: 'scan_result',
        downloadDirectory: 'C:\\Downloads',
        importedItemIds: [],
        rejectedItemIds: ['mods/example.jar'],
        pendingCandidates: 0,
        hasErrors: false,
        items: [],
    })

    state = reduceDownloadsScannerPresentation(state, { type: 'reset' })
    expect(state.phase).toBe('idle')
    expect(state.downloadDirectory).toBe(null)
    expect(state.rejectedItemIds).toEqual([])
})

it('missing-content scanner settings default enabled and persist a custom folder', () => {
    const storage = memoryStorage()
    expect(getMissingContentScannerSettings(storage)).toEqual({
        enabled: true,
        directory: null,
    })

    setMissingContentScannerSettings({ enabled: false, directory: 'D:\\Modpack Imports' }, storage)
    expect(getMissingContentScannerSettings(storage)).toEqual({
        enabled: false,
        directory: 'D:\\Modpack Imports',
    })
})

it('invalid scanner settings fall back safely', () => {
    const storage = memoryStorage('{not-json')
    expect(getMissingContentScannerSettings(storage)).toEqual({
        enabled: true,
        directory: null,
    })
})
