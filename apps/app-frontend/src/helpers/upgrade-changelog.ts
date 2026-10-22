import { LRUCache } from 'lru-cache'

const translationCache = new LRUCache<string, string>({
    max: 200,
    maxSize: 4 * 1024 * 1024,
    sizeCalculation: (value, key) => Math.max(1, (value.length + key.length) * 2),
})

export interface UpgradeChangelogPopoverOwnership {
    triggerHovered: boolean
    triggerFocused: boolean
    popupHovered: boolean
    popupFocused: boolean
}

export function shouldUpgradeChangelogStayOpen(state: UpgradeChangelogPopoverOwnership): boolean {
    return state.triggerHovered || state.triggerFocused || state.popupHovered || state.popupFocused
}

export function upgradeChangelogTranslationCacheKey(
    provider: string,
    projectId: string,
    releaseId: string,
    targetLanguage: string,
): string {
    return `${provider}:${projectId}:${releaseId}:${targetLanguage}`
}

export function getUpgradeChangelogTranslation(key: string): string | undefined {
    return translationCache.get(key)
}

export function setUpgradeChangelogTranslation(key: string, value: string): void {
    translationCache.set(key, value)
}

export function upgradeExternalChangelogUrl(href: string): string | null {
    try {
        const url = new URL(href.replace(/[),.;!?]+$/u, ''))
        return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null
    } catch {
        return null
    }
}
