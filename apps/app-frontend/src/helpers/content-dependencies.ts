import type { ContentItem } from '@modrinth/ui'

type DependencyReference = {
    provider: 'modrinth' | 'curseforge' | 'local'
    projectId: string
}

export function dependencyRefMatchesContentItem(
    reference: DependencyReference,
    item: { project?: { id: string } | null; provider_refs: ContentItem['provider_refs'] },
): boolean {
    const projectId =
        reference.provider === 'curseforge'
            ? reference.projectId.replace(/^curseforge:/, '')
            : reference.projectId
    if (reference.provider === 'local' && projectId.startsWith('local:')) {
        return item.project?.id === projectId
    }
    if (item.provider_refs.length > 0) {
        return item.provider_refs.some(
            (ref) => ref.provider === reference.provider && String(ref.project_id) === projectId,
        )
    }

    const itemProjectId = item.project?.id ?? ''
    return reference.provider === 'curseforge'
        ? itemProjectId.replace(/^curseforge:/, '') === projectId
        : itemProjectId === projectId
}
