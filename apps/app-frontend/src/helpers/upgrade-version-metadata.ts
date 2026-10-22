import { LRUCache } from 'lru-cache'

import { get_project_many, get_version_many } from './cache.js'
import {
    getCurseForgeChangelog,
    getCurseForgeFile,
    getCurseForgeImageUrl,
    getCurseForgeProjects,
} from './curseforge'
import {
    type UpgradeReleaseIdentity,
    upgradeVersionCacheKey,
    type UpgradeVersionDisplayMetadata,
} from './upgrade-version-display'

export {
    type UpgradeReleaseIdentity,
    upgradeVersionCacheKey,
    upgradeVersionDisplayLabel,
    type UpgradeVersionDisplayMetadata,
} from './upgrade-version-display'

export interface UpgradeVersionMetadata extends UpgradeVersionDisplayMetadata {
    changelog: string | null
}

const displayCache = new LRUCache<string, UpgradeVersionDisplayMetadata>({ max: 2048 })
const changelogCache = new LRUCache<string, UpgradeVersionMetadata>({
    max: 200,
    maxSize: 4 * 1024 * 1024,
    sizeCalculation: (value, key) => Math.max(1, (JSON.stringify(value).length + key.length) * 2),
})
const projectDisplayCache = new LRUCache<string, UpgradeProjectDisplayMetadata>({ max: 2048 })

export interface UpgradeProjectIdentity {
    provider: 'modrinth' | 'curseforge'
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
    const unique = [
        ...new Map(
            identities.map((identity) => [
                upgradeProjectDisplayCacheKey(identity.provider, identity.projectId),
                identity,
            ]),
        ).values(),
    ]
    const result = new Map<string, UpgradeProjectDisplayMetadata>()
    for (const identity of unique) {
        const key = upgradeProjectDisplayCacheKey(identity.provider, identity.projectId)
        const cached = projectDisplayCache.get(key)
        if (cached) result.set(key, cached)
    }
    const missing = unique.filter(
        (identity) =>
            !result.has(upgradeProjectDisplayCacheKey(identity.provider, identity.projectId)),
    )
    const modrinthIds = missing
        .filter((identity) => identity.provider === 'modrinth')
        .map((identity) => identity.projectId)
    const curseForgeIds = missing
        .filter((identity) => identity.provider === 'curseforge')
        .map((identity) => Number(identity.projectId))
        .filter((projectId) => Number.isSafeInteger(projectId))
    const [modrinthProjects, curseForgeProjects] = await Promise.all([
        modrinthIds.length
            ? (get_project_many(modrinthIds).catch(() => []) as Promise<
                  Array<{ id: string; title: string; icon_url?: string | null }>
              >)
            : [],
        curseForgeIds.length ? getCurseForgeProjects(curseForgeIds).catch(() => []) : [],
    ])
    for (const project of modrinthProjects) {
        const key = upgradeProjectDisplayCacheKey('modrinth', project.id)
        const metadata = {
            title: project.title,
            iconUrl: project.icon_url ?? null,
        }
        result.set(key, metadata)
        projectDisplayCache.set(key, metadata)
    }
    for (const project of curseForgeProjects) {
        const key = upgradeProjectDisplayCacheKey('curseforge', String(project.id))
        const metadata = {
            title: project.name,
            iconUrl: getCurseForgeImageUrl(project.logo?.thumbnailUrl ?? project.logo?.url) ?? null,
        }
        result.set(key, metadata)
        projectDisplayCache.set(key, metadata)
    }
    return result
}

async function loadCurseForgeDisplayMetadata(
    identities: UpgradeReleaseIdentity[],
    result: Map<string, UpgradeVersionDisplayMetadata>,
) {
    const queue = [...identities]
    await Promise.all(
        Array.from({ length: Math.min(4, queue.length) }, async () => {
            for (let identity = queue.shift(); identity; identity = queue.shift()) {
                const file = await getCurseForgeFile(
                    Number(identity.projectId),
                    Number(identity.releaseId),
                )
                const key = upgradeVersionCacheKey(
                    identity.provider,
                    identity.projectId,
                    identity.releaseId,
                )
                const metadata = {
                    version: file.displayName ?? file.fileName ?? identity.releaseId,
                    channel: file.releaseType,
                }
                result.set(key, metadata)
                displayCache.set(key, metadata)
            }
        }),
    )
}

export async function loadUpgradeVersionDisplayMetadata(identities: UpgradeReleaseIdentity[]) {
    const unique = [
        ...new Map(
            identities.map((identity) => [
                upgradeVersionCacheKey(identity.provider, identity.projectId, identity.releaseId),
                identity,
            ]),
        ).values(),
    ]
    const result = new Map<string, UpgradeVersionDisplayMetadata>()
    for (const identity of unique) {
        const key = upgradeVersionCacheKey(
            identity.provider,
            identity.projectId,
            identity.releaseId,
        )
        const cached = displayCache.get(key)
        if (cached) result.set(key, cached)
    }
    const missing = unique.filter(
        (identity) =>
            !result.has(
                upgradeVersionCacheKey(identity.provider, identity.projectId, identity.releaseId),
            ),
    )
    const modrinth = missing.filter((identity) => identity.provider === 'modrinth')
    if (modrinth.length) {
        const versions = (await get_version_many([
            ...new Set(modrinth.map((item) => item.releaseId)),
        ])) as Array<{
            id: string
            version_number?: string
            version_type?: string
        }>
        const byId = new Map(versions.map((version) => [version.id, version]))
        for (const identity of modrinth) {
            const version = byId.get(identity.releaseId)
            const key = upgradeVersionCacheKey(
                identity.provider,
                identity.projectId,
                identity.releaseId,
            )
            const metadata = {
                version: version?.version_number ?? identity.releaseId,
                channel: version?.version_type,
            }
            result.set(key, metadata)
            displayCache.set(key, metadata)
        }
    }
    await loadCurseForgeDisplayMetadata(
        missing.filter((identity) => identity.provider === 'curseforge'),
        result,
    )
    return result
}

export async function loadUpgradeVersionMetadata(
    provider: string,
    projectId: string,
    releaseId: string,
) {
    const key = upgradeVersionCacheKey(provider, projectId, releaseId)
    const cached = changelogCache.get(key)
    if (cached) return cached
    if (provider === 'modrinth') {
        const versions = (await get_version_many([releaseId])) as Array<{
            id: string
            version_number?: string
            version_type?: string
            changelog?: string | null
        }>
        const version = versions[0]
        const result = {
            version: version?.version_number ?? releaseId,
            channel: version?.version_type,
            changelog: version?.changelog ?? null,
        }
        displayCache.set(key, { version: result.version, channel: result.channel })
        changelogCache.set(key, result)
        return result
    }
    if (provider === 'curseforge') {
        const [file, changelog] = await Promise.all([
            getCurseForgeFile(Number(projectId), Number(releaseId)),
            getCurseForgeChangelog(Number(projectId), Number(releaseId)),
        ])
        const result = {
            version: file.displayName ?? file.fileName ?? releaseId,
            channel: file.releaseType,
            changelog: changelog || null,
        }
        displayCache.set(key, { version: result.version, channel: result.channel })
        changelogCache.set(key, result)
        return result
    }
    return { version: releaseId, changelog: null }
}
