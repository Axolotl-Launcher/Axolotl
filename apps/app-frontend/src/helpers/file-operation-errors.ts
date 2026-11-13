import { defineMessage, type useVIntl } from '@modrinth/ui'

import { errorMessage } from './error-text'

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
    // A bare string is matched directly; anything else has to carry a message,
    // and this returns null rather than a fallback because the caller only asks
    // whether the failure is a locked file.
    const message = typeof error === 'string' ? error : errorMessage(error)
    if (message === null) return null

    const hasLockDetails = /\bFile locked by:\s*\n\s*PID \d+ - /i.test(message)
    const hasWindowsLockCode =
        os === 'windows' &&
        /\bI\/O(?: \(std\))? error:|, path:/i.test(message) &&
        /\(os error (32|33)\)/.test(message)

    return hasLockDetails || hasWindowsLockCode ? formatMessage(fileInUseMessage) : null
}
