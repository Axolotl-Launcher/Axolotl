import type { ContentItem } from '@modrinth/ui'

import { sourceProjectUrl } from './source-project-links.ts'

export function contentProjectUrl(item: ContentItem): string | null {
    const source = item.origin_provider ?? item.provider_refs?.[0]?.provider
    const project = item.project
    if (!project || project.id.startsWith('local:')) return null
    if (source === 'modrinth') {
        const id = item.provider_refs?.find(
            (reference) => reference.provider === 'modrinth',
        )?.project_id
        return id ? sourceProjectUrl('modrinth', String(id)) : null
    }
    if (source === 'curseforge') {
        const id = item.provider_refs?.find(
            (reference) => reference.provider === 'curseforge',
        )?.project_id
        return id ? sourceProjectUrl('curseforge', String(id)) : null
    }
    return null
}
