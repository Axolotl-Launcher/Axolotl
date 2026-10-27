/**
 * Shares one in-flight content refresh per instance.
 *
 * The content page can ask for a refresh from several places at once: a
 * content-change event, an install revision bump, a user action. Every refresh
 * re-scans and re-hashes the whole instance directory in the backend, so
 * overlapping calls repeat the same expensive work and keep the tree busy for
 * longer than it needs to be. Concurrent callers share the pending request
 * instead; a settled or failed request is dropped so the next call starts a
 * fresh one.
 */
export function createContentRefreshCoalescer<Snapshot>(
    run: (instanceId: string) => Promise<Snapshot>,
) {
    const inFlight = new Map<string, Promise<Snapshot>>()

    return (instanceId: string): Promise<Snapshot> => {
        const pending = inFlight.get(instanceId)
        if (pending) return pending

        const request = run(instanceId).finally(() => {
            inFlight.delete(instanceId)
        })
        inFlight.set(instanceId, request)
        return request
    }
}
