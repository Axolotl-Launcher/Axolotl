import type {
    ContentProvider,
    InstanceUpgradeDependencyChangeKind,
    InstanceUpgradeSolution,
} from './backend'
import type { UpgradeProjectIdentity, UpgradeReleaseIdentity } from './metadata'

export const UPGRADE_RESULT_PAGE_SIZE = 25

export type UpgradeDetailFilter = 'all' | 'updated' | 'kept' | 'disabled' | 'dependencies'

export interface UpgradeDetailItem {
    key: string
    kind: 'selection' | 'dependency'
    contentId: string | null
    provider: ContentProvider | null
    projectId: string | null
    currentReleaseId: string | null
    targetReleaseId: string | null
    action: 'upgrade' | 'keep' | 'disable' | InstanceUpgradeDependencyChangeKind
}

export interface UpgradeDetailPage {
    items: UpgradeDetailItem[]
    page: number
    pageCount: number
    start: number
    end: number
    total: number
}

export function upgradeDetailItems(solution: InstanceUpgradeSolution): UpgradeDetailItem[] {
    return [
        ...solution.selections.map((selection) => ({
            key: `selection:${selection.contentId}`,
            kind: 'selection' as const,
            contentId: selection.contentId,
            provider: selection.provider,
            projectId: selection.projectId,
            currentReleaseId: selection.currentReleaseId,
            targetReleaseId: selection.targetReleaseId,
            action: selection.action,
        })),
        ...solution.dependencyChanges.map((change, index) => ({
            key: `dependency:${change.provider}:${change.projectId}:${change.existingContentId ?? index}`,
            kind: 'dependency' as const,
            contentId: change.existingContentId,
            provider: change.provider,
            projectId: change.projectId,
            currentReleaseId: change.currentReleaseId,
            targetReleaseId: change.targetReleaseId,
            action: change.kind,
        })),
    ]
}

export function filterUpgradeDetailItems(
    items: UpgradeDetailItem[],
    filter: UpgradeDetailFilter,
    query: string,
    searchFields: (
        item: UpgradeDetailItem,
    ) => Array<string | null | undefined> = defaultSearchFields,
): UpgradeDetailItem[] {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return items.filter((item) => {
        if (filter === 'dependencies' && item.kind !== 'dependency') return false
        if (filter === 'updated' && (item.kind !== 'selection' || item.action !== 'upgrade'))
            return false
        if (filter === 'kept' && (item.kind !== 'selection' || item.action !== 'keep')) return false
        if (filter === 'disabled' && (item.kind !== 'selection' || item.action !== 'disable'))
            return false
        if (!normalizedQuery) return true
        return searchFields(item).some((value) =>
            value?.toLocaleLowerCase().includes(normalizedQuery),
        )
    })
}

export function paginateUpgradeDetailItems(
    items: UpgradeDetailItem[],
    requestedPage: number,
    pageSize = UPGRADE_RESULT_PAGE_SIZE,
): UpgradeDetailPage {
    const pageCount = Math.max(1, Math.ceil(items.length / pageSize))
    const page = Math.min(Math.max(1, requestedPage), pageCount)
    const startIndex = (page - 1) * pageSize
    return {
        items: items.slice(startIndex, startIndex + pageSize),
        page,
        pageCount,
        start: items.length ? startIndex + 1 : 0,
        end: Math.min(startIndex + pageSize, items.length),
        total: items.length,
    }
}

export function upgradeDetailProjectIdentities(
    items: UpgradeDetailItem[],
): UpgradeProjectIdentity[] {
    const identities = new Map<string, UpgradeProjectIdentity>()
    for (const item of items) {
        if (item.provider !== 'modrinth' && item.provider !== 'curseforge') continue
        if (!item.projectId) continue
        identities.set(`${item.provider}:${item.projectId}`, {
            provider: item.provider,
            projectId: item.projectId,
        })
    }
    return [...identities.values()]
}

export function upgradeDetailReleaseIdentities(
    items: UpgradeDetailItem[],
): UpgradeReleaseIdentity[] {
    const identities = new Map<string, UpgradeReleaseIdentity>()
    for (const item of items) {
        if (item.provider !== 'modrinth' && item.provider !== 'curseforge') continue
        if (!item.projectId) continue
        for (const releaseId of [item.currentReleaseId, item.targetReleaseId]) {
            if (!releaseId) continue
            identities.set(`${item.provider}:${item.projectId}:${releaseId}`, {
                provider: item.provider,
                projectId: item.projectId,
                releaseId,
            })
        }
    }
    return [...identities.values()]
}

function defaultSearchFields(item: UpgradeDetailItem) {
    return [
        item.contentId,
        item.provider,
        item.projectId,
        item.currentReleaseId,
        item.targetReleaseId,
    ]
}
