/**
 * Reads the human-readable message out of whatever a rejected `invoke` threw.
 *
 * Tauri serializes command errors, so a rejection is usually a plain object such
 * as `{ field_name: 'Theseus', message: '…' }` instead of an `Error`. Passing
 * that to `String()` yields `[object Object]`, which is what the user would end
 * up reading in a failure dialog.
 *
 * This is the extraction that `multiplayerErrorText` and `fileOperationError`
 * already relied on; it lives here so new call sites share one implementation
 * instead of repeating it again.
 */
export function errorText(error: unknown): string {
    if (
        error &&
        typeof error === 'object' &&
        'message' in error &&
        typeof (error as { message: unknown }).message === 'string'
    ) {
        return (error as { message: string }).message
    }
    return String(error)
}
