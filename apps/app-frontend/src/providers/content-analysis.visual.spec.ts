import { expect, it, vi } from 'vitest'

import { RESOURCE_ANALYSIS_TIMEOUT_MS } from '@/helpers/resource-analysis-request'
import {
    resourceAnalysisClose,
    resourceAnalysisOpen,
    resourceAnalysisUpdate,
    type ResourceOperationPlan,
    type ResourceOperationProgress,
    resourcePrepareOperation,
    type ResourceSelection,
} from '@/helpers/resources'

import { createContentAnalysis } from './content-analysis'

vi.mock('@/helpers/resources', () => ({
    resourceAnalysisOpen: vi.fn().mockResolvedValue('session'),
    resourceAnalysisClose: vi.fn().mockResolvedValue(undefined),
    resourceAnalysisUpdate: vi.fn(),
    resourcePrepareOperation: vi.fn(),
}))

const plan = { id: 'prepared', nodes: [] } as unknown as ResourceOperationPlan
function input(project = 'first', instanceId = 'instance') {
    return {
        instanceId,
        context: '1.21.1/neoforge',
        installDependencies: true,
        selections: [
            {
                id: project,
                resource: { kind: 'source', project: { source: 'modrinth', projectId: project } },
                release: { kind: 'automatic' },
                resourceType: 'mod',
                scope: { kind: 'instance' },
            },
        ] as ResourceSelection[],
    }
}
const tick = async () => {
    for (let step = 0; step < 16; step++) await Promise.resolve()
}

it('replays only the nodes still shown and the latest stage, and ignores an older selection', async () => {
    vi.clearAllMocks()
    let report!: (value: ResourceOperationProgress) => void
    vi.mocked(resourceAnalysisUpdate).mockImplementationOnce((...args) => {
        report = args[5]
        return new Promise(() => {})
    })
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    const send = (value: object) =>
        report({ selectionRevision: 1, ...value } as ResourceOperationProgress)
    send({ kind: 'phase', phase: 'refreshingContent' })
    send({ kind: 'node', node: { id: 'kept' } })
    send({ kind: 'node', node: { id: 'removed' } })
    send({ kind: 'nodeRemoved', nodeId: 'removed' })
    send({ kind: 'phase', phase: 'prefetching' })
    send({ kind: 'node', selectionRevision: 0, node: { id: 'older' } })
    const onProgress = vi.fn()
    const prepared = cart.review()!.prepare(new AbortController().signal, onProgress)
    await tick()
    expect(
        onProgress.mock.calls.map(([value]: [ResourceOperationProgress]) =>
            value.kind === 'node'
                ? value.node.id
                : value.kind === 'phase'
                  ? value.phase
                  : value.kind,
        ),
    ).toEqual(['kept', 'prefetching'])
    cart.clear()
    await expect(prepared).rejects.toMatchObject({ name: 'AbortError' })
})

it('replays the current round first and keeps names a re-sent node lacks', async () => {
    vi.clearAllMocks()
    let report!: (value: ResourceOperationProgress) => void
    vi.mocked(resourceAnalysisUpdate).mockImplementationOnce((...args) => {
        report = args[5]
        return new Promise(() => {})
    })
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    const send = (value: object) =>
        report({ selectionRevision: 1, ...value } as ResourceOperationProgress)
    const metadata = { title: 'Named', iconUrl: null, summary: '' }
    send({ kind: 'roundStarted', round: 0 })
    send({ kind: 'node', node: { id: 'merged' }, metadata })
    send({ kind: 'roundStarted', round: 1 })
    send({ kind: 'phase', phase: 'prefetching' })
    send({ kind: 'node', node: { id: 'merged', state: 'conflict' }, metadata: null })
    const onProgress = vi.fn()
    const prepared = cart.review()!.prepare(new AbortController().signal, onProgress)
    await tick()
    expect(onProgress.mock.calls.map(([value]) => value)).toMatchObject([
        { kind: 'roundStarted', round: 1 },
        { kind: 'node', node: { id: 'merged', state: 'conflict' }, metadata },
    ])
    cart.clear()
    await expect(prepared).rejects.toMatchObject({ name: 'AbortError' })
})

it('starts on addition and review reuses the running analysis and its progress', async () => {
    vi.clearAllMocks()
    let finish!: (value: ResourceOperationPlan) => void
    let report!: (value: ResourceOperationProgress) => void
    vi.mocked(resourceAnalysisUpdate).mockImplementationOnce((...args) => {
        report = args[5]
        return new Promise((resolve) => {
            finish = resolve
        })
    })
    vi.mocked(resourcePrepareOperation).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    const value = input()
    cart.update(value)
    await tick()
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(1)
    const progress = {
        kind: 'node',
        selectionRevision: 1,
        node: { id: 'first' },
    } as ResourceOperationProgress
    report(progress)
    const onProgress = vi.fn()
    const prepared = cart.review()!.prepare(new AbortController().signal, onProgress)
    await tick()
    expect(onProgress).toHaveBeenCalledWith(progress)
    expect(resourcePrepareOperation).not.toHaveBeenCalled()
    finish(plan)
    await expect(prepared).resolves.toBe(plan)
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(1)
    expect(resourcePrepareOperation).toHaveBeenCalledWith(
        'instance',
        'install',
        value.selections,
        true,
        expect.any(AbortSignal),
        undefined,
        undefined,
        expect.any(Function),
        true,
        { sessionId: 'session', selectionRevision: 1 },
    )
    cart.clear()
})

it('changing selections cancels stale work and keeps the same session for incremental analysis', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceAnalysisUpdate).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    const obsolete = cart.review()!
    const signal = vi.mocked(resourceAnalysisUpdate).mock.calls[0][4]
    cart.update(input('second'))
    await tick()
    expect(signal.aborted).toBe(true)
    await expect(obsolete.prepare(new AbortController().signal, vi.fn())).rejects.toThrow()
    expect(resourcePrepareOperation).not.toHaveBeenCalled()
    expect(resourceAnalysisOpen).toHaveBeenCalledTimes(1)
    expect(vi.mocked(resourceAnalysisUpdate).mock.calls[1][1]).toBe(2)
    cart.clear()
})

it('closing a review stops waiting without canceling the cart, and reopening reuses it', async () => {
    vi.clearAllMocks()
    let finish!: (value: ResourceOperationPlan) => void
    vi.mocked(resourceAnalysisUpdate).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    vi.mocked(resourcePrepareOperation).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    const controller = new AbortController()
    const waiting = cart.review()!.prepare(controller.signal, vi.fn())
    controller.abort()
    await expect(waiting).rejects.toThrow()
    expect(vi.mocked(resourceAnalysisUpdate).mock.calls[0][4].aborted).toBe(false)
    finish(plan)
    await expect(cart.review()!.prepare(new AbortController().signal, vi.fn())).resolves.toBe(plan)
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(1)
    cart.clear()
})

it('clearing or switching instance closes even an opening session and cannot promote late results', async () => {
    vi.clearAllMocks()
    let finish!: (value: string) => void
    vi.mocked(resourceAnalysisOpen).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    vi.mocked(resourceAnalysisUpdate).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    cart.update(input())
    const obsolete = cart.review()!
    cart.update(input('second', 'another'))
    finish('old-session')
    await tick()
    await expect(obsolete.prepare(new AbortController().signal, vi.fn())).rejects.toThrow()
    expect(resourceAnalysisClose).toHaveBeenCalledWith('old-session')
    expect(resourcePrepareOperation).not.toHaveBeenCalled()
    expect(
        vi.mocked(resourceAnalysisUpdate).mock.calls.every((call) => call[2][0].id === 'second'),
    ).toBe(true)
    cart.clear()
})

it('a failed background request is retried when opening the preview without dropping selections', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceAnalysisUpdate)
        .mockResolvedValue(plan)
        .mockRejectedValueOnce(new Error('offline'))
    vi.mocked(resourcePrepareOperation).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    const value = input()
    cart.update(value)
    await tick()
    await expect(cart.review()!.prepare(new AbortController().signal, vi.fn())).resolves.toBe(plan)
    expect(vi.mocked(resourcePrepareOperation).mock.calls[0][2]).toEqual(value.selections)
    cart.clear()
})

it('canceling stops the cart analysis, closes its session and does not restart the same selection', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceAnalysisUpdate).mockImplementation(() => new Promise(() => {}))
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    const review = cart.review()!
    const waiting = review.prepare(new AbortController().signal, vi.fn())
    review.cancel()
    await expect(waiting).rejects.toThrow()
    expect(review.signal.aborted).toBe(true)
    expect(vi.mocked(resourceAnalysisUpdate).mock.calls[0][4].aborted).toBe(true)
    await tick()
    expect(resourceAnalysisClose).toHaveBeenCalledWith('session')
    cart.update({ ...input(), context: 'a download finished' })
    await tick()
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(1)
    cart.update(input('second'))
    await tick()
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(2)
    cart.clear()
})

it('opening a review after a cancel starts a new analysis of the kept selection', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceAnalysisUpdate)
        .mockImplementationOnce(() => new Promise(() => {}))
        .mockResolvedValueOnce(plan)
    vi.mocked(resourcePrepareOperation).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    const value = input()
    cart.update(value)
    await tick()
    cart.cancel()
    const review = cart.review()!
    expect(review.signal.aborted).toBe(false)
    await expect(review.prepare(new AbortController().signal, vi.fn())).resolves.toBe(plan)
    expect(resourceAnalysisOpen).toHaveBeenCalledTimes(2)
    expect(vi.mocked(resourceAnalysisUpdate).mock.calls[1][2]).toEqual(value.selections)
    cart.clear()
})

it('a finished analysis survives a review cancel', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceAnalysisUpdate).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    await tick()
    const review = cart.review()!
    review.cancel()
    expect(review.signal.aborted).toBe(false)
    expect(resourceAnalysisClose).not.toHaveBeenCalled()
    cart.clear()
})

it('replacing the analysis while a review waits is not recorded as a user cancel', async () => {
    vi.clearAllMocks()
    vi.mocked(resourceAnalysisUpdate).mockImplementation(() => new Promise(() => {}))
    const cart = createContentAnalysis()
    cart.update(input())
    await tick()
    const review = cart.review()!
    // The preview cancels its borrowed analysis when that analysis is aborted.
    review.signal.addEventListener('abort', () => review.cancel())
    void review.prepare(new AbortController().signal, vi.fn()).catch(() => {})
    cart.update({ ...input(), context: 'a download finished' })
    await tick()
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(2)
    cart.update({ ...input(), context: 'another download finished' })
    await tick()
    expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(3)
    cart.clear()
})

it('times out an opening session without invalidating the review and closes its late handle', async () => {
    vi.clearAllMocks()
    vi.useFakeTimers()
    let opened!: (value: string) => void
    vi.mocked(resourceAnalysisOpen)
        .mockImplementationOnce(() => new Promise((resolve) => (opened = resolve)))
        .mockResolvedValue('retry-session')
    vi.mocked(resourceAnalysisUpdate).mockResolvedValue(plan)
    vi.mocked(resourcePrepareOperation).mockResolvedValue(plan)
    const cart = createContentAnalysis()
    try {
        cart.update(input())
        const review = cart.review()!
        const waiting = review.prepare(new AbortController().signal, vi.fn())
        const finished = expect(waiting).rejects.toMatchObject({ name: 'TimeoutError' })
        await vi.advanceTimersByTimeAsync(RESOURCE_ANALYSIS_TIMEOUT_MS)
        await finished
        expect(review.signal.aborted).toBe(false)
        await expect(review.prepare(new AbortController().signal, vi.fn())).resolves.toBe(plan)
        expect(resourceAnalysisOpen).toHaveBeenCalledTimes(2)
        expect(resourcePrepareOperation).toHaveBeenLastCalledWith(
            'instance',
            'install',
            input().selections,
            true,
            expect.any(AbortSignal),
            undefined,
            undefined,
            expect.any(Function),
            true,
            { sessionId: 'retry-session', selectionRevision: 2 },
        )
        opened('late-session')
        await vi.advanceTimersByTimeAsync(0)
        expect(resourceAnalysisClose).toHaveBeenCalledWith('late-session')
        expect(resourceAnalysisUpdate).toHaveBeenCalledTimes(1)
    } finally {
        cart.clear()
        vi.useRealTimers()
    }
})

it.each(['failure', 'timeout'])(
    'retries a %s in native preparation through the same review with a fresh session',
    async (kind) => {
        vi.clearAllMocks()
        vi.useFakeTimers()
        vi.mocked(resourceAnalysisOpen)
            .mockResolvedValueOnce('old-session')
            .mockResolvedValueOnce('new-session')
        vi.mocked(resourceAnalysisUpdate).mockResolvedValue(plan)
        let late!: (value: ResourceOperationPlan) => void
        let report!: (value: ResourceOperationProgress) => void
        vi.mocked(resourcePrepareOperation)
            .mockImplementationOnce((...args) => {
                report = args[7]!
                return kind === 'failure'
                    ? Promise.reject(new Error('offline'))
                    : new Promise((resolve) => (late = resolve))
            })
            .mockResolvedValue(plan)
        const cart = createContentAnalysis()
        try {
            cart.update(input())
            await tick()
            const review = cart.review()!
            const progress = vi.fn()
            const waiting = review.prepare(new AbortController().signal, progress)
            const finished = expect(waiting).rejects.toMatchObject({
                name: kind === 'timeout' ? 'TimeoutError' : 'Error',
            })
            await vi.advanceTimersByTimeAsync(kind === 'timeout' ? RESOURCE_ANALYSIS_TIMEOUT_MS : 0)
            await finished
            expect(review.signal.aborted).toBe(false)
            expect(vi.mocked(resourcePrepareOperation).mock.calls[0][4]?.aborted).toBe(true)
            expect(resourceAnalysisClose).toHaveBeenCalledWith('old-session')
            await expect(review.prepare(new AbortController().signal, progress)).resolves.toBe(plan)
            expect(resourceAnalysisOpen).toHaveBeenCalledTimes(2)
            expect(vi.mocked(resourcePrepareOperation).mock.calls[1][9]).toEqual({
                sessionId: 'new-session',
                selectionRevision: 2,
            })
            report({ kind: 'node', node: { id: 'late' } } as ResourceOperationProgress)
            late?.({ ...plan, id: 'late-plan' })
            await vi.advanceTimersByTimeAsync(0)
            expect(progress).not.toHaveBeenCalled()
            expect(vi.mocked(resourceAnalysisUpdate).mock.calls[1][2]).toEqual(input().selections)
        } finally {
            cart.clear()
            vi.useRealTimers()
        }
    },
)
