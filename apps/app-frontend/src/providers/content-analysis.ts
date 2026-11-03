import { resourceAnalysisRequest } from '@/helpers/resource-analysis-request'
import {
    resourceAnalysisClose,
    resourceAnalysisOpen,
    resourceAnalysisUpdate,
    type ResourceOperationPlan,
    type ResourceOperationProgress,
    resourcePrepareOperation,
    type ResourceSelection,
} from '@/helpers/resources'

export type ContentAnalysisInput = {
    instanceId: string
    context: string
    selections: ResourceSelection[]
    installDependencies: boolean
}

export type ContentAnalysisReview = {
    signal: AbortSignal
    /** Stops the cart's analysis while it is still running. The selection is kept. */
    cancel: () => void
    prepare: (
        signal: AbortSignal,
        onProgress: (progress: ResourceOperationProgress) => void,
        selections?: ResourceSelection[],
    ) => Promise<ResourceOperationPlan>
}

/** What a review opened later needs to catch up with a running analysis. */
type ProgressReplay = {
    round: ResourceOperationProgress | null
    nodes: Map<string, Extract<ResourceOperationProgress, { kind: 'node' }>>
    /** A stage reported after the last node; a node event ends it. */
    phase: ResourceOperationProgress | null
}

function remember(replay: ProgressReplay, value: ResourceOperationProgress) {
    switch (value.kind) {
        case 'roundStarted':
            replay.round = value
            replay.phase = null
            return
        case 'phase':
            replay.phase = value
            return
        case 'nodeRemoved':
            replay.nodes.delete(value.nodeId)
            return
        case 'node': {
            // A node sent again after a merge carries no metadata; keep its name.
            const previous = replay.nodes.get(value.node.id)
            replay.nodes.set(
                value.node.id,
                value.metadata || !previous ? value : { ...value, metadata: previous.metadata },
            )
            replay.phase = null
        }
    }
}

function* replayed(replay: ProgressReplay) {
    if (replay.round) yield replay.round
    yield* replay.nodes.values()
    if (replay.phase) yield replay.phase
}

const selectionKey = (input: ContentAnalysisInput) =>
    JSON.stringify([input.instanceId, input.selections, input.installDependencies])

/**
 * The cart owns background work; a review borrows its current request.
 * Canceling stops that work and closes its session. The same selection is not
 * analyzed again until the selection changes or a review asks for it.
 */
export function createContentAnalysis() {
    type Analysis = {
        key: string
        input: ContentAnalysisInput
        controller: AbortController
        /** Only cart changes and user cancellation invalidate an open review. */
        invalidation: AbortController
        session: Promise<string>
        pending: Promise<ResourceOperationPlan>
        settled: boolean
        failed: boolean
        revision: number
        progress: ProgressReplay
        listeners: Set<(progress: ResourceOperationProgress) => void>
    }
    let current: Analysis | null = null
    let session: { context: string; id: Promise<string> } | null = null
    let revision = 0
    let latest: ContentAnalysisInput | null = null
    let canceledSelection: string | null = null

    function closeSession() {
        const previous = session
        session = null
        if (previous) void previous.id.then(resourceAnalysisClose).catch(() => {})
    }

    function clear() {
        // Detach before aborting: abort listeners may call back into cancel(),
        // which must not mistake this replacement for a user cancel.
        const replaced = current
        current = null
        replaced?.invalidation.abort()
        replaced?.controller.abort()
        closeSession()
    }

    function fail(analysis: Analysis, failure: unknown) {
        if (analysis.failed) return
        analysis.failed = true
        analysis.controller.abort(failure)
        if (session?.id === analysis.session) closeSession()
    }

    function start(input: ContentAnalysisInput, invalidation = new AbortController()): Analysis {
        const context = JSON.stringify([input.instanceId, input.context])
        if (session?.context !== context) {
            closeSession()
            session = { context, id: resourceAnalysisOpen(input.instanceId) }
        }
        const id = session!.id
        const controller = new AbortController()
        const progress: ProgressReplay = { round: null, nodes: new Map(), phase: null }
        const listeners = new Set<(progress: ResourceOperationProgress) => void>()
        const selectionRevision = ++revision
        const pending = resourceAnalysisRequest(controller, async (signal) => {
            const sessionId = await id
            signal.throwIfAborted()
            return resourceAnalysisUpdate(
                sessionId,
                selectionRevision,
                input.selections,
                input.installDependencies,
                signal,
                (value) => {
                    if (signal.aborted || value.selectionRevision !== selectionRevision) return
                    remember(progress, value)
                    for (const listener of listeners) listener(value)
                },
            )
        })
        const analysis: Analysis = {
            key: JSON.stringify(input),
            input,
            controller,
            invalidation,
            session: id,
            pending,
            settled: false,
            failed: false,
            revision: selectionRevision,
            progress,
            listeners,
        }
        // A failure retires the native session, but keeps the review and selection
        // available for an explicit retry with a fresh session.
        void pending
            .catch((failure) => {
                if (!invalidation.signal.aborted) fail(analysis, failure)
            })
            .finally(() => {
                analysis.settled = true
            })
        current = analysis
        return analysis
    }

    function update(input: ContentAnalysisInput | null) {
        latest = input
        if (!input?.selections.length) {
            canceledSelection = null
            clear()
            return
        }
        if (canceledSelection === selectionKey(input)) return
        canceledSelection = null
        if (current?.key === JSON.stringify(input)) return
        const replaced = current
        current = null
        replaced?.invalidation.abort()
        replaced?.controller.abort()
        start(input)
    }

    /** Stops the running analysis and keeps the selection out of automatic restarts. */
    function cancel() {
        if (!current) return
        canceledSelection = selectionKey(current.input)
        clear()
    }

    function review(): ContentAnalysisReview | undefined {
        // Opening a review is an explicit retry of a canceled selection.
        if (canceledSelection !== null && latest) {
            canceledSelection = null
            update(latest)
        }
        if (!current) return
        let analysis: Analysis = current
        const invalidation = analysis.invalidation.signal
        return {
            signal: invalidation,
            cancel() {
                if (current === analysis && !analysis.settled) cancel()
            },
            async prepare(signal, onProgress, selections) {
                signal.throwIfAborted()
                invalidation.throwIfAborted()
                if (current !== analysis) throw new DOMException('Cart changed', 'AbortError')
                if (analysis.failed) analysis = start(analysis.input, analysis.invalidation)
                const attempt = analysis
                const controller = new AbortController()
                const abort = () => controller.abort(signal.reason)
                const invalidate = () => controller.abort(invalidation.reason)
                signal.addEventListener('abort', abort, { once: true })
                invalidation.addEventListener('abort', invalidate, { once: true })
                let acceptingProgress = true
                const report = (value: ResourceOperationProgress) => {
                    if (acceptingProgress && !controller.signal.aborted && current === attempt)
                        onProgress(value)
                }
                for (const value of replayed(attempt.progress)) report(value)
                attempt.listeners.add(report)
                const check = () => {
                    controller.signal.throwIfAborted()
                    if (current !== attempt) throw new DOMException('Cart changed', 'AbortError')
                }
                try {
                    return await resourceAnalysisRequest(controller, async (signal) => {
                        // Reuse healthy in-flight work. A review's own cancel only
                        // stops waiting; the cart retains its independent request.
                        await attempt.pending
                        check()
                        const sessionId = await attempt.session
                        check()
                        const prepare = (analysisSession?: {
                            sessionId: string
                            selectionRevision: number
                        }) =>
                            resourcePrepareOperation(
                                attempt.input.instanceId,
                                'install',
                                selections ?? attempt.input.selections,
                                attempt.input.installDependencies,
                                signal,
                                undefined,
                                undefined,
                                report,
                                true,
                                analysisSession,
                            )
                        try {
                            return await prepare({
                                sessionId,
                                selectionRevision: attempt.revision,
                            })
                        } catch (error) {
                            check()
                            if (String(error).includes('analysis session expired'))
                                return await prepare()
                            throw error
                        }
                    })
                } catch (failure) {
                    controller.abort(failure)
                    if (
                        !invalidation.aborted &&
                        (!signal.aborted || signal.reason?.name === 'TimeoutError')
                    )
                        fail(attempt, failure)
                    throw failure
                } finally {
                    acceptingProgress = false
                    signal.removeEventListener('abort', abort)
                    invalidation.removeEventListener('abort', invalidate)
                    attempt.listeners.delete(report)
                }
            },
        }
    }

    return { update, review, cancel, clear }
}
