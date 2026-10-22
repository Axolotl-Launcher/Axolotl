import { computed, ref } from 'vue'

import {
    addContentFavorite,
    type ContentFavorite,
    type ContentFavoriteInput,
    contentFavoriteKey,
    type FavoriteProvider,
    listContentFavorites,
    removeContentFavorite,
} from '../helpers/content-favorites.ts'
import type { SourceProjectMetadata } from '../helpers/resources.ts'

export interface ContentFavoritesApi {
    list: () => Promise<ContentFavorite[]>
    add: (input: ContentFavoriteInput) => Promise<ContentFavorite>
    remove: (provider: FavoriteProvider, projectId: string) => Promise<void>
}

function sortFavorites(values: ContentFavorite[]) {
    return [...values].sort(
        (left, right) =>
            right.saved_at - left.saved_at ||
            left.provider.localeCompare(right.provider) ||
            left.project_id.localeCompare(right.project_id),
    )
}

function sameFavorites(left: ContentFavorite[], right: ContentFavorite[]) {
    return (
        left.length === right.length &&
        left.every(
            (favorite, index) =>
                favorite.provider === right[index]?.provider &&
                favorite.project_id === right[index]?.project_id &&
                favorite.content_type === right[index]?.content_type &&
                favorite.saved_at === right[index]?.saved_at &&
                favorite.resource_id === right[index]?.resource_id &&
                favorite.locator === right[index]?.locator &&
                JSON.stringify(favorite.projects) === JSON.stringify(right[index]?.projects),
        )
    )
}

export function createContentFavoritesStore(api: ContentFavoritesApi) {
    const favorites = ref<ContentFavorite[]>([])
    const loaded = ref(false)
    const loading = ref(false)
    const pendingKeys = ref(new Set<string>())
    const metadata = new Map<string, { identity: string; value: SourceProjectMetadata }>()
    let loadPromise: Promise<ContentFavorite[]> | null = null

    function metadataIdentity(favorite: ContentFavorite) {
        return JSON.stringify([favorite.resource_id, favorite.locator, favorite.projects])
    }

    function getMetadata(favorite: ContentFavorite) {
        const entry = metadata.get(contentFavoriteKey(favorite.provider, favorite.project_id))
        return entry?.identity === metadataIdentity(favorite) ? entry.value : undefined
    }

    function setMetadata(favorite: ContentFavorite, value: SourceProjectMetadata) {
        const key = contentFavoriteKey(favorite.provider, favorite.project_id)
        const current = favorites.value.find(
            (item) => contentFavoriteKey(item.provider, item.project_id) === key,
        )
        if (!current || metadataIdentity(current) !== metadataIdentity(favorite)) return
        metadata.set(key, { identity: metadataIdentity(favorite), value })
    }

    function setFavorites(values: ContentFavorite[]) {
        const sorted = sortFavorites(values)
        if (!sameFavorites(favorites.value, sorted)) favorites.value = sorted
        const keys = new Set(
            sorted.map((item) => contentFavoriteKey(item.provider, item.project_id)),
        )
        for (const key of metadata.keys()) {
            if (!keys.has(key)) metadata.delete(key)
        }
        return favorites.value
    }

    function setPending(key: string, pending: boolean) {
        const next = new Set(pendingKeys.value)
        if (pending) next.add(key)
        else next.delete(key)
        pendingKeys.value = next
    }

    async function load(force = false): Promise<ContentFavorite[]> {
        if (loaded.value && !force) return favorites.value
        if (loadPromise) return await loadPromise

        loading.value = true
        loadPromise = api
            .list()
            .then((values) => {
                setFavorites(values)
                loaded.value = true
                return favorites.value
            })
            .finally(() => {
                loading.value = false
                loadPromise = null
            })

        return await loadPromise
    }

    const favoriteKeys = computed(
        () =>
            new Set(
                favorites.value.flatMap((favorite) => [
                    contentFavoriteKey(favorite.provider, favorite.project_id),
                    ...(favorite.projects ?? []).map((project) =>
                        contentFavoriteKey(project.source, project.projectId),
                    ),
                ]),
            ),
    )

    function isFavorite(provider: FavoriteProvider, projectId: string) {
        return favoriteKeys.value.has(contentFavoriteKey(provider, projectId))
    }

    function isPending(provider: FavoriteProvider, projectId: string) {
        return pendingKeys.value.has(contentFavoriteKey(provider, projectId))
    }

    async function add(input: ContentFavoriteInput) {
        const key = contentFavoriteKey(input.provider, input.project_id)
        if (pendingKeys.value.has(key)) return
        setPending(key, true)
        try {
            const saved = await api.add(input)
            setFavorites([
                ...favorites.value.filter(
                    (favorite) =>
                        contentFavoriteKey(favorite.provider, favorite.project_id) !== key &&
                        (!saved.resource_id || favorite.resource_id !== saved.resource_id),
                ),
                saved,
            ])
            loaded.value = true
        } finally {
            setPending(key, false)
        }
    }

    async function remove(provider: FavoriteProvider, projectId: string) {
        const key = contentFavoriteKey(provider, projectId)
        if (pendingKeys.value.has(key)) return
        setPending(key, true)
        try {
            await api.remove(provider, projectId)
            setFavorites(
                favorites.value.filter(
                    (favorite) =>
                        contentFavoriteKey(favorite.provider, favorite.project_id) !== key &&
                        !favorite.projects?.some(
                            (project) =>
                                project.source === provider && project.projectId === projectId,
                        ),
                ),
            )
            loaded.value = true
        } finally {
            setPending(key, false)
        }
    }

    async function toggle(input: ContentFavoriteInput) {
        if (isFavorite(input.provider, input.project_id)) {
            await remove(input.provider, input.project_id)
        } else {
            await add(input)
        }
    }

    return {
        favorites,
        loaded,
        loading,
        load,
        getMetadata,
        setMetadata,
        isFavorite,
        isPending,
        add,
        remove,
        toggle,
    }
}

const sharedStore = createContentFavoritesStore({
    list: listContentFavorites,
    add: addContentFavorite,
    remove: removeContentFavorite,
})

export function useContentFavorites() {
    return sharedStore
}
