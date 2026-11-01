import type { LabPluginContext, LabPluginStorage } from './types'

const PLUGIN_STORAGE_PREFIX = 'axolotl-lab-plugin:'

/**
 * Builds the storage a plugin gets. Every key is prefixed with the plugin id,
 * so one plugin cannot read or clobber another one's values even though all of
 * them share localStorage.
 */
export function createLabPluginStorage(pluginId: string): LabPluginStorage {
    const prefix = `${PLUGIN_STORAGE_PREFIX}${pluginId}:`
    return {
        read<T>(key: string, fallback: T): T {
            try {
                const raw = globalThis.localStorage?.getItem(prefix + key)
                if (raw === null || raw === undefined) return fallback
                return JSON.parse(raw) as T
            } catch {
                return fallback
            }
        },
        write(key: string, value: unknown) {
            try {
                globalThis.localStorage?.setItem(prefix + key, JSON.stringify(value))
            } catch {
                // Storage can be full or unavailable; losing a plugin preference
                // must not break the page the plugin is rendered on.
            }
        },
        remove(key: string) {
            try {
                globalThis.localStorage?.removeItem(prefix + key)
            } catch {
                // See write().
            }
        },
    }
}

/**
 * Context handed to a plugin component while it renders. Recreated per
 * boundary, so a plugin always sees its own id and its own storage.
 */
export function createLabPluginContext(pluginId: string): LabPluginContext {
    return {
        pluginId,
        storage: createLabPluginStorage(pluginId),
        log(message: string, detail?: unknown) {
            if (detail === undefined) console.info(`[lab plugin ${pluginId}] ${message}`)
            else console.info(`[lab plugin ${pluginId}] ${message}`, detail)
        },
    }
}
