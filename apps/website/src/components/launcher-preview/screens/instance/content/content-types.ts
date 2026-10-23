import type { ProjectDetail, ProjectVersion } from '../../../data'
import type { GameInstance } from '../../../lib/types'

/* Content-type rules of the instance content list. */

/** Project types the content list knows, keyed by the snapshot's `project_type`. */
export type ContentProjectType = 'mod' | 'resourcepack' | 'shader' | 'datapack'

export function contentProjectType(
    project: ProjectDetail,
    instance: GameInstance,
): ContentProjectType {
    if (project.project_type === 'resourcepack') return 'resourcepack'
    if (project.project_type === 'shader') return 'shader'
    if (project.project_type === 'datapack') return 'datapack'
    // Vanilla instances install data-driven "mods" (Terralith, Structory, ...) as datapacks.
    if (instance.loader === 'vanilla') return 'datapack'
    return 'mod'
}

/** Releases of `project` the instance can run, newest first (the app's compatible list). */
export function compatibleVersions(
    project: ProjectDetail,
    instance: GameInstance,
): ProjectVersion[] {
    const type = contentProjectType(project, instance)
    const versions = [...project.versions].sort(
        (a, b) => Date.parse(b.date_published) - Date.parse(a.date_published),
    )
    const matches = versions.filter((version) => {
        if (type === 'shader') return true
        if (!version.game_versions.includes(instance.game_version)) return false
        if (type === 'resourcepack') return true
        if (type === 'datapack')
            return version.loaders.includes('datapack') || version.loaders.includes(instance.loader)
        return version.loaders.includes(instance.loader)
    })
    return matches.length ? matches : versions.slice(0, 1)
}
