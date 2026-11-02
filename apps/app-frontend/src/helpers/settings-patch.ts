type SettingsObject = Record<string, unknown>

function isObject(value: unknown): value is SettingsObject {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function cloneSettings(settings: object): SettingsObject {
    return JSON.parse(JSON.stringify(settings))
}

/** Returns only edited fields, including individual keys of nested settings. */
export function diffSettings(before: SettingsObject, after: SettingsObject): SettingsObject {
    const patch: SettingsObject = {}
    for (const [key, value] of Object.entries(after)) {
        if (JSON.stringify(before[key]) === JSON.stringify(value)) continue
        patch[key] =
            ['feature_flags', 'memory', 'hooks'].includes(key) &&
            isObject(before[key]) &&
            isObject(value)
                ? diffSettings(before[key], value)
                : value
    }
    return patch
}

export interface SettingsPatchSaver<T extends object> {
    (draft: T): Promise<void>
    /**
     * Moves the baseline without saving, for stored settings this saver did not
     * write: otherwise the next draft would diff against the older baseline and
     * write back the keys another writer has just cleared.
     */
    reset(settings: T): void
}

/** Captures each draft at submission and advances its baseline only after saving. */
export function createSettingsPatchSaver<T extends object>(
    initial: T,
    save: (patch: Record<string, unknown>) => Promise<unknown>,
): SettingsPatchSaver<T> {
    let committed: SettingsObject = cloneSettings(initial)
    let queue = Promise.resolve()
    const saver = (draft: T): Promise<void> => {
        const snapshot: SettingsObject = cloneSettings(draft)
        const operation = queue
            .catch(() => {})
            .then(async () => {
                const patch = diffSettings(committed, snapshot)
                if (Object.keys(patch).length > 0) await save(patch)
                committed = snapshot
            })
        queue = operation
        return operation
    }
    return Object.assign(saver, {
        reset: (settings: T) => {
            committed = cloneSettings(settings)
        },
    })
}
