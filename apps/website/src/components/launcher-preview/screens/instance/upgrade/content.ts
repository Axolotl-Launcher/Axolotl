import type { ContentItem } from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'

import { currentPreview } from '../../../lib/state'
import { contentFolderFor } from '../../../lib/state/downloads'
import { instanceContent, toLayoutItem } from '../state'

export interface InstanceContentSnapshotItem {
    entryId: string
    memberId: string | null
    fileId: string
    expectedRelativePath: string
    provider: 'modrinth' | 'local'
    providerProjectId: string | null
    providerReleaseId: string | null
    projectType: string
    content: ContentItem & { provider_refs: { provider: string; project_id: string }[] }
}
export interface InstanceContentSnapshot {
    items: InstanceContentSnapshotItem[]
}
export interface InstanceContentData {
    snapshot: InstanceContentSnapshot
    contentItems: ContentItem[]
    linkedContentItems: ContentItem[]
}

export function localContentIconUrl(icon?: string | null) {
    return icon ?? ''
}

export async function loadInstanceContentData(instanceId: string): Promise<InstanceContentData> {
    const domain = instanceContent(currentPreview())
    await domain.ensureLoaded(instanceId)
    const items = domain.sharedItems(instanceId).filter((item) => !item.removed)
    const contentItems = items.map((item) =>
        toLayoutItem(instanceId, item, domain.metaFor(instanceId, item.id), null, []),
    )
    return {
        contentItems,
        linkedContentItems: [],
        snapshot: {
            items: items.map((item, index) => ({
                entryId: item.id,
                memberId: null,
                fileId: item.id,
                expectedRelativePath: `${item.folder ?? contentFolderFor(item.project_type)}/${item.file_name}`,
                provider: item.project_id ? 'modrinth' : 'local',
                providerProjectId: item.project_id,
                providerReleaseId: item.version_id,
                projectType: item.project_type,
                content: {
                    ...contentItems[index],
                    provider_refs: item.project_id
                        ? [{ provider: 'modrinth', project_id: item.project_id }]
                        : [],
                },
            })),
        },
    }
}

export async function get_content_snapshot(instanceId: string) {
    return (await loadInstanceContentData(instanceId)).snapshot
}
