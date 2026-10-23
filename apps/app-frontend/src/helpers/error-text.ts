/**
 * Reads a human-readable message out of whatever a rejected `invoke` threw.
 *
 * Tauri serializes command errors, so a rejection is usually a plain object such
 * as `{ field_name: 'Theseus', message: '…' }` rather than an `Error`. Passing
 * that to `String()` yields `[object Object]`, which is what the user would end
 * up reading in the dialog, so every caller needs this extraction instead.
 */
export function errorText(error: unknown, fallback = ''): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error.trim() || fallback
    if (error && typeof error === 'object') {
        const message = (error as { message?: unknown }).message
        if (typeof message === 'string') return message.trim() || fallback
        try {
            return JSON.stringify(error)
        } catch {
            // Circular or otherwise unserializable: nothing readable to show.
            return fallback
        }
    }
    return error === undefined || error === null || error === '' ? fallback : String(error)
}
