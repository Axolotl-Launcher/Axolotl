import { abortable } from '@/helpers/abortable'

/** Native analysis has fifteen minutes; allow thirty seconds for IPC delivery. */
export const RESOURCE_ANALYSIS_TIMEOUT_MS = (15 * 60 + 30) * 1000

/** A total deadline, independent of progress, also bounds native promises that never settle. */
export async function resourceAnalysisRequest<T>(
    controller: AbortController,
    action: (signal: AbortSignal) => Promise<T>,
): Promise<T> {
    const { signal } = controller
    signal.throwIfAborted()
    const timer = setTimeout(
        () => controller.abort(new DOMException('Resource analysis timed out', 'TimeoutError')),
        RESOURCE_ANALYSIS_TIMEOUT_MS,
    )
    try {
        return await abortable(action(signal), signal, () => {})
    } finally {
        clearTimeout(timer)
    }
}
