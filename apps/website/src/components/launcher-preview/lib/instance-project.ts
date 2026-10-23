import type { InstanceLink } from './types'

/** `SourceProjectRef` of the app's `helpers/resources.ts`. */
export interface SourceProjectRef {
    source: 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
    projectId: string
}

/* Port of the app's `helpers/instance-project.ts`. */

/** A link's IDs belong to its recorded provider, regardless of their format. */
export function instanceContentProject(link?: InstanceLink | null): SourceProjectRef | null {
    switch (link?.type) {
        case 'curseforge_modpack':
            return { source: 'curseforge', projectId: link.project_id }
        case 'modrinth_modpack':
        case 'server_project':
            return { source: 'modrinth', projectId: link.project_id }
        case 'server_project_modpack':
            return {
                source: 'modrinth',
                projectId: link.content_project_id ?? link.project_id ?? link.server_project_id,
            }
        default:
            return null
    }
}

/** Modrinth server metadata must never be queried with another source's ID. */
export function instanceServerProjectId(link?: InstanceLink | null): string | null {
    switch (link?.type) {
        case 'modrinth_modpack':
        case 'server_project':
            return link.project_id
        case 'server_project_modpack':
            return link.server_project_id
        default:
            return null
    }
}

export function isServerProjectLink(link?: InstanceLink | null): boolean {
    return link?.type === 'server_project' || link?.type === 'server_project_modpack'
}
