import type { ContentItem } from '@modrinth/ui'

import type { PendingContentChange } from '@/providers/download-manager.ts'

import type { InstallJobSnapshot } from './install.ts'
import { isActiveInstallJobStatus } from './install-job-status.ts'

export function activeContentChangeJobs(
    jobs: InstallJobSnapshot[],
    instanceId: string,
): InstallJobSnapshot[] {
    return jobs.filter(
        (job) =>
            job.kind === 'change_content' &&
            (job.instance_id ?? job.target.instance_id ?? null) === instanceId &&
            isActiveInstallJobStatus(job.status),
    )
}

export function contentItemStableId(item: ContentItem): string | null {
    return item.instanceEntryId ?? item.instanceMemberId ?? item.instanceFileId ?? null
}

export function indexContentChanges(
    jobs: InstallJobSnapshot[],
    pendingChanges: PendingContentChange[],
) {
    const contentIds = new Set<string>()
    let allUserAdded = false
    for (const job of jobs) {
        const change = job.content_change
        if (!change) continue
        if (change.intent.type === 'update_all_user_added' && change.content_ids.length === 0) {
            allUserAdded = true
        }
        for (const id of change.content_ids) contentIds.add(id)
    }
    for (const pending of pendingChanges) {
        if (pending.updateAll) allUserAdded = true
        else for (const id of pending.contentIds) contentIds.add(id)
    }
    return { contentIds, allUserAdded }
}

export function contentChangeAffectsItem(
    index: ReturnType<typeof indexContentChanges>,
    item: ContentItem,
): boolean {
    if (index.allUserAdded && item.instanceOwnershipKind === 'user_added') return true
    const contentId = contentItemStableId(item)
    return contentId != null && index.contentIds.has(contentId)
}
