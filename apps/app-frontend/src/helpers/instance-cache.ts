export interface InstanceContentCache {
    modpackHintDismissed: boolean
}

function removeLegacyContent(instanceId: string) {
    try {
        localStorage.removeItem(`instance:${instanceId}:data`)
        for (const key of ['instance-content-cache', 'instance-linked-content-cache'])
            localStorage.removeItem(key)
    } catch (error) {
        console.warn('Could not clear legacy instance content cache', error)
    }
}

/** Content lives in the launcher database; only independent UI preferences stay here. */
export function readInstanceCache(instanceId: string): InstanceContentCache | null {
    removeLegacyContent(instanceId)
    try {
        const raw = localStorage.getItem(`instance:${instanceId}:ui`)
        return raw ? { modpackHintDismissed: JSON.parse(raw).modpackHintDismissed === true } : null
    } catch {
        return null
    }
}

export function writeInstanceCache(instanceId: string, patch: Partial<InstanceContentCache>): void {
    removeLegacyContent(instanceId)
    try {
        const current = readInstanceCache(instanceId) ?? { modpackHintDismissed: false }
        localStorage.setItem(`instance:${instanceId}:ui`, JSON.stringify({ ...current, ...patch }))
    } catch (error) {
        console.warn('Could not save instance UI preferences', error)
    }
}

export function removeInstanceCache(instanceId: string): void {
    removeLegacyContent(instanceId)
    try {
        localStorage.removeItem(`instance:${instanceId}:ui`)
    } catch {
        /* Unavailable storage does not block deletion. */
    }
}
