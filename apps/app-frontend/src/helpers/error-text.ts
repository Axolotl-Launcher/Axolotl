/**
 * Reads the `message` a rejected `invoke` carries, or `null` when the value has
 * no string message.
 *
 * Tauri serializes command errors, so a rejection is usually a plain object such
 * as `{ field_name: 'Theseus', message: '…' }` instead of an `Error`. Callers
 * that only pattern match a message want this nullable form; callers that show
 * the error to the user want {@link errorText}.
 *
 * Nothing here may throw: it runs inside `catch` blocks, and an error path that
 * throws again hides the original failure. A value that cannot be inspected (a
 * `Proxy` whose `has` trap or getter throws) is treated as carrying no message.
 */
export function errorMessage(error: unknown): string | null {
    try {
        if (error && typeof error === 'object' && 'message' in error) {
            const message = (error as { message?: unknown }).message
            if (typeof message === 'string') return message
        }
    } catch {
        // Fall through: an uninspectable value has no readable message.
    }
    return null
}

/**
 * The human-readable text for whatever a rejected `invoke` threw.
 *
 * Objects without a string message retain their structured details as JSON.
 * Missing values and objects that cannot be serialized use the caller's fallback.
 *
 * Pass a localized `unknownError` when the text reaches the user: this module has
 * no i18n context, and the English default is only meant for callers that just
 * pattern-match the result.
 */
export function errorText(error: unknown, unknownError = 'Unknown error'): string {
    const message = errorMessage(error)
    if (message !== null) return message
    if (error === null || error === undefined) return unknownError
    try {
        if (typeof error === 'object') {
            const details = JSON.stringify(error)
            return typeof details === 'string' ? details : unknownError
        }
        return String(error)
    } catch {
        // Circular references, throwing getters and custom conversions can fail.
        return unknownError
    }
}
