import { loadProject, projectIcon } from '../../../data'
import { getGameVersions, getLoaderVersions } from '../settings/installation/metadata'
import {
    type UpgradeReleaseIdentity,
    upgradeVersionCacheKey,
    type UpgradeVersionDisplayMetadata,
} from './upgrade-version-display'

export type { LoaderMetadataManifest as Manifest } from '../settings/installation/metadata'
export type {
    UpgradeReleaseIdentity,
    UpgradeVersionDisplayMetadata,
} from './upgrade-version-display'
export { upgradeVersionCacheKey, upgradeVersionDisplayLabel } from './upgrade-version-display'
export const get_game_versions = getGameVersions
export const get_loader_versions = getLoaderVersions

export interface UpgradeProjectIdentity {
    provider: string
    projectId: string
}
export interface UpgradeProjectDisplayMetadata {
    title: string
    iconUrl: string | null
}
export function upgradeProjectDisplayCacheKey(provider: string, projectId: string) {
    return `${provider}:${projectId}`
}
export async function loadUpgradeProjectDisplayMetadata(identities: UpgradeProjectIdentity[]) {
    const result = new Map<string, UpgradeProjectDisplayMetadata>()
    const unique = new Map(identities.map((item) => [item.projectId, item]))
    for (const identity of unique.values()) {
        if (identity.provider !== 'modrinth') continue
        const project = await loadProject(identity.projectId).catch(() => null)
        if (project)
            result.set(upgradeProjectDisplayCacheKey(identity.provider, identity.projectId), {
                title: project.title,
                iconUrl: projectIcon(project.id),
            })
    }
    return result
}
export async function loadUpgradeVersionDisplayMetadata(identities: UpgradeReleaseIdentity[]) {
    const result = new Map<string, UpgradeVersionDisplayMetadata>()
    for (const identity of identities) {
        if (identity.provider !== 'modrinth') continue
        const project = await loadProject(identity.projectId).catch(() => null)
        const version = project?.versions.find((entry) => entry.id === identity.releaseId)
        if (version)
            result.set(
                upgradeVersionCacheKey(identity.provider, identity.projectId, identity.releaseId),
                {
                    version: version.version_number,
                    channel: version.version_type,
                },
            )
    }
    return result
}
export async function loadUpgradeVersionMetadata(
    provider: string,
    projectId: string,
    releaseId: string,
) {
    if (provider !== 'modrinth') return null
    const project = await loadProject(projectId).catch(() => null)
    const version = project?.versions.find((entry) => entry.id === releaseId)
    return version
        ? {
              version: version.version_number,
              channel: version.version_type,
              changelog: version.changelog ?? null,
          }
        : null
}
