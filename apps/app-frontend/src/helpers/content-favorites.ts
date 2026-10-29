import { invoke } from '@tauri-apps/api/core'

import {
    type ResourceCachePolicy,
    resourceGet,
    resourceLocate,
    resourceProjectDocuments,
    type ResourceType,
    type SourceProjectMetadata,
    type SourceProjectRef,
} from './resources.ts'

export async function resolveContentFavorite(favorite: ContentFavorite, signal?: AbortSignal) {
    if (!favorite.resource_id) {
        try {
            return (
                await resourceLocate(
                    favorite.provider,
                    favorite.locator ?? favorite.project_id,
                    signal,
                )
            ).metadata
        } catch {
            signal?.throwIfAborted()
            return (
                await resourceLocate(
                    favorite.provider,
                    favorite.locator ?? favorite.project_id,
                    signal,
                    'onlyCached',
                )
            ).metadata
        }
    }
    const resource = await resourceGet(
        { kind: 'resource', id: favorite.resource_id },
        undefined,
        undefined,
        signal,
    )
    const metadata = resource.projects.find(
        (project) =>
            project.project.source === resource.defaultProject.source &&
            project.project.projectId === resource.defaultProject.projectId,
    )
    if (!metadata) throw new Error('Favorite resource presentation is unavailable')
    return metadata
}

export async function prefetchContentFavorites(
    favorites: ContentFavorite[],
    signal?: AbortSignal,
    cachePolicy: ResourceCachePolicy = 'useFresh',
    onSourceSettled?: (
        source: FavoriteProvider,
        metadata: ReadonlyMap<string, SourceProjectMetadata>,
    ) => void,
) {
    const metadata = new Map<string, SourceProjectMetadata>()
    const projects = favorites.flatMap(
        (favorite) =>
            favorite.projects ?? [{ source: favorite.provider, projectId: favorite.project_id }],
    )
    const sources = [...new Set(projects.map((project) => project.source))]
    await Promise.allSettled(
        sources
            .filter(
                (source) =>
                    cachePolicy === 'onlyCached' ||
                    source === 'modrinth' ||
                    source === 'curseforge',
            )
            .map(async (source) => {
                try {
                    const ids = [
                        ...new Set(
                            projects
                                .filter((project) => project.source === source)
                                .map((project) => project.projectId),
                        ),
                    ]
                    let documents
                    try {
                        documents = await resourceProjectDocuments(source, ids, signal, cachePolicy)
                    } catch {
                        signal?.throwIfAborted()
                        if (cachePolicy === 'onlyCached') {
                            if (ids.length === 1) return
                            const cached = await Promise.allSettled(
                                ids.map((id) =>
                                    resourceProjectDocuments(source, [id], signal, 'onlyCached'),
                                ),
                            )
                            documents = cached.flatMap((result) =>
                                result.status === 'fulfilled' ? result.value : [],
                            )
                        } else {
                            documents = await resourceProjectDocuments(
                                source,
                                ids,
                                signal,
                                'onlyCached',
                            )
                        }
                    }
                    for (const document of documents)
                        metadata.set(
                            `${source}:${document.metadata.project.projectId}`,
                            document.metadata,
                        )
                } finally {
                    if (!signal?.aborted) onSourceSettled?.(source, metadata)
                }
            }),
    )
    signal?.throwIfAborted()
    return metadata
}

export type FavoriteProvider = 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
export type FavoriteContentType = 'mod' | 'resourcepack' | 'datapack' | 'shader'

export interface ContentFavorite {
    locator?: string | null
    resource_id?: string | null
    projects?: SourceProjectRef[]
    provider: FavoriteProvider
    project_id: string
    content_type: FavoriteContentType
    saved_at: number
}

export interface ContentFavoriteInput {
    slug?: string | null
    provider: FavoriteProvider
    project_id: string
    content_type: FavoriteContentType
}

export const FAVORITE_CONTENT_TYPES: FavoriteContentType[] = [
    'mod',
    'resourcepack',
    'datapack',
    'shader',
]

export function isFavoriteContentType(value: string): value is FavoriteContentType {
    return FAVORITE_CONTENT_TYPES.includes(value as FavoriteContentType)
}

export function contentFavoriteKey(provider: FavoriteProvider, projectId: string) {
    return `${provider}:${projectId}`
}

export async function listContentFavorites(): Promise<ContentFavorite[]> {
    const values = await invoke<
        {
            resourceId: string | null
            locator: string | null
            project: SourceProjectRef
            projects: SourceProjectRef[]
            contentType: ResourceType
            savedAt: number
        }[]
    >('plugin:axolotl-resources|resource_favorites')
    return values.map((value) => ({
        resource_id: value.resourceId,
        locator: value.locator,
        projects: value.projects,
        provider: value.project.source,
        project_id: value.project.projectId,
        content_type: value.contentType.toLowerCase() as FavoriteContentType,
        saved_at: value.savedAt,
    }))
}

export async function addContentFavorite(favorite: ContentFavoriteInput): Promise<ContentFavorite> {
    const types: Record<FavoriteContentType, ResourceType> = {
        mod: 'mod',
        shader: 'shader',
        resourcepack: 'resourcePack',
        datapack: 'dataPack',
    }
    await invoke('plugin:axolotl-resources|resource_favorite_add', {
        project: { source: favorite.provider, projectId: favorite.project_id },
        contentType: types[favorite.content_type],
        locator: favorite.slug ?? null,
    })
    const values = await listContentFavorites()
    const saved = values.find((value) =>
        value.projects?.some(
            (project) =>
                project.source === favorite.provider && project.projectId === favorite.project_id,
        ),
    )
    if (!saved) throw new Error('Saved favorite could not be reloaded')
    return saved
}

export async function removeContentFavorite(
    provider: FavoriteProvider,
    projectId: string,
): Promise<void> {
    await invoke('plugin:axolotl-resources|resource_favorite_remove', {
        project: { source: provider, projectId },
    })
}
