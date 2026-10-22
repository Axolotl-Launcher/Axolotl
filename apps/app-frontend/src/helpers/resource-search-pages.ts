import type { ResourcePage, ResourceSearchQuery, ResourceSearchResult } from './resources'

/** Cursor pages retain the complete backend page; local filters never consume its remainder. */
export class ResourceSearchPages {
    private key = ''
    private generation = 0
    private pages: ResourcePage<ResourceSearchResult>[] = []

    private fetchPage: (
        query: ResourceSearchQuery,
        cursor: string | null,
        signal: AbortSignal,
    ) => Promise<ResourcePage<ResourceSearchResult>>

    constructor(fetchPage: ResourceSearchPages['fetchPage']) {
        this.fetchPage = fetchPage
    }

    indexOf(page: ResourcePage<ResourceSearchResult>) {
        return this.pages.indexOf(page)
    }

    invalidate(index = 0) {
        this.generation++
        this.pages.splice(index)
    }

    async get(
        query: ResourceSearchQuery,
        index: number,
        signal: AbortSignal,
        canRestart = true,
    ): Promise<ResourcePage<ResourceSearchResult>> {
        if (!Number.isInteger(index) || index < 0) throw new Error('Invalid search page')
        const key = JSON.stringify(query)
        if (key !== this.key) {
            this.key = key
            this.generation++
            this.pages = []
        }
        const generation = this.generation
        if (this.pages[index]?.sourceFailures.some((failure) => failure.retryable))
            this.pages.splice(index)
        for (let page = this.pages.length; page <= index; page++) {
            signal.throwIfAborted()
            const previous = this.pages[page - 1]
            if (page > 0 && previous && !previous.hasMore) return previous
            let result: ResourcePage<ResourceSearchResult>
            try {
                result = await this.fetchPage(query, previous?.cursor ?? null, signal)
            } catch (error) {
                signal.throwIfAborted()
                if (generation !== this.generation)
                    throw new DOMException('Query changed', 'AbortError')
                if (
                    canRestart &&
                    typeof error === 'object' &&
                    error !== null &&
                    'code' in error &&
                    error.code === 'cursorExpired'
                ) {
                    this.pages = []
                    this.generation++
                    return this.get(query, index, signal, false)
                }
                throw error
            }
            signal.throwIfAborted()
            if (this.generation !== generation)
                throw new DOMException('Query changed', 'AbortError')
            this.pages[page] = result
        }
        return this.pages[index]
    }
}
