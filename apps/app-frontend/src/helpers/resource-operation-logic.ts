import { defineMessages, type useVIntl } from '@modrinth/ui'

export function resourceSourceName(source: string): string {
    switch (source) {
        case 'modrinth':
            return 'Modrinth'
        case 'curseforge':
            return 'CurseForge'
        case 'mcarchive':
            return 'MCArchive'
        case 'planet_minecraft':
            return 'Planet Minecraft'
        default:
            return source
    }
}

const errorMessages = defineMessages({
    unavailable: {
        id: 'app.resources.error.unavailable',
        defaultMessage: 'The content source is unavailable. Check your connection and try again.',
    },
    network: {
        id: 'app.resources.error.network',
        defaultMessage:
            'Could not connect to the content source. Check your connection and try again.',
    },
    http: {
        id: 'app.resources.error.http',
        defaultMessage:
            'The content source rejected the request (HTTP {status, number, ::group-off}). Try again later.',
    },
    canceled: {
        id: 'app.resources.error.canceled',
        defaultMessage: 'The content operation was cancelled.',
    },
    cursorExpired: {
        id: 'app.resources.error.cursor-expired',
        defaultMessage: 'The search session expired. Restart the search.',
    },
    stale: {
        id: 'app.resources.error.stale',
        defaultMessage:
            'The instance or selected files changed. Refresh the preview before continuing.',
    },
    backup: {
        id: 'app.resources.error.backup-unavailable',
        defaultMessage:
            'This rollback backup is unavailable or no longer matches the recorded file.',
    },
    sourceResponse: {
        id: 'app.resources.error.source-response',
        defaultMessage: 'The content source returned an invalid response. Try again.',
    },
    cache: {
        id: 'app.resources.error.cache',
        defaultMessage: 'The resource cache is unavailable. Restart the launcher and try again.',
    },
    database: {
        id: 'app.resources.error.database',
        defaultMessage: 'The resource database is unavailable. Restart the launcher and try again.',
    },
    invalidSearch: {
        id: 'app.resources.error.invalid-search',
        defaultMessage:
            'These search filters could not be applied. Reset the filters and try again.',
    },
    incompleteVersions: {
        id: 'app.resources.error.incomplete-versions',
        defaultMessage:
            'The source could not return all versions. Open the project’s Versions tab and choose a file.',
    },
    unsupported: {
        id: 'app.resources.error.unsupported',
        defaultMessage: 'This content source does not support the requested operation or filters.',
    },
    failed: {
        id: 'app.resources.error.failed',
        defaultMessage: 'Could not complete the content operation. Refresh and try again.',
    },
})

type FormatMessage = ReturnType<typeof useVIntl>['formatMessage']

export function isResourceOperationError(error: unknown): boolean {
    return (
        (typeof error === 'object' &&
            error !== null &&
            'code' in error &&
            ['resourceError', 'cursorExpired'].includes(String(error.code))) ||
        /^(resource (cursor|source|request|cache|database)|unsupported resource operation|invalid (resource request|source response)|Source presentation is unavailable|Source release window is incomplete|Source returned an invalid release cursor|.*search filter|Invalid search page)/i.test(
            rawMessage(error),
        )
    )
}

export function resourceOperationError(error: unknown, formatMessage?: FormatMessage): string {
    const message = rawMessage(error)
    if (!formatMessage) return message
    if (
        (typeof error === 'object' &&
            error !== null &&
            'code' in error &&
            error.code === 'cursorExpired') ||
        /resource cursor expired|release cursor repeated/i.test(message)
    )
        return formatMessage(errorMessages.cursorExpired)
    if (/Source release window is incomplete/i.test(message))
        return formatMessage(errorMessages.incompleteVersions)
    if (/Source returned an invalid release cursor/i.test(message))
        return formatMessage(errorMessages.sourceResponse)
    if (/search filter|Invalid search page/i.test(message))
        return formatMessage(errorMessages.invalidSearch)
    if (/unsupported resource operation/i.test(message))
        return formatMessage(errorMessages.unsupported)
    if (
        /instance changed|stale .*revision|file changed|changed after export|export session expired|cleanup plan expired/i.test(
            message,
        )
    )
        return formatMessage(errorMessages.stale)
    if (/verified rollback backup not available|rollback backup unavailable/i.test(message))
        return formatMessage(errorMessages.backup)
    if (/resource request canceled/i.test(message)) return formatMessage(errorMessages.canceled)
    if (/resource source is unavailable|Source presentation is unavailable/i.test(message))
        return formatMessage(errorMessages.unavailable)
    const status = message.match(/HTTP status Some\((\d+)\)/)?.[1]
    if (status) return formatMessage(errorMessages.http, { status: Number(status) })
    if (/resource request failed/i.test(message)) return formatMessage(errorMessages.network)
    if (/invalid source response/i.test(message)) return formatMessage(errorMessages.sourceResponse)
    if (/resource cache unavailable/i.test(message)) return formatMessage(errorMessages.cache)
    if (/resource database unavailable/i.test(message)) return formatMessage(errorMessages.database)
    return formatMessage(errorMessages.failed)
}

function rawMessage(error: unknown): string {
    if (typeof error === 'object' && error !== null && 'message' in error) {
        return String(error.message)
    }
    return String(error)
}
