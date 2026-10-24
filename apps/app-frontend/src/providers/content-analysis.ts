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
    prepare: (
        signal: AbortSignal,
        onProgress: (progress: ResourceOperationProgress) => void,
        selections?: ResourceSelection[],
    ) => Promise<ResourceOperationPlan>
}

/** The cart owns background work; a review borrows its current request. */
export function createContentAnalysis() {
    let current: {
        key: string
        input: ContentAnalysisInput
        controller: AbortController
        session: Promise<string>
        pending: Promise<ResourceOperationPlan>
        revision: number
        progress: Map<string, ResourceOperationProgress>
        listeners: Set<(progress: ResourceOperationProgress) => void>
    } | null = null
    let session: { context: string; id: Promise<string> } | null = null
    let revision = 0

    function clear() {
        current?.controller.abort()
        current = null
        const previous = session
        session = null
        if (previous) void previous.id.then(resourceAnalysisClose).catch(() => {})
    }

    function update(input: ContentAnalysisInput | null) {
        if (!input?.selections.length) {
            clear()
            return
        }
        const key = JSON.stringify(input)
        if (current?.key === key) return
        current?.controller.abort()
        const context = JSON.stringify([input.instanceId, input.context])
        if (session?.context !== context) {
            clear()
            session = { context, id: resourceAnalysisOpen(input.instanceId) }
        }
        const id = session!.id
        const controller = new AbortController()
        const progress = new Map<string, ResourceOperationProgress>()
        const listeners = new Set<(progress: ResourceOperationProgress) => void>()
        const selectionRevision = ++revision
        const pending = id.then((sessionId) => {
            controller.signal.throwIfAborted()
            return resourceAnalysisUpdate(
                sessionId,
                selectionRevision,
                input.selections,
                input.installDependencies,
                controller.signal,
                (value) => {
                    if (controller.signal.aborted) return
                    progress.set(value.node.id, value)
                    for (const listener of listeners) listener(value)
                },
            )
        })
        // Errors stay with this analysis and are retried by the review action.
        void pending.catch(() => {})
        current = {
            key,
            input,
            controller,
            session: id,
            pending,
            revision: selectionRevision,
            progress,
            listeners,
        }
    }

    function review(): ContentAnalysisReview | undefined {
        const analysis = current
        if (!analysis) return
        return {
            signal: analysis.controller.signal,
            async prepare(signal, onProgress, selections) {
                signal.throwIfAborted()
                analysis.controller.signal.throwIfAborted()
                for (const value of analysis.progress.values()) onProgress(value)
                analysis.listeners.add(onProgress)
                let stopWaiting = () => {}
                const canceled = new Promise<never>((_, reject) => {
                    const abort = () => reject(new DOMException('Analysis canceled', 'AbortError'))
                    signal.addEventListener('abort', abort, { once: true })
                    analysis.controller.signal.addEventListener('abort', abort, { once: true })
                    stopWaiting = () => {
                        signal.removeEventListener('abort', abort)
                        analysis.controller.signal.removeEventListener('abort', abort)
                    }
                })
                const check = () => {
                    signal.throwIfAborted()
                    analysis.controller.signal.throwIfAborted()
                    if (current !== analysis) throw new DOMException('Cart changed', 'AbortError')
                }
                try {
                    // Reuse in-flight work. Closing the review does not cancel
                    // the cart's independent subscription.
                    await Promise.race([analysis.pending.catch(() => {}), canceled])
                    check()
                    const sessionId = await analysis.session.catch(() => undefined)
                    check()
                    const prepare = (analysisSession?: {
                        sessionId: string
                        selectionRevision: number
                    }) =>
                        resourcePrepareOperation(
                            analysis.input.instanceId,
                            'install',
                            selections ?? analysis.input.selections,
                            analysis.input.installDependencies,
                            signal,
                            undefined,
                            undefined,
                            onProgress,
                            true,
                            analysisSession,
                        )
                    try {
                        return await prepare(
                            sessionId
                                ? { sessionId, selectionRevision: analysis.revision }
                                : undefined,
                        )
                    } catch (error) {
                        check()
                        if (String(error).includes('analysis session expired'))
                            return await prepare()
                        throw error
                    }
                } finally {
                    stopWaiting()
                    analysis.listeners.delete(onProgress)
                }
            },
        }
    }

    return { update, review, clear }
}
