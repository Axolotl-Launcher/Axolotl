import { reactive } from 'vue'

const startedAt = reactive(new Map<string, number>())
const launches = new Map<string, Promise<unknown>>()

export function instanceLaunchStartedAt(instanceId: string): number | undefined {
    return startedAt.get(instanceId)
}

export function trackInstanceLaunch<T>(instanceId: string, launch: () => Promise<T>): Promise<T> {
    const existing = launches.get(instanceId)
    if (existing) return existing as Promise<T>

    startedAt.set(instanceId, Date.now())
    const pending = Promise.resolve()
        .then(launch)
        .finally(() => {
            launches.delete(instanceId)
            startedAt.delete(instanceId)
        })
    launches.set(instanceId, pending)
    return pending
}
