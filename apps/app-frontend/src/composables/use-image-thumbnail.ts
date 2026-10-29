import { invoke } from '@tauri-apps/api/core'
import { type MaybeRefOrGetter, ref, toValue, watch } from 'vue'

import { createThumbnailQueue } from './thumbnail-queue'

export type ImageThumbnailSource = {
    instanceId: string
    filePath: string
    path: string
}

type ThumbnailCacheEntry = {
    promise: Promise<string>
    cancelQueued: () => boolean
    url?: string
    users: number
    lastUsed: number
}

const MAX_CONCURRENT_THUMBNAILS = 4
const MAX_CACHED_THUMBNAILS = 256

const thumbnailCache = new Map<string, ThumbnailCacheEntry>()
const thumbnailQueue = createThumbnailQueue(MAX_CONCURRENT_THUMBNAILS)

function removeThumbnail(cacheKey: string, entry: ThumbnailCacheEntry) {
    if (thumbnailCache.get(cacheKey) !== entry) return
    thumbnailCache.delete(cacheKey)
    entry.cancelQueued()
    if (entry.url) {
        URL.revokeObjectURL(entry.url)
    } else {
        void entry.promise.then(
            (url) => URL.revokeObjectURL(url),
            () => undefined,
        )
    }
}

function trimThumbnailCache() {
    if (thumbnailCache.size <= MAX_CACHED_THUMBNAILS) return

    const unused = [...thumbnailCache.entries()]
        .filter(([, entry]) => entry.users === 0)
        .sort(([, left], [, right]) => left.lastUsed - right.lastUsed)
    for (const [cacheKey, entry] of unused) {
        if (thumbnailCache.size <= MAX_CACHED_THUMBNAILS) break
        removeThumbnail(cacheKey, entry)
    }
}

function acquireThumbnail(source: ImageThumbnailSource, maxDimension: number, revision: unknown) {
    const cacheKey = JSON.stringify([
        source.instanceId,
        source.filePath,
        maxDimension,
        revision ?? null,
    ])
    let entry = thumbnailCache.get(cacheKey)
    if (!entry) {
        const request = thumbnailQueue.enqueue(async () => {
            const bytes = await invoke<ArrayBuffer>('plugin:files|screenshot_thumbnail', {
                instanceId: source.instanceId,
                filePath: source.filePath,
                maxDimension,
            })
            return URL.createObjectURL(new Blob([bytes]))
        })
        entry = {
            users: 0,
            lastUsed: performance.now(),
            promise: request.promise,
            cancelQueued: request.cancel,
        }
        thumbnailCache.set(cacheKey, entry)
        void entry.promise.then(
            (url) => {
                entry!.url = url
                trimThumbnailCache()
            },
            () => {
                if (thumbnailCache.get(cacheKey) === entry) thumbnailCache.delete(cacheKey)
            },
        )
    }

    entry.users += 1
    entry.lastUsed = performance.now()
    return {
        promise: entry.promise,
        release: () => {
            entry!.users = Math.max(0, entry!.users - 1)
            entry!.lastUsed = performance.now()
            if (entry!.users === 0 && !entry!.url && entry!.cancelQueued()) {
                removeThumbnail(cacheKey, entry!)
            }
            trimThumbnailCache()
        },
    }
}

export function useImageThumbnail(
    source: MaybeRefOrGetter<ImageThumbnailSource | null | undefined>,
    maxDimension: MaybeRefOrGetter<number> = 512,
    revision: MaybeRefOrGetter<string | number | undefined> = undefined,
) {
    const thumbnail = ref('')

    watch(
        [() => toValue(source), () => toValue(maxDimension), () => toValue(revision)],
        ([currentSource, currentMaxDimension, currentRevision], _, onCleanup) => {
            thumbnail.value = ''
            if (!currentSource) return

            let active = true
            const acquired = acquireThumbnail(
                currentSource,
                Math.max(1, Math.round(currentMaxDimension)),
                currentRevision,
            )
            onCleanup(() => {
                active = false
                acquired.release()
            })

            void acquired.promise.then(
                (url) => {
                    if (active) thumbnail.value = url
                },
                (error) => {
                    if (!active) return
                    console.warn('Could not create image thumbnail', error)
                },
            )
        },
        { immediate: true },
    )

    return thumbnail
}
