/** The text of a launch error as the backend reports it, whatever shape it arrives in. */
export function launchErrorText(error: unknown): string {
    if (typeof error === 'string') return error
    if (error && typeof error === 'object') {
        const record = error as Record<string, unknown>
        const values = [record.message, record.error, record.cause]
            .filter((value): value is string => typeof value === 'string')
            .join('\n')
        if (values) return values
        try {
            return JSON.stringify(error)
        } catch {
            return ''
        }
    }
    return String(error)
}

export type LaunchFailureKind =
    'exitedBeforeInitialization' | 'initializationTimedOut' | 'preparationTimedOut'

/**
 * Which launch failure an error reports, if any. Kept apart from the crash modal so
 * the launch error handler can decide synchronously before the modal has loaded.
 */
export function launchFailureKind(error: unknown): LaunchFailureKind | null {
    const errorText = launchErrorText(error)
    if (errorText.includes('Minecraft exited before launcher initialization completed')) {
        return 'exitedBeforeInitialization'
    }
    if (errorText.includes('Minecraft launcher initialization did not respond')) {
        return 'initializationTimedOut'
    }
    if (errorText.includes('Minecraft launch preparation timed out')) {
        return 'preparationTimedOut'
    }
    return null
}
