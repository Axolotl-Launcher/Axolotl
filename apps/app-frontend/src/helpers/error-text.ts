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
 * `String()` on a serialized error object yields `[object Object]`, which is what
 * the user would otherwise read in a failure dialog.
 */
export function errorText(error: unknown): string {
    const message = errorMessage(error)
    if (message !== null) return message
    try {
        return String(error)
    } catch {
        // Only reachable for values whose own conversion throws.
        return 'Unknown error'
    }
}
