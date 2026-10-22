import { invoke } from '@tauri-apps/api/core'

import { restoreCachedUrls } from './download-url'
import {
    resourceCachePolicy,
    resourceLocate,
    resourceProjectDocuments,
    resourceReleaseDocuments,
    resourceRequest,
} from './resources'

async function readCache(command, args) {
    return restoreCachedUrls(await invoke(command, args))
}

/** @returns {Promise<import('@modrinth/api-client').Labrinth.Projects.v2.Project & { resourceDates: Pick<import('./resources').SourceProjectMetadata, 'createdAt' | 'projectPublishedAt' | 'updatedAt' | 'firstPublishedAt' | 'latestPublishedAt'> }>} */
export async function get_project(id, cacheBehaviour) {
    const document = await resourceLocate(
        'modrinth',
        id,
        undefined,
        resourceCachePolicy(cacheBehaviour),
    )
    if (!document?.protocol) throw new Error('Source presentation is unavailable')
    return restoreCachedUrls({
        ...document.protocol,
        resourceDates: {
            createdAt: document.metadata.createdAt,
            projectPublishedAt: document.metadata.projectPublishedAt,
            updatedAt: document.metadata.updatedAt,
            firstPublishedAt: document.metadata.firstPublishedAt,
            latestPublishedAt: document.metadata.latestPublishedAt,
        },
    })
}

/** @returns {Promise<import('@modrinth/api-client').Labrinth.Projects.v2.Project[]>} */
export async function get_project_many(ids, cacheBehaviour) {
    return (
        await resourceProjectDocuments(
            'modrinth',
            ids,
            undefined,
            resourceCachePolicy(cacheBehaviour),
        )
    ).map((document) => {
        if (!document.protocol) throw new Error('Source presentation is unavailable')
        return restoreCachedUrls(document.protocol)
    })
}

export async function get_project_v3(id, cacheBehaviour) {
    return (await get_project_v3_many([id], cacheBehaviour))[0] ?? null
}

export async function get_project_v3_many(ids, cacheBehaviour) {
    const documents = await resourceRequest('resource_project_protocol', {
        locators: ids,
        protocolVersion: 'v3',
        cachePolicy: resourceCachePolicy(cacheBehaviour),
    })
    return documents.map((document) => {
        if (!document.protocol) throw new Error('Source presentation is unavailable')
        return restoreCachedUrls(document.protocol)
    })
}

/** @returns {Promise<import('@modrinth/api-client').Labrinth.Versions.v2.Version>} */
export async function get_version(id, cacheBehaviour) {
    return (await get_version_many([id], cacheBehaviour))[0]
}

/** @returns {Promise<import('@modrinth/api-client').Labrinth.Versions.v2.Version[]>} */
export async function get_version_many(ids, cacheBehaviour) {
    const documents = await resourceReleaseDocuments(
        'modrinth',
        ids,
        undefined,
        undefined,
        resourceCachePolicy(cacheBehaviour),
    )
    return documents.map((document) => {
        if (!document.protocol) throw new Error('Source presentation is unavailable')
        return restoreCachedUrls(document.protocol)
    })
}

export async function get_user(id, cacheBehaviour) {
    return await readCache('plugin:cache|get_user', { id, cacheBehaviour })
}

export async function get_user_many(ids, cacheBehaviour) {
    return await readCache('plugin:cache|get_user_many', { ids, cacheBehaviour })
}

export async function get_team(id, cacheBehaviour) {
    return await readCache('plugin:cache|get_team', { id, cacheBehaviour })
}

export async function get_team_many(ids, cacheBehaviour) {
    return await readCache('plugin:cache|get_team_many', { ids, cacheBehaviour })
}

export async function get_organization(id, cacheBehaviour) {
    return await readCache('plugin:cache|get_organization', { id, cacheBehaviour })
}

export async function get_organization_many(ids, cacheBehaviour) {
    return await readCache('plugin:cache|get_organization_many', { ids, cacheBehaviour })
}

export async function get_search_results(id, cacheBehaviour) {
    return await readCache('plugin:cache|get_search_results', { id, cacheBehaviour })
}

export async function get_search_results_many(ids, cacheBehaviour) {
    return await readCache('plugin:cache|get_search_results_many', { ids, cacheBehaviour })
}

export async function get_search_results_v3(id, cacheBehaviour, requestId) {
    return await readCache('plugin:cache|get_search_results_v3', { id, cacheBehaviour, requestId })
}

export async function get_search_results_v3_many(ids, cacheBehaviour) {
    return await readCache('plugin:cache|get_search_results_v3_many', { ids, cacheBehaviour })
}

export async function cancel_search_request(requestId) {
    return await invoke('plugin:cache|cancel_search_request', { requestId })
}

export async function purge_cache_types(cacheTypes) {
    return await invoke('plugin:cache|purge_cache_types', { cacheTypes })
}

/**
 * Get versions for a project (without changelogs for fast loading).
 * Uses the cache system - versions are cached for 30 minutes.
 * @param {string} projectId - The project ID
 * @param {string} [cacheBehaviour] - Cache behaviour ('must_revalidate', etc.)
 * @returns {Promise<Array|null>} Array of version objects (without changelogs) or null
 */
export async function get_project_versions(projectId, cacheBehaviour) {
    return readCache('plugin:cache|get_project_versions', { projectId, cacheBehaviour })
}

/** Clears rebuildable metadata and downloaded files and reports disk usage. */
export async function purgeLauncherCache() {
    return await invoke('plugin:storage|storage_cache_clear')
}
