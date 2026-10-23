/**
 * The app's settings patch saver (apps/app-frontend/src/helpers/settings-patch.ts):
 * a page edits a local copy of the settings and sends only the fields it changed,
 * one save after another, advancing its baseline only once a save succeeded.
 */
type SettingsObject = Record<string, unknown>

function isObject(value: unknown): value is SettingsObject {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
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

/** Captures each draft at submission and advances its baseline only after saving. */
export function createSettingsPatchSaver<T extends object>(
    initial: T,
    save: (patch: Record<string, unknown>) => Promise<unknown>,
) {
    let committed: SettingsObject = JSON.parse(JSON.stringify(initial))
    let queue = Promise.resolve()
    return (draft: T): Promise<void> => {
        const snapshot: SettingsObject = JSON.parse(JSON.stringify(draft))
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
}
