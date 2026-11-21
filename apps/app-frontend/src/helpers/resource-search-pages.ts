import type { ResourcePage, ResourceSearchQuery, ResourceSearchResult } from './resources'

/**
 * Caches one backend page per requested index. Every page is read directly
 * through the source's offset/limit API, so a distant page costs a single
 * request and pages can be visited in any order.
 */
export class ResourceSearchPages {
    private key = ''
    private generation = 0
    private pages = new Map<number, ResourcePage<ResourceSearchResult>>()

    private fetchPage: (
        query: ResourceSearchQuery,
        page: number,
        signal: AbortSignal,
    ) => Promise<ResourcePage<ResourceSearchResult>>

    constructor(fetchPage: ResourceSearchPages['fetchPage']) {
        this.fetchPage = fetchPage
    }

    indexOf(page: ResourcePage<ResourceSearchResult>) {
        for (const [index, cached] of this.pages) {
            if (cached === page) return index
        }
        return -1
    }

    invalidate(index = 1) {
        this.generation++
        for (const page of this.pages.keys()) {
            if (page >= index) this.pages.delete(page)
        }
    }

    async get(
        query: ResourceSearchQuery,
        index: number,
        signal: AbortSignal,
    ): Promise<ResourcePage<ResourceSearchResult>> {
        if (!Number.isInteger(index) || index < 1) throw new Error('Invalid search page')
        // The page is supplied by `get`, so it must not take part in the cache
        // key even if a caller happens to carry one on its query object.
        const { page: _page, ...queryKey } = query
        const key = JSON.stringify(queryKey)
        if (key !== this.key) {
            this.key = key
            this.generation++
            this.pages.clear()
        }
        const generation = this.generation
        const cached = this.pages.get(index)
        if (cached && !cached.sourceFailures.some((failure) => failure.retryable)) return cached

        signal.throwIfAborted()
        let result: ResourcePage<ResourceSearchResult>
        try {
            result = await this.fetchPage({ ...query, page: index }, index, signal)
        } catch (error) {
            signal.throwIfAborted()
            if (generation !== this.generation)
                throw new DOMException('Query changed', 'AbortError')
            throw error
        }
        signal.throwIfAborted()
        if (this.generation !== generation) throw new DOMException('Query changed', 'AbortError')
        this.pages.set(index, result)
        return result
    }
}
