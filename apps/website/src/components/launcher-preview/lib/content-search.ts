import { shallowRef } from 'vue'

/*
 * `helpers/content-search.ts` and `content_search.rs` of the app over the MC 百科
 * dictionary it bundles (`WikiEntries.txt`). `data/content-search.json` holds the
 * entries of the projects the preview shows and the segmentation words;
 * `data/content-search-dictionary.json` the Modrinth entries a Chinese query is
 * matched against, loaded on the first such query.
 */

type Snapshot = {
    /** Modrinth project id → slug. */
    slugs: Record<string, string>
    /** Lowercase slug → [MC 百科 class id, Chinese name]. */
    entries: Record<string, [number | null, string | null]>
    /** `searcher_words.txt`, the segmentation words of compact queries. */
    words: string[]
}

type Dictionary = {
    /** [Modrinth slug, Chinese name, popularity] */
    entries: [string, string, number][]
}

let snapshot: Promise<Snapshot> | null = null
let dictionary: Promise<Dictionary> | null = null
const loadedSnapshot = shallowRef<Snapshot | null>(null)

function loadSnapshot() {
    snapshot ??= import('../data/content-search.json').then((module) => {
        loadedSnapshot.value = module.default as Snapshot
        return loadedSnapshot.value
    })
    return snapshot
}

/**
 * The entries of a Modrinth project for computed values: `undefined` until the
 * snapshot has loaded, which this starts.
 */
export function useContentEntries() {
    if (!loadedSnapshot.value) void loadSnapshot()
    return (projectId: string | null | undefined, slug?: string | null) => {
        const data = loadedSnapshot.value
        const key = (slug ?? (projectId ? data?.slugs[projectId] : null))?.toLowerCase()
        const entry = key ? data?.entries[key] : undefined
        return entry ? { wikiId: entry[0], chineseName: entry[1] } : undefined
    }
}

/** The MC 百科 page of a dictionary class id. */
export function mcmodUrl(wikiId: number | null | undefined) {
    return wikiId ? `https://www.mcmod.cn/class/${wikiId}.html` : null
}

function loadDictionary() {
    dictionary ??= import('../data/content-search-dictionary.json').then(
        (module) => module.default as unknown as Dictionary,
    )
    return dictionary
}

export function containsChineseSearchText(query: string): boolean {
    return /[㐀-䶿一-鿿]/u.test(query)
}

/** `contains_chinese`. */
function containsChinese(value: string) {
    return /[一-龻]/u.test(value)
}

async function entryFor(slug: string | null | undefined) {
    if (!slug) return undefined
    return (await loadSnapshot()).entries[slug.toLowerCase()]
}

/** The slug of a Modrinth project the preview shows. */
export async function projectSlug(projectId: string | null | undefined) {
    if (!projectId) return null
    return (await loadSnapshot()).slugs[projectId] ?? null
}

/** `resolveMcmodUrl`: the MC 百科 page of a Modrinth slug, or `null`. */
export async function resolveMcmodUrl(slug: string | null | undefined): Promise<string | null> {
    return mcmodUrl((await entryFor(slug))?.[0])
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

/**
 * `translateSearchHitTitles`: in zh-CN, hits read `中文名 (English)` when the
 * dictionary knows the project's Chinese name.
 */
export async function translateSearchHitTitles<T extends { slug?: string | null; title: string }>(
    hits: T[],
    locale: string,
): Promise<T[]> {
    if (locale !== 'zh-CN' || hits.length === 0) return hits
    const { entries } = await loadSnapshot()
    return hits.map((hit) => {
        const chineseName = hit.slug ? entries[hit.slug.toLowerCase()]?.[1] : null
        return chineseName ? { ...hit, title: bilingualTitle(chineseName, hit.title) } : hit
    })
}

/* ---------------------------------------------------------- file names */

/** `strip_english_alias`: drops the trailing ` (English Name)` of a dictionary name. */
function stripEnglishAlias(name: string) {
    const trimmed = name.trimEnd()
    if (!trimmed.endsWith(')')) return trimmed
    const withoutParen = trimmed.slice(0, -1)
    const open = withoutParen.lastIndexOf('(')
    if (open < 0) return trimmed
    const inner = withoutParen.slice(open + 1)
    if (/[()]/.test(inner) || !/[A-Za-z]/.test(inner)) return trimmed
    const prefix = withoutParen.slice(0, open)
    const stripped = prefix.trimEnd()
    if (!stripped || stripped.length === prefix.length) return trimmed
    return stripped
}

/** `chinese_file_title`: the `[中文名]` a downloaded file is prefixed with. */
function chineseFileTitle(chineseName: string) {
    const sanitized = [...stripEnglishAlias(chineseName)]
        .filter((character) => {
            // `char::is_control`: the C0 and C1 control characters.
            const code = character.codePointAt(0) ?? 0
            const control = code < 0x20 || (code >= 0x7f && code <= 0x9f)
            return !control && !'/\\:*?"<>|[]'.includes(character)
        })
        .join('')
        .trim()
    return sanitized && containsChinese(sanitized) ? sanitized : null
}

/** `localized_content_file_name`: `[中文名]file.jar`, within 200 bytes. */
function localizedContentFileName(fileName: string, chineseTitle: string) {
    if (!fileName || fileName.startsWith('[')) return null
    if (!chineseTitle || !containsChinese(chineseTitle) || /[[\]/\\]/.test(chineseTitle))
        return null
    const localized = `[${chineseTitle}]${fileName}`
    return new TextEncoder().encode(localized).length <= 200 ? localized : null
}

/** `Settings::resource_chinese_filenames` (on by default), as the settings page keeps it. */
export function chineseFileNamesEnabled(domains: Map<string, { value: unknown }>) {
    const settings = domains.get('settings')?.value as
        { extras?: { resource_chinese_filenames?: boolean } } | undefined
    return settings?.extras?.resource_chinese_filenames ?? true
}

/**
 * The name a Modrinth download is saved under (`resource_chinese_filenames`):
 * the project's Chinese name in front when the setting is on and the
 * dictionary has one.
 */
export async function contentFileName(
    fileName: string,
    slug: string | null,
    chineseNames: boolean,
) {
    if (!chineseNames || !slug) return fileName
    const chineseName = (await entryFor(slug))?.[1]
    const title = chineseName ? chineseFileTitle(chineseName) : null
    return (title && localizedContentFileName(fileName, title)) ?? fileName
}

/* -------------------------------------------------------------- search */

const MAX_MATCHES = 100
const MIN_SIMILARITY = 0.25
const MIN_COMPACT_QUERY_LENGTH = 6
const STOP_WORDS = new Set([
    'the',
    'of',
    'mod',
    'and',
    'forge',
    'fabric',
    'for',
    'quilt',
    'neoforge',
])

type Entry = { slug: string; name: string; popularity: number }
type Match = { entry: Entry; similarity: number; exact: boolean }

/** `search_sources`: the Chinese aliases at full weight, name and slug at half. */
function searchSources(entry: Entry): [string[], number][] {
    const primary = entry.name.split(' (')[0]
    return [
        [primary.split('/'), 1],
        [[`${entry.name}${entry.slug}`], 0.5],
    ]
}

/** `search_similarity`: longest common runs, weighted by length and position. */
function searchSimilarity(source: string, query: string) {
    if (!source || !query) return 0
    const sourceChars = [...source.toLowerCase()].filter((character) => character !== ' ')
    const queryChars = [...query.toLowerCase()].filter((character) => character !== ' ')
    const sourceLength = sourceChars.length
    const queryLength = queryChars.length
    if (queryLength === 0) return 0
    let queryPosition = 0
    let lengthSum = 0
    while (queryPosition < queryLength) {
        let sourcePosition = 0
        let maxLength = 0
        let maxPosition = 0
        while (sourcePosition < sourceChars.length) {
            let length = 0
            while (
                queryPosition + length < queryLength &&
                sourcePosition + length < sourceChars.length &&
                sourceChars[sourcePosition + length] === queryChars[queryPosition + length]
            ) {
                length += 1
            }
            if (length > maxLength) {
                maxLength = length
                maxPosition = sourcePosition
            }
            sourcePosition += length > 0 ? length : 1
        }
        if (maxLength > 0) {
            sourceChars.splice(maxPosition, maxLength)
            let weight = 1.4 ** (3 + maxLength) - 3.6
            const distance = Math.abs(queryPosition - maxPosition)
            weight *= 1 + 0.3 * Math.max(0, 3 - distance)
            lengthSum += weight
        }
        queryPosition += maxLength > 0 ? maxLength : 1
    }
    return (
        (lengthSum / queryLength) *
        (3 / Math.sqrt(sourceLength + 15)) *
        (queryLength <= 2 ? 3 - queryLength : 1)
    )
}

function weightedSimilarity(sources: [string[], number][], query: string) {
    const total = sources.reduce((sum, [, weight]) => sum + weight, 0)
    if (total === 0) return 0
    return (
        sources.reduce(
            (sum, [aliases, weight]) =>
                sum +
                Math.max(0, ...aliases.map((alias) => searchSimilarity(alias, query))) * weight,
            0,
        ) / total
    )
}

/** `search_entries` for the Modrinth platform. */
function searchEntries(entries: Entry[], query: string): Match[] {
    const queryParts = query.split(/\s+/).filter(Boolean)
    const candidates: Match[] = []
    for (const entry of entries) {
        const sources = searchSources(entry)
        const similarity = weightedSimilarity(sources, query)
        const exact = queryParts.every((part) =>
            sources.some(([aliases]) =>
                aliases.some((alias) =>
                    alias.replaceAll(' ', '').toLowerCase().includes(part.toLowerCase()),
                ),
            ),
        )
        if (exact || similarity >= MIN_SIMILARITY) candidates.push({ entry, similarity, exact })
    }
    candidates.sort(
        (left, right) =>
            Number(right.exact) - Number(left.exact) ||
            right.similarity - left.similarity ||
            right.entry.popularity - left.entry.popularity ||
            (left.entry.slug < right.entry.slug ? -1 : left.entry.slug > right.entry.slug ? 1 : 0),
    )
    let fuzzy = 0
    return candidates.filter((candidate) => candidate.exact || fuzzy++ < MAX_MATCHES)
}

function canFormFromWords(value: string, words: string[]): boolean {
    return (
        !value ||
        words.some(
            (word) =>
                value === word ||
                (value.startsWith(word) && canFormFromWords(value.slice(word.length), words)),
        )
    )
}

/** `extract_words`: the English words of an entry's slug and name. */
function extractWords(entry: Entry) {
    const parenthetical = entry.name.includes(' (')
        ? entry.name.slice(entry.name.lastIndexOf(' (') + 2).replace(/[) ]+$/, '')
        : entry.name.replace(/[) ]+$/, '')
    const candidates = [
        entry.slug.replace(/[-/]/g, ' '),
        parenthetical.split(' - ')[0].replace(/[-/:()]/g, ' '),
    ]
    const seen = new Set<string>()
    const words = candidates
        .flatMap((candidate) => candidate.split(/\s+/))
        .map((word) =>
            word
                .replace(/^[{[(]+/, '')
                .replace(/[}\])]+$/, '')
                .toLowerCase(),
        )
        .filter(
            (word) =>
                word.length > 1 &&
                !STOP_WORDS.has(word) &&
                !/^\d+$/.test(word) &&
                // eslint-disable-next-line no-control-regex
                /^[\x00-\x7f]+$/.test(word),
        )
        .filter((word) => !seen.has(word) && seen.add(word))
    return words.filter(
        (word) =>
            !words.some(
                (prefix) =>
                    prefix.length < word.length &&
                    word.startsWith(prefix) &&
                    canFormFromWords(word.slice(prefix.length), words),
            ),
    )
}

/** `select_modrinth_query`: the English word the best matches share most. */
function selectModrinthQuery(matches: Match[], query: string) {
    const weights = new Map<string, number>()
    for (const match of matches) {
        const exactAlias = searchSources(match.entry).some(([aliases]) =>
            aliases.some((alias) => alias.toLowerCase() === query.toLowerCase()),
        )
        const similarity = exactAlias ? 1000 : match.similarity
        for (const word of extractWords(match.entry)) {
            weights.set(word, (weights.get(word) ?? 0) + similarity * match.entry.popularity)
        }
    }
    let best: [string, number] | null = null
    for (const [word, weight] of weights) {
        if (!best || weight > best[1] || (weight === best[1] && word < best[0]))
            best = [word, weight]
    }
    return best?.[0] ?? null
}

/** `segment_compact_query`: `sodiumextra` → `sodium extra` with the bundled words. */
function segmentCompactQuery(compact: string, words: string[]) {
    if (compact.length < MIN_COMPACT_QUERY_LENGTH || words.includes(compact)) return null
    const parts: string[] = []
    let rest = compact
    while (rest) {
        const word = words.find((candidate) => rest.startsWith(candidate))
        if (!word) return null
        parts.push(word)
        rest = rest.slice(word.length)
    }
    if (parts.length < 2 || parts.some((part) => part.length < 2)) return null
    return parts.join(' ')
}

let sortedWords: string[] | null = null

/**
 * The text the Modrinth source is asked for (`ResourceService::search`): a
 * Chinese query becomes the English word of the dictionary entries it matches
 * (`resolve_chinese_content_search`), a compact one is split into known words
 * (`expand_content_search_query`). The app also folds Traditional characters
 * to Simplified first; the dictionary's names are Simplified.
 */
export async function modrinthSearchText(query: string): Promise<string> {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return query
    if (containsChinese(normalized)) {
        const { entries } = await loadDictionary()
        const matches = searchEntries(
            entries.map(([slug, name, popularity]) => ({ slug, name, popularity })),
            normalized,
        )
        return selectModrinthQuery(matches, normalized) ?? query
    }
    const compact = [...normalized].filter((character) => /[\p{L}\p{N}]/u.test(character)).join('')
    if (compact.length < MIN_COMPACT_QUERY_LENGTH) return query
    const { words } = await loadSnapshot()
    sortedWords ??= [...words].sort((a, b) => b.length - a.length || (a < b ? -1 : 1))
    return segmentCompactQuery(compact, sortedWords) ?? query
}
