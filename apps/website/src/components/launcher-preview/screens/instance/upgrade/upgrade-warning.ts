import type { InstanceUpgradeIssueCode, InstanceUpgradeResult } from './backend'

export interface UpgradeWarningRow {
    key: string
    code: InstanceUpgradeIssueCode | null
    contentId: string | null
    relativePath: string | null
    provider: string | null
    projectId: string | null
    legacyMessage: string | null
}

export type UpgradeWarningCategory = 'local' | 'kept' | 'fallback'
export type UpgradeWarningFilter = 'all' | UpgradeWarningCategory

export const UPGRADE_WARNING_PAGE_SIZE = 10

export interface UpgradeWarningSummary {
    local: number
    kept: number
    fallback: number
}

export interface UpgradeWarningPage {
    items: UpgradeWarningRow[]
    page: number
    pageCount: number
    start: number
    end: number
    total: number
}

export function upgradeWarningMessageId(code: InstanceUpgradeIssueCode): string {
    return `instance.upgrade.warning.${code.replaceAll('_', '-')}`
}

export function upgradeResultWarningRows(result: InstanceUpgradeResult): UpgradeWarningRow[] {
    if (result.compatibilityWarningDetails !== undefined) {
        return result.compatibilityWarningDetails.map((warning, index) => ({
            key: `${warning.code}:${warning.contentId ?? warning.relativePath ?? index}`,
            code: warning.code,
            contentId: warning.contentId,
            relativePath: warning.relativePath,
            provider: warning.provider,
            projectId: warning.projectId,
            legacyMessage: null,
        }))
    }
    return result.compatibilityWarnings.map((warning, index) => ({
        key: `${warning.code}:${warning.contentId ?? index}`,
        code: null,
        contentId: warning.contentId,
        relativePath: null,
        provider: warning.provider,
        projectId: warning.projectId,
        legacyMessage: warning.message || warning.code,
    }))
}

export function upgradeWarningCategory(row: UpgradeWarningRow): UpgradeWarningCategory {
    if (row.code === 'unidentified' || row.code === 'unsupported_content_type') return 'local'
    if (row.code === 'keep_incompatible' || row.code === 'no_compatible_release') return 'kept'
    return 'fallback'
}

export function summarizeUpgradeWarnings(rows: UpgradeWarningRow[]): UpgradeWarningSummary {
    const summary: UpgradeWarningSummary = { local: 0, kept: 0, fallback: 0 }
    for (const row of rows) summary[upgradeWarningCategory(row)] += 1
    return summary
}

export function filterUpgradeWarnings(
    rows: UpgradeWarningRow[],
    filter: UpgradeWarningFilter,
    query: string,
    searchFields: (
        row: UpgradeWarningRow,
    ) => Array<string | null | undefined> = defaultWarningSearchFields,
): UpgradeWarningRow[] {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return rows.filter((row) => {
        if (filter !== 'all' && upgradeWarningCategory(row) !== filter) return false
        if (!normalizedQuery) return true
        return searchFields(row).some((value) =>
            value?.toLocaleLowerCase().includes(normalizedQuery),
        )
    })
}

export function paginateUpgradeWarnings(
    rows: UpgradeWarningRow[],
    requestedPage: number,
    pageSize = UPGRADE_WARNING_PAGE_SIZE,
): UpgradeWarningPage {
    const pageCount = Math.max(1, Math.ceil(rows.length / pageSize))
    const page = Math.min(Math.max(1, requestedPage), pageCount)
    const startIndex = (page - 1) * pageSize
    return {
        items: rows.slice(startIndex, startIndex + pageSize),
        page,
        pageCount,
        start: rows.length ? startIndex + 1 : 0,
        end: Math.min(startIndex + pageSize, rows.length),
        total: rows.length,
    }
}

export function upgradeWarningDisplayName(row: UpgradeWarningRow): string | null {
    const path = row.relativePath?.replaceAll('\\', '/')
    const filename = path?.split('/').filter(Boolean).at(-1)
    return filename ?? row.projectId ?? row.contentId
}

export function upgradeWarningContentKind(row: UpgradeWarningRow): string {
    const path = row.relativePath?.replaceAll('\\', '/').toLocaleLowerCase()
    if (path?.startsWith('resourcepacks/')) return 'resourcepack'
    if (path?.startsWith('shaderpacks/')) return 'shaderpack'
    if (path?.startsWith('datapacks/')) return 'datapack'
    if (path?.startsWith('mods/')) return 'mod'
    return 'content'
}

function defaultWarningSearchFields(row: UpgradeWarningRow) {
    return [
        row.contentId,
        row.relativePath,
        row.code,
        row.provider,
        row.projectId,
        row.legacyMessage,
    ]
}
