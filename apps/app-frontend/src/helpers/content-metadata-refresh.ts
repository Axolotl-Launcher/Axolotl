import type { ContentItem } from '@modrinth/ui'

export interface ContentMetadataDrainOptions {
    paths: string[]
    chunkSize?: number
    fetchChunk: (paths: string[]) => Promise<ContentItem[]>
    translate: (items: ContentItem[]) => Promise<ContentItem[]>
    /**
     * The items as the list holds them right now, for the paths in this chunk.
     * `merge` compares the refreshed item against these, so they must be the
     * list's own objects rather than whatever `fetchChunk` returned.
     */
    originalsForChunk: (paths: string[]) => ContentItem[]
    merge: (items: ContentItem[], originals: Map<string, ContentItem>) => void
    isCurrent: () => boolean
}

const DEFAULT_CHUNK_SIZE = 64

/**
 * Resolves metadata for every supplied path in bounded, sequential chunks.
 *
 * Why this is a batch and not a per-visible-row refresh: the list is sorted on
 * `project.title`, and that field is rewritten asynchronously as metadata
 * arrives. Refreshing only what is on screen therefore makes the sort key a
 * function of the scroll position -- each completed chunk changes the order,
 * the virtual list recomputes its visible window, that window queues more
 * paths, and the list never settles. Completing the whole set in one pass
 * removes the feedback loop: the order can change once, then stays put.
 *
 * `isCurrent` is checked after every await, so a superseded drain stops
 * instead of writing metadata into a list that has moved on. The check after
 * `fetchChunk` is also what keeps a superseded drain from opening the next
 * chunk. `originalsForChunk` supplies the list's own items for the chunk,
 * which is what `merge` has to compare against.
 */
export async function drainContentMetadata(options: ContentMetadataDrainOptions): Promise<void> {
    const { paths, fetchChunk, translate, merge, isCurrent } = options
    const chunkSize = options.chunkSize ?? DEFAULT_CHUNK_SIZE
    if (chunkSize <= 0 || paths.length === 0) return

    for (let start = 0; start < paths.length; start += chunkSize) {
        const chunk = paths.slice(start, start + chunkSize)
        const originals = new Map(
            options.originalsForChunk(chunk).map((item) => [item.file_path, item]),
        )

        const items = await fetchChunk(chunk)
        if (!isCurrent()) return

        const translated = await translate(items)
        if (!isCurrent()) return

        merge(translated, originals)
        if (!isCurrent()) return
    }
}
