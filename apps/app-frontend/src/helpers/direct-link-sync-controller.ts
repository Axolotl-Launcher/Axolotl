import type { DirectLinkSyncReport, ExternalMinecraftRoot } from './instance'

export function createDirectLinkSyncController(
    reconcile: (roots: ExternalMinecraftRoot[]) => Promise<DirectLinkSyncReport>,
    onSynced: (report: DirectLinkSyncReport) => void,
) {
    let requestedRoots: ExternalMinecraftRoot[] = []
    let worker: Promise<DirectLinkSyncReport> | undefined
    let pending = false

    async function drain() {
        let latestReport!: DirectLinkSyncReport
        try {
            while (pending) {
                pending = false
                latestReport = await reconcile(requestedRoots)
                onSynced(latestReport)
            }
            return latestReport
        } finally {
            worker = undefined
        }
    }

    return (roots: readonly ExternalMinecraftRoot[], background = false) => {
        if (background && worker && JSON.stringify(roots) === JSON.stringify(requestedRoots)) {
            return worker
        }
        requestedRoots = roots.map((root) => ({ ...root }))
        pending = true
        worker ??= Promise.resolve().then(drain)
        return worker
    }
}

export function directLinkSyncChanged(report: DirectLinkSyncReport): boolean {
    return report.imported > 0 || report.updated > 0 || report.removed > 0
}

/** Starts once, then scans after focus settles; blur and teardown cancel deferred scans. */
export function startDirectLinkSyncOnFocus(
    sync: () => Promise<unknown>,
    onError: (error: unknown) => void,
    target: EventTarget,
): () => void {
    let disposed = false
    let focusTimer: ReturnType<typeof setTimeout> | undefined

    const cancelFocusScan = () => {
        clearTimeout(focusTimer)
        focusTimer = undefined
    }
    const scan = async () => {
        try {
            await sync()
        } catch (error) {
            if (!disposed) onError(error)
        }
    }
    const onFocus = () => {
        cancelFocusScan()
        focusTimer = setTimeout(() => {
            focusTimer = undefined
            if (!disposed) void scan()
        }, 300)
    }

    target.addEventListener('focus', onFocus)
    target.addEventListener('blur', cancelFocusScan)
    void scan()

    return () => {
        disposed = true
        cancelFocusScan()
        target.removeEventListener('focus', onFocus)
        target.removeEventListener('blur', cancelFocusScan)
    }
}
