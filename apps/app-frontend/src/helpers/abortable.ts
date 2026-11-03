/**
 * Waits for a native request but rejects as soon as `signal` aborts. `onAbort`
 * runs once at that moment with the still-pending request, so the caller can
 * ask the native side to stop and release whatever a late result holds.
 */
export async function abortable<T>(
    request: Promise<T>,
    signal: AbortSignal | undefined,
    onAbort: (late: Promise<T>) => void,
): Promise<T> {
    if (!signal) return await request
    if (signal.aborted) {
        onAbort(request)
        throw signal.reason
    }
    let stop = () => {}
    const aborted = new Promise<never>((_, reject) => {
        const abort = () => {
            onAbort(request)
            reject(signal.reason)
        }
        signal.addEventListener('abort', abort, { once: true })
        stop = () => signal.removeEventListener('abort', abort)
    })
    try {
        return await Promise.race([request, aborted])
    } finally {
        stop()
    }
}
