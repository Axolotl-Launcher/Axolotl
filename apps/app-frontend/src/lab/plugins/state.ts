import { ref } from 'vue'

/**
 * Persisted per-plugin state. Both lists are id lists rather than a map, so a
 * plugin that disappears from the build leaves nothing behind.
 */
export type LabPluginState = {
    /** Plugins the user turned off. Everything not listed here is enabled. */
    disabled: string[]
    /** Plugins the launcher turned off after they threw, with the message. */
    failures: Record<string, string>
}

const LAB_PLUGIN_STATE_STORAGE_KEY = 'axolotl-lab-plugin-state'

const emptyState: LabPluginState = { disabled: [], failures: {} }

function toStringArray(value: unknown): string[] {
    return Array.isArray(value)
        ? value.filter((item): item is string => typeof item === 'string')
        : []
}

function toStringRecord(value: unknown): Record<string, string> {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return {}
    const result: Record<string, string> = {}
    for (const [key, message] of Object.entries(value)) {
        if (typeof message === 'string') result[key] = message
    }
    return result
}

/**
 * Turns whatever localStorage held into a usable state. Unreadable input falls
 * back to "everything enabled", which is the only safe direction: a corrupted
 * entry should not hide every plugin.
 */
export function parseLabPluginState(raw: string | null): LabPluginState {
    if (!raw) return emptyState
    try {
        const parsed: unknown = JSON.parse(raw)
        if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed))
            return emptyState
        const record = parsed as Record<string, unknown>
        return {
            disabled: toStringArray(record.disabled),
            failures: toStringRecord(record.failures),
        }
    } catch {
        return emptyState
    }
}

/** Adds or removes an id from the disabled list without mutating the input. */
export function setDisabled(state: LabPluginState, id: string, disabled: boolean): LabPluginState {
    const next = new Set(state.disabled)
    if (disabled) next.add(id)
    else next.delete(id)
    return { ...state, disabled: [...next] }
}

/** Records a failure and disables the plugin in one step. */
export function setFailure(state: LabPluginState, id: string, message: string): LabPluginState {
    const disabled = setDisabled(state, id, true)
    return { ...disabled, failures: { ...disabled.failures, [id]: message } }
}

/** Removes a recorded failure so the plugin can run again. */
export function clearFailure(state: LabPluginState, id: string): LabPluginState {
    if (!(id in state.failures)) return state
    const failures = Object.fromEntries(
        Object.entries(state.failures).filter(([pluginId]) => pluginId !== id),
    )
    return { ...state, failures }
}

function readStoredState(): LabPluginState {
    try {
        return parseLabPluginState(
            globalThis.localStorage?.getItem(LAB_PLUGIN_STATE_STORAGE_KEY) ?? null,
        )
    } catch {
        return emptyState
    }
}

const labPluginState = ref<LabPluginState>(readStoredState())

function commit(next: LabPluginState) {
    labPluginState.value = next
    try {
        globalThis.localStorage?.setItem(LAB_PLUGIN_STATE_STORAGE_KEY, JSON.stringify(next))
    } catch {
        // Storage can be unavailable (private mode, quota); the session keeps working.
    }
}

/** Reactive snapshot of all plugin state. Changing it does not persist; use the setters. */
export function getLabPluginState() {
    return labPluginState
}

/** Whether a plugin should render. Plugins are enabled until turned off. */
export function isLabPluginEnabled(id: string): boolean {
    return !labPluginState.value.disabled.includes(id)
}

/** Turns one plugin on or off, keeping any recorded failure until it runs again. */
export function setLabPluginEnabled(id: string, enabled: boolean) {
    commit(setDisabled(labPluginState.value, id, !enabled))
}

/** Last error message recorded for a plugin, if the launcher disabled it. */
export function getLabPluginFailure(id: string): string | undefined {
    return labPluginState.value.failures[id]
}

/** Disables a plugin after it threw and stores why, so the list page can explain it. */
export function recordLabPluginFailure(id: string, message: string) {
    commit(setFailure(labPluginState.value, id, message))
}

/** Clears a recorded failure and turns the plugin back on. */
export function retryLabPlugin(id: string) {
    commit(setDisabled(clearFailure(labPluginState.value, id), id, false))
}
