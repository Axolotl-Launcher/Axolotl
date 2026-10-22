import { expect, it, vi } from 'vitest'

import { clearCrashAnalysis, refreshCrashAnalysis, useCrashAnalysis } from './useCrashAnalysis'

const calls = vi.hoisted(() => ({ analyze: vi.fn() }))
vi.mock('@/helpers/logs', () => ({ analyze_crash: calls.analyze }))

it('clearing an instance invalidates pending results and does not unlock a newer request', async () => {
    let completeOld!: (value: unknown) => void
    let completeNew!: (value: unknown) => void
    calls.analyze
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    completeOld = resolve
                }),
        )
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    completeNew = resolve
                }),
        )
    const old = refreshCrashAnalysis('same-instance')
    clearCrashAnalysis('same-instance')
    const fresh = refreshCrashAnalysis('same-instance')
    completeOld({ instance_id: 'same-instance', combined_log: 'old' })
    expect(await old).toBe(null)
    const state = useCrashAnalysis('same-instance')
    expect(state.loading.value).toBe(true)
    expect(state.analysis.value).toBe(null)
    completeNew({ instance_id: 'same-instance', combined_log: 'new' })
    await fresh
    expect(state.analysis.value?.combined_log).toBe('new')
    expect(state.loading.value).toBe(false)
})
