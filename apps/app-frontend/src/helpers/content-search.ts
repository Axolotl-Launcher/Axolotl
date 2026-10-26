import type { ContentItem } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'

import { get as getSettings } from '@/helpers/settings.ts'

export interface ChineseSearchTranslation {
    chineseName: string
    curseforgeSlug?: string
    modrinthSlug?: string
    matchScore: number
    exact: boolean
}

export interface ChineseSearchResolution {
    isChinese: boolean
    normalizedQuery: string
    curseforgeQuery?: string
    modrinthQuery?: string
    modrinthSlugs: string[]
    translations: ChineseSearchTranslation[]
}

export interface ContentSearchExpansion {
    suggestedSplit?: string | null
}

export interface ChineseNameLookup {
    modrinth: Record<string, string>
    curseforge: Record<string, string>
}

export interface WikiIdLookup {
    modrinth: Record<string, number>
    curseforge: Record<string, number>
}

export function containsChineseSearchText(query: string): boolean {
    return /[\u3400-\u4dbf\u4e00-\u9fff]/u.test(query)
}

export function resolveChineseContentSearch(query: string) {
    return invoke<ChineseSearchResolution>('plugin:content-search|resolve_chinese_content_search', {
        query,
    })
}

/**
 * Resolves the dictionary-based segmentation of a compact (separator-free)
 * search query, e.g. `sodiumextra` → `sodium extra`. Returns `null` when the
 * bundled dictionary cannot safely split the query.
 */
export function expandContentSearchQuery(query: string) {
    return invoke<ContentSearchExpansion>('plugin:content-search|expand_content_search_query', {
        query,
    })
}

export function lookupChineseContentNames(modrinthSlugs: string[], curseforgeSlugs: string[]) {
    return invoke<ChineseNameLookup>('plugin:content-search|lookup_chinese_content_names', {
        modrinthSlugs,
        curseforgeSlugs,
    })
}

export function lookupContentWikiIds(modrinthSlugs: string[], curseforgeSlugs: string[]) {
    return invoke<WikiIdLookup>('plugin:content-search|lookup_content_wiki_ids', {
        modrinthSlugs,
        curseforgeSlugs,
    })
}

/**
 * Resolves the MC 百科 (mcmod.cn) page URL for a project slug, or `null`
 * when the bundled wiki dictionary has no entry for it.
 */
export async function resolveMcmodUrl(
    slug: string | null | undefined,
    provider: 'modrinth' | 'curseforge',
): Promise<string | null> {
    if (!slug) return null
    const lookup = await lookupContentWikiIds(
        provider === 'modrinth' ? [slug] : [],
        provider === 'curseforge' ? [slug] : [],
    ).catch(() => null)
    const wikiId = provider === 'curseforge' ? lookup?.curseforge[slug] : lookup?.modrinth[slug]
    return wikiId ? `https://www.mcmod.cn/class/${wikiId}.html` : null
}

export function bilingualTitle(chineseName: string, originalTitle: string) {
    const chineseTitle = chineseName.replace(/\s+\([^()]*[A-Za-z][^()]*\)$/u, '').trim()
    if (
        !chineseTitle ||
        chineseTitle.toLocaleLowerCase() === originalTitle.toLocaleLowerCase() ||
        originalTitle.startsWith(`${chineseTitle} (`)
    ) {
        return originalTitle
    }
    return `${chineseTitle} (${originalTitle})`
}

/** Whether content lists should keep dictionary names out of the displayed title. */
async function hidesChineseContentTitles(): Promise<boolean> {
    return (await getSettings()).hide_chinese_content_titles
}

/**
 * Rewrites search hit titles to the bilingual `中文名 (English)` format,
 * resolving names from the bundled wiki dictionary by project slug. Hits are
 * returned unchanged unless the locale is zh-CN; hits already carrying a
 * bilingual title (e.g. from the Chinese search flow) are left untouched.
 */
export async function translateSearchHitTitles<
    T extends { slug?: string | null; title: string; provider: 'modrinth' | 'curseforge' },
>(hits: T[], locale: string): Promise<T[]> {
    if (locale !== 'zh-CN' || hits.length === 0) return hits

    const modrinthSlugs: string[] = []
    const curseforgeSlugs: string[] = []
    for (const hit of hits) {
        if (!hit.slug) continue
        if (hit.provider === 'curseforge') curseforgeSlugs.push(hit.slug)
        else if (hit.provider === 'modrinth') modrinthSlugs.push(hit.slug)
    }
    if (modrinthSlugs.length === 0 && curseforgeSlugs.length === 0) return hits

    const lookup = await lookupChineseContentNames(modrinthSlugs, curseforgeSlugs).catch(() => null)
    if (!lookup) return hits
    if (await hidesChineseContentTitles()) return hits

    return hits.map((hit) => {
        if (!hit.slug) return hit
        const chineseName =
            hit.provider === 'curseforge'
                ? lookup.curseforge[hit.slug]
                : hit.provider === 'modrinth'
                  ? lookup.modrinth[hit.slug]
                  : undefined
        if (!chineseName) return hit
        return { ...hit, title: bilingualTitle(chineseName, hit.title) }
    })
}

/**
 * Attaches the bundled wiki dictionary name to each content item and, in
 * zh-CN with the preference enabled, rewrites the title to the bilingual
 * `中文名 (English)` format used by the Browse page. The name is attached in
 * every locale so the local content search matches Chinese queries regardless
 * of the interface language.
 */
export async function translateContentItemTitles<T extends ContentItem>(
    items: T[],
    locale: string,
): Promise<T[]> {
    if (items.length === 0) return items

    const modrinthSlugs: string[] = []
    const curseforgeSlugs: string[] = []
    for (const item of items) {
        const slug = item.project?.slug
        if (!slug) continue
        if (item.origin_provider === 'curseforge') curseforgeSlugs.push(slug)
        else if (item.origin_provider === 'modrinth') modrinthSlugs.push(slug)
    }
    if (modrinthSlugs.length === 0 && curseforgeSlugs.length === 0) return items

    const lookup = await lookupChineseContentNames(modrinthSlugs, curseforgeSlugs).catch(() => null)
    if (!lookup) return items

    // Read the preference after the lookup so a change made while it was pending
    // still applies to the result.
    const localizeTitles = locale === 'zh-CN' && !(await hidesChineseContentTitles())

    return items.map((item) => {
        const project = item.project
        if (!project?.slug) return item
        const chineseName =
            item.origin_provider === 'curseforge'
                ? lookup.curseforge[project.slug]
                : item.origin_provider === 'modrinth'
                  ? lookup.modrinth[project.slug]
                  : undefined
        if (!chineseName) return item
        const titled = localizeTitles
            ? {
                  ...item,
                  project: {
                      ...project,
                      title: bilingualTitle(chineseName, project.title),
                  },
              }
            : item
        return { ...titled, chinese_title: chineseName }
    })
}
