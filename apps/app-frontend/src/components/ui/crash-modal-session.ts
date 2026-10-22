/** Coalesce duplicate notifications for one visible game run, regardless of delivery delay. */
export function createCrashModalSession() {
    let visible: { instanceId: string; runId?: string } | null = null
    const runs = new Map<string, string>()

    return {
        open(instanceId: string, runId?: string): boolean {
            const currentRun = runId ?? runs.get(instanceId)
            if (
                visible?.instanceId === instanceId &&
                (!currentRun || !visible.runId || visible.runId === currentRun)
            )
                return false
            visible = { instanceId, runId: currentRun }
            return true
        },
        launched(instanceId: string, runId: string) {
            if (runs.get(instanceId) === runId) return
            runs.set(instanceId, runId)
            if (visible?.instanceId === instanceId) visible = null
        },
        close() {
            visible = null
        },
    }
}
