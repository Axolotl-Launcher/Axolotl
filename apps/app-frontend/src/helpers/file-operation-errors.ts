import { defineMessage, type useVIntl } from '@modrinth/ui'

const fileInUseMessage = defineMessage({
    id: 'app.files.error.in-use',
    defaultMessage:
        'This file is in use. Close the game or any other program using it, then try again.',
})

export function fileOperationError(
    error: unknown,
    os: string,
    formatMessage: ReturnType<typeof useVIntl>['formatMessage'],
): string | null {
    const message =
        typeof error === 'string'
            ? error
            : error && typeof error === 'object' && 'message' in error
              ? error.message
              : null
    if (typeof message !== 'string') return null

    const hasLockDetails = /\bFile locked by:\s*\n\s*PID \d+ - /i.test(message)
    const hasWindowsLockCode =
        os === 'windows' &&
        /\bI\/O(?: \(std\))? error:|, path:/i.test(message) &&
        /\(os error (32|33)\)/.test(message)

    return hasLockDetails || hasWindowsLockCode ? formatMessage(fileInUseMessage) : null
}
