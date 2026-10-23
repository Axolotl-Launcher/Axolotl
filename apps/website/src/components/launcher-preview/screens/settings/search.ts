/**
 * Settings search, as the desktop app runs it
 * (apps/app-frontend/src/components/ui/settings/settings-search.ts): the query is
 * normalised and matched with Fuse (`keys: ['text']`, `threshold: 0.35`,
 * `ignoreLocation: true`), capped at 20 results.
 *
 * The website does not ship Fuse, so the part of Fuse 6.6.2 (Apache-2.0) this
 * search exercises is reproduced here unchanged in behaviour: the Bitap matcher
 * with its 32-bit pattern chunks, the field-length norm and the score product
 * across every matching text, sorted by score and then by document order.
 */

export const MAX_SETTINGS_SEARCH_RESULTS = 20

const THRESHOLD = 0.35
const MAX_BITS = 32

export interface SettingsSearchDocument<T> {
    item: T
    text: string | string[]
}

export function normalizeSettingsSearchText(value: string): string {
    return value.trim().toLocaleLowerCase().replace(/\s+/g, ' ')
}

/* ---- Fuse: Bitap matcher -------------------------------------------------- */

function createPatternAlphabet(pattern: string): Record<string, number> {
    const mask: Record<string, number> = {}
    for (let i = 0, len = pattern.length; i < len; i += 1) {
        const char = pattern.charAt(i)
        mask[char] = (mask[char] || 0) | (1 << (len - i - 1))
    }
    return mask
}

/** `computeScore` with `ignoreLocation: true`: only the error ratio counts. */
function accuracy(pattern: string, errors: number): number {
    return errors / pattern.length
}

function bitap(text: string, pattern: string, alphabet: Record<string, number>, location: number) {
    const patternLen = pattern.length
    const textLen = text.length
    const expectedLocation = Math.max(0, Math.min(location, textLen))
    let currentThreshold = THRESHOLD
    let bestLocation = expectedLocation

    let index: number
    while ((index = text.indexOf(pattern, bestLocation)) > -1) {
        const score = accuracy(pattern, 0)
        currentThreshold = Math.min(score, currentThreshold)
        bestLocation = index + patternLen
    }

    bestLocation = -1
    let lastBitArr: number[] = []
    let finalScore = 1
    let binMax = patternLen + textLen
    const mask = 1 << (patternLen - 1)

    for (let i = 0; i < patternLen; i += 1) {
        let binMin = 0
        let binMid = binMax
        while (binMin < binMid) {
            const score = accuracy(pattern, i)
            if (score <= currentThreshold) binMin = binMid
            else binMax = binMid
            binMid = Math.floor((binMax - binMin) / 2 + binMin)
        }
        binMax = binMid

        let start = Math.max(1, expectedLocation - binMid + 1)
        const finish = Math.min(expectedLocation + binMid, textLen) + patternLen
        const bitArr: number[] = Array(finish + 2)
        bitArr[finish + 1] = (1 << i) - 1

        for (let j = finish; j >= start; j -= 1) {
            const currentLocation = j - 1
            const charMatch = alphabet[text.charAt(currentLocation)]
            bitArr[j] = ((bitArr[j + 1] << 1) | 1) & charMatch
            if (i) {
                bitArr[j] |= ((lastBitArr[j + 1] | lastBitArr[j]) << 1) | 1 | lastBitArr[j + 1]
            }
            if (bitArr[j] & mask) {
                finalScore = accuracy(pattern, i)
                if (finalScore <= currentThreshold) {
                    currentThreshold = finalScore
                    bestLocation = currentLocation
                    if (bestLocation <= expectedLocation) break
                    start = Math.max(1, 2 * expectedLocation - bestLocation)
                }
            }
        }

        if (accuracy(pattern, i + 1) > currentThreshold) break
        lastBitArr = bitArr
    }

    return { isMatch: bestLocation >= 0, score: Math.max(0.001, finalScore) }
}

class BitapSearch {
    private readonly pattern: string
    private readonly chunks: {
        pattern: string
        alphabet: Record<string, number>
        startIndex: number
    }[] = []

    constructor(pattern: string) {
        this.pattern = pattern.toLowerCase()
        if (!this.pattern.length) return
        const len = this.pattern.length
        const addChunk = (chunk: string, startIndex: number) =>
            this.chunks.push({ pattern: chunk, alphabet: createPatternAlphabet(chunk), startIndex })
        if (len > MAX_BITS) {
            let i = 0
            const remainder = len % MAX_BITS
            const end = len - remainder
            while (i < end) {
                addChunk(this.pattern.substr(i, MAX_BITS), i)
                i += MAX_BITS
            }
            if (remainder) addChunk(this.pattern.substr(len - MAX_BITS), len - MAX_BITS)
        } else {
            addChunk(this.pattern, 0)
        }
    }

    searchIn(input: string) {
        const text = input.toLowerCase()
        if (this.pattern === text) return { isMatch: true, score: 0 }
        let totalScore = 0
        let hasMatches = false
        for (const { pattern, alphabet, startIndex } of this.chunks) {
            // Fuse's `location` (0) plus the chunk's offset within the pattern.
            const { isMatch, score } = bitap(text, pattern, alphabet, startIndex)
            if (isMatch) hasMatches = true
            totalScore += score
        }
        return { isMatch: hasMatches, score: hasMatches ? totalScore / this.chunks.length : 1 }
    }
}

/* ---- Fuse: field-length norm and scoring ---------------------------------- */

const normCache = new Map<number, number>()

/** `1 / sqrt(tokens)`, rounded to three decimals, as Fuse's index stores it. */
function fieldNorm(value: string): number {
    const tokens = value.match(/[^ ]+/g)?.length ?? 1
    let cached = normCache.get(tokens)
    if (cached === undefined) {
        cached = Math.round((1 / Math.pow(tokens, 0.5)) * 1000) / 1000
        normCache.set(tokens, cached)
    }
    return cached
}

function isBlank(value: string) {
    return !value.trim().length
}

/** The texts of one document in the order Fuse's index records them. */
function indexRecords(text: string | string[]): { v: string; n: number }[] {
    if (!Array.isArray(text)) return isBlank(text) ? [] : [{ v: text, n: fieldNorm(text) }]
    const records: { v: string; n: number }[] = []
    const stack: unknown[] = [text]
    while (stack.length) {
        const value = stack.pop()
        if (value === undefined || value === null) continue
        if (typeof value === 'string') {
            if (!isBlank(value)) records.push({ v: value, n: fieldNorm(value) })
        } else if (Array.isArray(value)) {
            for (const item of value) stack.push(item)
        }
    }
    return records
}

export function filterSettingsSearchDocuments<T>(
    query: string,
    documents: SettingsSearchDocument<T>[],
): SettingsSearchDocument<T>[] {
    const normalizedQuery = normalizeSettingsSearchText(query)
    if (!normalizedQuery) return []

    const searcher = new BitapSearch(normalizedQuery)
    const results: { idx: number; score: number }[] = []

    documents.forEach((document, idx) => {
        let matched = false
        let totalScore = 1
        for (const { v, n } of indexRecords(document.text)) {
            const { isMatch, score } = searcher.searchIn(v)
            if (!isMatch) continue
            matched = true
            // The single key carries weight 1, so an exact match counts as EPSILON.
            totalScore *= Math.pow(score === 0 ? Number.EPSILON : score, n)
        }
        if (matched) results.push({ idx, score: totalScore })
    })

    results.sort((a, b) =>
        a.score === b.score ? (a.idx < b.idx ? -1 : 1) : a.score < b.score ? -1 : 1,
    )
    return results.slice(0, MAX_SETTINGS_SEARCH_RESULTS).map(({ idx }) => documents[idx])
}
