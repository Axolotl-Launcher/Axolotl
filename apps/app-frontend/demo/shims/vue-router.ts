import { createMemoryHistory } from 'vue-router-original'

export * from 'vue-router-original'

/**
 * Inside the website's frame, navigating the launcher must not add entries to
 * the page's history (or change its URL), so web history becomes memory history.
 */
export function createWebHistory(base?: string) {
    return createMemoryHistory(base)
}
