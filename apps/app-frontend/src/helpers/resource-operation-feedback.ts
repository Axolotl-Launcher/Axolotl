import { defineMessages, type useVIntl } from '@modrinth/ui'

import type { ResourceOperationPlan } from './resources'

type FormatMessage = ReturnType<typeof useVIntl>['formatMessage']

const details = defineMessages({
    unknown: {
        id: 'app.resources.feedback.unknown.detail',
        defaultMessage:
            'The operation could not be completed. Retry, or reopen this review if it keeps failing.',
    },
    network: {
        id: 'app.resources.feedback.network.detail',
        defaultMessage:
            'Could not connect to the content source. Check your connection or proxy and retry.',
    },
    timeout: {
        id: 'app.resources.feedback.timeout.detail',
        defaultMessage: 'The request timed out. Check your connection and retry.',
    },
    tls: {
        id: 'app.resources.feedback.tls.detail',
        defaultMessage:
            'A secure connection could not be established. Check your system time, proxy and certificates, then retry.',
    },
    notFound: {
        id: 'app.resources.feedback.notFound.detail',
        defaultMessage:
            'The selected project or version is no longer available. Refresh the project and choose another version.',
    },
    rateLimited: {
        id: 'app.resources.feedback.rateLimited.detail',
        defaultMessage:
            'The content source is receiving too many requests. Wait a moment and retry.',
    },
    accessDenied: {
        id: 'app.resources.feedback.accessDenied.detail',
        defaultMessage:
            'The content source denied access. Check source access settings or choose another source.',
    },
    server: {
        id: 'app.resources.feedback.server.detail',
        defaultMessage:
            'The content source is temporarily failing. Retry later or choose another source.',
    },
    unavailable: {
        id: 'app.resources.feedback.unavailable.detail',
        defaultMessage:
            'The content source is unavailable. Check the connection and source settings, then retry.',
    },
    invalidResponse: {
        id: 'app.resources.feedback.invalidResponse.detail',
        defaultMessage:
            'The content source returned incomplete or invalid information. Retry or choose another version.',
    },
    noCompatibleRelease: {
        id: 'app.resources.feedback.noCompatibleRelease.detail',
        defaultMessage:
            'No version matches the current game version and loader. Open the project’s Versions tab and choose a specific version.',
    },
    catalogIncomplete: {
        id: 'app.resources.feedback.catalogIncomplete.detail',
        defaultMessage:
            'The version list could not be fully loaded. Open the project’s Versions tab and choose a specific version or file.',
    },
    chooseFile: {
        id: 'app.resources.feedback.chooseFile.detail',
        defaultMessage:
            'The selected version does not identify one installable file. Open the project’s Versions tab and choose a specific file.',
    },
    invalidArtifact: {
        id: 'app.resources.feedback.invalidArtifact.detail',
        defaultMessage:
            'The selected file is unavailable or its integrity information cannot be verified. Refresh the project or choose another file.',
    },
    identity: {
        id: 'app.resources.feedback.identity.detail',
        defaultMessage:
            'The selected content and installed entry do not match reliably. Refresh the instance and select the intended content again.',
    },
    manualDownload: {
        id: 'app.resources.feedback.manualDownload.detail',
        defaultMessage:
            'The source requires a manual download. Download the exact selected file from its project page, then import it into the instance.',
    },
    localMismatch: {
        id: 'app.resources.feedback.localMismatch.detail',
        defaultMessage:
            'The imported file does not match the selected version. Download the correct file and select it again.',
    },
    integrity: {
        id: 'app.resources.feedback.integrity.detail',
        defaultMessage:
            'The downloaded file failed verification. Retry the download or choose another source.',
    },
    path: {
        id: 'app.resources.feedback.path.detail',
        defaultMessage:
            'The installation path is invalid or outside the instance. Choose a valid file or target world and retry.',
    },
    occupied: {
        id: 'app.resources.feedback.occupied.detail',
        defaultMessage:
            'Another file already occupies the destination. Review the existing file, then remove or rename it if appropriate before retrying.',
    },
    permission: {
        id: 'app.resources.feedback.permission.detail',
        defaultMessage:
            'The launcher cannot write the file. Close programs using it and check the instance folder’s write permissions, then retry.',
    },
    diskFull: {
        id: 'app.resources.feedback.diskFull.detail',
        defaultMessage:
            'There is not enough disk space. Free space on the instance and cache drives, then retry.',
    },
    stale: {
        id: 'app.resources.feedback.stale.detail',
        defaultMessage: 'The instance or selected files changed. Refresh this review and retry.',
    },
    planExpired: {
        id: 'app.resources.feedback.planExpired.detail',
        defaultMessage:
            'This review is no longer current or was already submitted. Reopen the review and check existing tasks before trying again.',
    },
    instanceBusy: {
        id: 'app.resources.feedback.instanceBusy.detail',
        defaultMessage:
            'The instance is not ready for content changes. Wait for installation or other content tasks to finish, then retry.',
    },
    instanceMissing: {
        id: 'app.resources.feedback.instanceMissing.detail',
        defaultMessage:
            'The target instance could not be found. Return to the library and select an existing instance.',
    },
    cache: {
        id: 'app.resources.feedback.cache.detail',
        defaultMessage: 'The resource cache cannot be read. Restart the launcher and retry.',
    },
    database: {
        id: 'app.resources.feedback.database.detail',
        defaultMessage:
            'The launcher could not read or save the content state. Restart the launcher and retry.',
    },
    dependency: {
        id: 'app.resources.feedback.dependency.detail',
        defaultMessage:
            'A required dependency could not be prepared. Retry or select a compatible dependency version; installing only selected content will omit unresolved dependencies.',
    },
    analysis: {
        id: 'app.resources.feedback.analysis.detail',
        defaultMessage:
            'Dependency checks could not be completed. Retry the check; installing only selected content will skip these checks.',
    },
    selected: {
        id: 'app.resources.feedback.selected.detail',
        defaultMessage:
            'The selected content could not be prepared. Retry or select another version or file. Skipping dependencies cannot resolve this failure.',
    },
    constraints: {
        id: 'app.resources.feedback.constraints.detail',
        defaultMessage:
            'The dependency requirements cannot be satisfied together. Choose compatible versions or remove the conflicting selection.',
    },
    scope: {
        id: 'app.resources.feedback.scope.detail',
        defaultMessage:
            'This content cannot be installed in the selected location. Choose the appropriate instance or target world.',
    },
    missingFile: {
        id: 'app.resources.feedback.missingFile.detail',
        defaultMessage:
            'A required local file is missing, disabled, or ambiguous. Check the instance files and enable or install the intended dependency.',
    },
    backup: {
        id: 'app.resources.feedback.backup.detail',
        defaultMessage:
            'The rollback backup is unavailable or has changed. Refresh the backup list and choose an available backup.',
    },
    canceled: {
        id: 'app.resources.feedback.canceled.detail',
        defaultMessage: 'The content operation was cancelled. Retry when you are ready.',
    },
    unsupported: {
        id: 'app.resources.feedback.unsupported.detail',
        defaultMessage:
            'This source does not support the requested operation. Choose another source or an exact file.',
    },
})
const summaries = defineMessages({
    unknown: {
        id: 'app.resources.feedback.unknown.summary',
        defaultMessage: 'The operation could not be completed.',
    },
    network: {
        id: 'app.resources.feedback.network.summary',
        defaultMessage: 'Could not connect to the content source.',
    },
    timeout: {
        id: 'app.resources.feedback.timeout.summary',
        defaultMessage: 'The request timed out.',
    },
    tls: {
        id: 'app.resources.feedback.tls.summary',
        defaultMessage: 'A secure connection could not be established.',
    },
    notFound: {
        id: 'app.resources.feedback.notFound.summary',
        defaultMessage: 'The selected project or version is unavailable.',
    },
    rateLimited: {
        id: 'app.resources.feedback.rateLimited.summary',
        defaultMessage: 'Too many requests.',
    },
    accessDenied: {
        id: 'app.resources.feedback.accessDenied.summary',
        defaultMessage: 'The content source denied access.',
    },
    server: {
        id: 'app.resources.feedback.server.summary',
        defaultMessage: 'The content source is temporarily failing.',
    },
    unavailable: {
        id: 'app.resources.feedback.unavailable.summary',
        defaultMessage: 'The content source is unavailable.',
    },
    invalidResponse: {
        id: 'app.resources.feedback.invalidResponse.summary',
        defaultMessage: 'The content source returned invalid information.',
    },
    noCompatibleRelease: {
        id: 'app.resources.feedback.noCompatibleRelease.summary',
        defaultMessage: 'No matching version was found.',
    },
    catalogIncomplete: {
        id: 'app.resources.feedback.catalogIncomplete.summary',
        defaultMessage: 'The version list is incomplete.',
    },
    chooseFile: {
        id: 'app.resources.feedback.chooseFile.summary',
        defaultMessage: 'Select a specific installation file.',
    },
    invalidArtifact: {
        id: 'app.resources.feedback.invalidArtifact.summary',
        defaultMessage: 'The selected file cannot be verified.',
    },
    identity: {
        id: 'app.resources.feedback.identity.summary',
        defaultMessage: 'The content identity could not be confirmed.',
    },
    manualDownload: {
        id: 'app.resources.feedback.manualDownload.summary',
        defaultMessage: 'A manual download is required.',
    },
    localMismatch: {
        id: 'app.resources.feedback.localMismatch.summary',
        defaultMessage: 'The imported file does not match.',
    },
    integrity: {
        id: 'app.resources.feedback.integrity.summary',
        defaultMessage: 'The file failed verification.',
    },
    path: {
        id: 'app.resources.feedback.path.summary',
        defaultMessage: 'The installation path is invalid.',
    },
    occupied: {
        id: 'app.resources.feedback.occupied.summary',
        defaultMessage: 'The destination is occupied.',
    },
    permission: {
        id: 'app.resources.feedback.permission.summary',
        defaultMessage: 'The file cannot be written.',
    },
    diskFull: {
        id: 'app.resources.feedback.diskFull.summary',
        defaultMessage: 'There is not enough disk space.',
    },
    stale: {
        id: 'app.resources.feedback.stale.summary',
        defaultMessage: 'The instance content has changed.',
    },
    planExpired: {
        id: 'app.resources.feedback.planExpired.summary',
        defaultMessage: 'This review is no longer current.',
    },
    instanceBusy: {
        id: 'app.resources.feedback.instanceBusy.summary',
        defaultMessage: 'The instance is busy.',
    },
    instanceMissing: {
        id: 'app.resources.feedback.instanceMissing.summary',
        defaultMessage: 'The instance could not be found.',
    },
    cache: {
        id: 'app.resources.feedback.cache.summary',
        defaultMessage: 'The resource cache is unavailable.',
    },
    database: {
        id: 'app.resources.feedback.database.summary',
        defaultMessage: 'The content state could not be saved or read.',
    },
    dependency: {
        id: 'app.resources.feedback.dependency.summary',
        defaultMessage: 'A required dependency could not be prepared.',
    },
    analysis: {
        id: 'app.resources.feedback.analysis.summary',
        defaultMessage: 'Dependency checks could not be completed.',
    },
    selected: {
        id: 'app.resources.feedback.selected.summary',
        defaultMessage: 'The selected content could not be prepared.',
    },
    constraints: {
        id: 'app.resources.feedback.constraints.summary',
        defaultMessage: 'The dependency requirements conflict.',
    },
    scope: {
        id: 'app.resources.feedback.scope.summary',
        defaultMessage: 'The installation location is unsuitable.',
    },
    missingFile: {
        id: 'app.resources.feedback.missingFile.summary',
        defaultMessage: 'A required local file is unavailable.',
    },
    backup: {
        id: 'app.resources.feedback.backup.summary',
        defaultMessage: 'The rollback backup is unavailable.',
    },
    canceled: {
        id: 'app.resources.feedback.canceled.summary',
        defaultMessage: 'The operation was cancelled.',
    },
    unsupported: {
        id: 'app.resources.feedback.unsupported.summary',
        defaultMessage: 'The source does not support this operation.',
    },
})

export type ResourceFailureKind = keyof typeof details
export type ResourceFailureFeedback = { detail: string; summary: string; kind: ResourceFailureKind }

function message(error: unknown): string {
    return typeof error === 'object' && error !== null && 'message' in error
        ? String(error.message)
        : String(error)
}

export function resourceFailureKind(error: unknown): ResourceFailureKind | null {
    const text = message(error)
    if (/cancell?ed|resource request canceled/i.test(text)) return 'canceled'
    if (
        /HTTP status Some\(404\)|HTTP 404|release not found|project not found|unknown (release|project)|release is unavailable/i.test(
            text,
        )
    )
        return 'notFound'
    if (/HTTP status Some\(429\)|HTTP 429|too many requests/i.test(text)) return 'rateLimited'
    if (/HTTP status Some\((401|403)\)|HTTP (401|403)/i.test(text)) return 'accessDenied'
    if (/HTTP status Some\((408|504)\)|HTTP (408|504)/i.test(text)) return 'timeout'
    if (/HTTP status Some\(5\d\d\)|HTTP 5\d\d/i.test(text)) return 'server'
    if (/timed? ?out|timeout/i.test(text)) return 'timeout'
    if (/tls|ssl|certificate|cert error/i.test(text)) return 'tls'
    if (/request failed|connection|network|dns|offline|failed to fetch/i.test(text))
        return 'network'
    if (
        /resource source is unavailable|Source presentation is unavailable|source unavailable/i.test(
            text,
        )
    )
        return 'unavailable'
    if (
        /source release window is incomplete|release window exhausted|release cursor repeated|exceeded its window|source returned no release page/i.test(
            text,
        )
    )
        return 'catalogIncomplete'
    if (/no compatible release/i.test(text)) return 'noCompatibleRelease'
    if (
        /select one exact primary artifact|artifact has no filename|selected artifact is not part|no (primary|installable) (artifact|file)/i.test(
            text,
        )
    )
        return 'chooseFile'
    if (
        /invalid artifact integrity or ownership|invalid frozen artifact|artifact has no download URL/i.test(
            text,
        )
    )
        return 'invalidArtifact'
    if (
        /local file does not match|local import.*(match|changed)|imported file.*(match|changed)/i.test(
            text,
        )
    )
        return 'localMismatch'
    if (
        /hash.*mismatch|checksum|integrity.*(fail|mismatch)|downloaded.*(match|verif)|staged artifact/i.test(
            text,
        )
    )
        return 'integrity'
    if (/manual download/i.test(text)) return 'manualDownload'
    if (
        /belongs to another resource|target resource does not own|does not match.*(resource|project)|identity.*(ambiguous|unconfirmed)|entry ownership/i.test(
            text,
        )
    )
        return 'identity'
    if (/backup.*(unavailable|not available|changed)|recovery backup/i.test(text)) return 'backup'
    if (/no space|disk full|os error 112|os error 28|磁盘空间不足/i.test(text)) return 'diskFull'
    if (
        /access.*denied|permission denied|sharing violation|being used by another process|os error (5|32)|拒绝访问|另一个程序/i.test(
            text,
        )
    )
        return 'permission'
    if (/target already exists|destination already exists|path is occupied/i.test(text))
        return 'occupied'
    if (
        /invalid.*(path|filename)|path.*(escaped|traverse|symbolic)|paths cannot traverse|not a regular file|local imports must belong to selected roots/i.test(
            text,
        )
    )
        return 'path'
    if (/unknown instance/i.test(text)) return 'instanceMissing'
    if (/instance is not ready|active.*(install|task)|another.*(install|content).*task/i.test(text))
        return 'instanceBusy'
    if (
        /plan was revised|already queued|plan changed concurrently|plan revision|durable resource plan not found|reviewed resource plan|resource cursor expired|analysis session.*expired|unknown analysis session/i.test(
            text,
        )
    )
        return 'planExpired'
    if (
        /instance changed|stale.*revision|file.*changed|artifact changed|dependency (changed|disappeared)|entry disappeared|snapshot missing/i.test(
            text,
        )
    )
        return 'stale'
    if (/resource cache unavailable/i.test(text)) return 'cache'
    if (/resource database unavailable|database.*(locked|unavailable)/i.test(text))
        return 'database'
    if (
        /required local file is missing, disabled or ambiguous|no such file|cannot find the file|os error 2\b|系统找不到指定的文件/i.test(
            text,
        )
    )
        return 'missingFile'
    if (
        /content.*scope|unsupported content type|unambiguous installable content type|target world/i.test(
            text,
        )
    )
        return 'scope'
    if (
        /dependency.*(constraints|version range|version constraints|did not converge)|conflict.*installed dependent|dependency graph exceeded/i.test(
            text,
        )
    )
        return 'constraints'
    if (/dependency.*(metadata|verification)|cannot verify.*dependency/i.test(text))
        return 'analysis'
    if (/unsupported resource operation/i.test(text)) return 'unsupported'
    if (/invalid source response|source returned|unexpected JSON/i.test(text))
        return 'invalidResponse'
    return null
}

export function resourceFailureFeedback(
    error: unknown,
    formatMessage: FormatMessage,
    fallback: ResourceFailureKind = 'unknown',
): ResourceFailureFeedback {
    const kind = resourceFailureKind(error) ?? fallback
    return {
        kind,
        detail: formatMessage(details[kind]),
        summary: formatMessage(summaries[kind]),
    }
}

export function resourcePlanIssueFeedback(
    issue: ResourceOperationPlan['issues'][number],
    formatMessage: FormatMessage,
    selected: boolean,
): ResourceFailureFeedback {
    const fallback: ResourceFailureKind = (() => {
        switch (issue.code) {
            case 'missingDependency':
                return selected ? 'selected' : 'dependency'
            case 'dependencyAnalysisFailed':
                return 'analysis'
            case 'sourceUnavailable':
                return selected ? 'selected' : 'dependency'
            case 'invalidPath':
                return 'path'
            case 'fileNotWritable':
                return 'permission'
            case 'manualDownloadRequired':
                return 'manualDownload'
            case 'identityUnconfirmed':
            case 'artifactMismatch':
                return 'identity'
            case 'staleRevision':
                return 'stale'
            default:
                return 'unknown'
        }
    })()
    return resourceFailureFeedback(issue.message, formatMessage, fallback)
}
