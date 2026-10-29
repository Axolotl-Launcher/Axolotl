import { type Ref, ref, type ShallowRef, shallowRef } from 'vue'

/**
 * Where an analysis stands. A run moves through `queued → processing` to
 * `ready` or `error`; canceling moves it through `canceling → canceled`.
 */
export type AnalysisTaskStatus =
    'idle' | 'queued' | 'processing' | 'ready' | 'error' | 'canceling' | 'canceled'

export interface AnalysisRunContext<P, R> {
    signal: AbortSignal
    /** Reports progress; ignored once this run is no longer the current one. */
    progress: (value: P) => void
    /** Publishes results found so far; ignored once this run is no longer current. */
    partial: (value: R) => void
    /**
     * Registers the native side's acknowledgement of a stop request made when
     * this run's signal aborted. `cancel()` reports canceled only once every
     * registered acknowledgement has resolved, and an error if one rejects.
     */
    stopping: (confirmation: Promise<unknown>) => void
}

export interface AnalysisTaskOptions<I, P, R> {
    /** Names the task in timing logs. */
    label: string
    run: (input: I, context: AnalysisRunContext<P, R>) => Promise<R>
    /**
     * Asks the native side to stop and resolves once it confirmed. Without it,
     * aborting the run's signal is the whole cancellation.
     */
    stop?: (input: I) => Promise<void> | void
    /** Coalesces progress and partial results; defaults to one commit per animation frame. */
    schedule?: (commit: () => void) => void
    /**
     * Receives a result that arrived after its run was abandoned or canceled,
     * so whatever it holds, such as a native session, can be released.
     */
    discard?: (result: R, input: I) => void
}

export interface AnalysisTask<I, P, R> {
    readonly status: Readonly<Ref<AnalysisTaskStatus>>
    readonly progress: Readonly<ShallowRef<P | null>>
    /** Results found so far. Kept after a failure or cancel, so finished work stays usable. */
    readonly partial: Readonly<ShallowRef<R | null>>
    readonly result: Readonly<ShallowRef<R | null>>
    readonly error: Readonly<ShallowRef<unknown>>
    /** The input of the latest run, kept for `retry`. */
    readonly input: Readonly<ShallowRef<I | null>>
    /** True while a run is queued, processing or canceling. */
    readonly running: Readonly<Ref<boolean>>
    /** Starts a new run, abandoning the current one. Resolves with its result, or undefined if it did not finish. */
    start(input: I): Promise<R | undefined>
    /** Stops the current run. Its results are no longer accepted from this moment on. */
    cancel(): Promise<void>
    retry(): Promise<R | undefined>
    /** Abandons the current run and forgets its input and results, as if nothing had run. */
    reset(): void
    /** Cancels the current run and refuses later starts. */
    dispose(): void
}

const defaultSchedule = (commit: () => void) =>
    typeof requestAnimationFrame === 'function'
        ? requestAnimationFrame(() => commit())
        : setTimeout(commit, 16)

export function createAnalysisTask<I, P, R>(
    options: AnalysisTaskOptions<I, P, R>,
): AnalysisTask<I, P, R> {
    // Same console channel and prefix as the shared debug logger, without
    // tying every caller's tests to the UI package.
    const log = (message: string, data: Record<string, unknown>) =>
        console.debug(`[analysis:${options.label}]`, message, data)
    const schedule = options.schedule ?? defaultSchedule
    const status = ref<AnalysisTaskStatus>('idle')
    const running = ref(false)
    const progress = shallowRef<P | null>(null)
    const partial = shallowRef<R | null>(null)
    const result = shallowRef<R | null>(null)
    const error = shallowRef<unknown>(null)
    const input = shallowRef<I | null>(null)
    let current: {
        id: number
        controller: AbortController
        startedAt: number
        flush: () => void
        stops: Promise<unknown>[]
    } | null = null
    let runs = 0
    let disposed = false

    function settle(next: AnalysisTaskStatus) {
        status.value = next
        running.value = next === 'queued' || next === 'processing' || next === 'canceling'
    }

    async function start(value: I): Promise<R | undefined> {
        if (disposed) return undefined
        if (current) abandon(current)
        const run: NonNullable<typeof current> = {
            id: ++runs,
            controller: new AbortController(),
            startedAt: performance.now(),
            flush: () => {},
            stops: [],
        }
        current = run
        input.value = value
        progress.value = null
        partial.value = null
        result.value = null
        error.value = null
        settle('queued')
        let firstResultAt: number | null = null
        let pending: { progress?: P; partial?: R } | null = null
        const isCurrent = () => current === run && !run.controller.signal.aborted
        const commitPending = () => {
            const batch = pending
            pending = null
            if (!batch || !isCurrent()) return
            if ('progress' in batch) progress.value = batch.progress!
            if ('partial' in batch) partial.value = batch.partial!
        }
        run.flush = commitPending
        const queue = (update: { progress?: P; partial?: R }) => {
            if (!isCurrent()) return
            firstResultAt ??= performance.now()
            const scheduled = pending !== null
            pending = { ...pending, ...update }
            if (!scheduled) schedule(commitPending)
        }
        await Promise.resolve()
        if (!isCurrent()) return undefined
        settle('processing')
        try {
            const outcome = await options.run(value, {
                signal: run.controller.signal,
                progress: (next) => queue({ progress: next }),
                partial: (next) => queue({ partial: next }),
                stopping: (confirmation) => {
                    const acknowledged = Promise.resolve(confirmation)
                    // Abandoned runs never await it, so a rejection must not go unhandled.
                    acknowledged.catch(() => {})
                    run.stops.push(acknowledged)
                },
            })
            if (!isCurrent()) {
                options.discard?.(outcome, value)
                return undefined
            }
            pending = null
            result.value = outcome
            partial.value = outcome
            settle('ready')
            return outcome
        } catch (failure) {
            if (!isCurrent()) return undefined
            commitPending()
            error.value = failure
            settle('error')
            return undefined
        } finally {
            if (current === run) {
                log('run finished', {
                    status: status.value,
                    firstResultMs: firstResultAt && Math.round(firstResultAt - run.startedAt),
                    totalMs: Math.round(performance.now() - run.startedAt),
                })
                if (status.value !== 'canceling') current = null
            }
        }
    }

    /** Stops accepting a run's results and aborts it without waiting for confirmation. */
    function abandon(run: NonNullable<typeof current>) {
        run.controller.abort()
        const stopped = input.value
        if (options.stop && stopped !== null)
            void Promise.resolve(options.stop(stopped)).catch(() => {})
    }

    async function cancel() {
        const run = current
        if (!run || !running.value) return
        const stopped = input.value
        const requestedAt = performance.now()
        // Results published just before the cancel stay visible.
        run.flush()
        run.controller.abort()
        settle('canceling')
        try {
            await Promise.all([
                options.stop && stopped !== null ? options.stop(stopped) : undefined,
                ...run.stops,
            ])
            if (current !== run) return
            settle('canceled')
        } catch (failure) {
            if (current !== run) return
            // The work may still be running, so it must not look stopped.
            error.value = failure
            settle('error')
        } finally {
            if (current === run) {
                log('cancel confirmed', {
                    status: status.value,
                    confirmMs: Math.round(performance.now() - requestedAt),
                })
                current = null
            }
        }
    }

    function retry() {
        return input.value === null ? Promise.resolve(undefined) : start(input.value)
    }

    function reset() {
        if (current) {
            abandon(current)
            current = null
        }
        input.value = null
        progress.value = null
        partial.value = null
        result.value = null
        error.value = null
        settle('idle')
    }

    function dispose() {
        disposed = true
        if (current) {
            abandon(current)
            current = null
        }
        if (running.value) settle('canceled')
    }

    return {
        status,
        progress,
        partial,
        result,
        error,
        input,
        running,
        start,
        cancel,
        retry,
        reset,
        dispose,
    }
}
