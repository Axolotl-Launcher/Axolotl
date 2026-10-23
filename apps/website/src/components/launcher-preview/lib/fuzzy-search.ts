/**
 * The part of `fuse.js` the app's palettes use (`new Fuse(list, options).search(q)`
 * with `keys`, `threshold` and `ignoreLocation`), for the preview, which cannot
 * add the dependency. A value matches when the query appears in it with at most
 * `threshold × query length` edits (case-insensitive, anywhere in the value, as
 * with `ignoreLocation`). Results are ordered by that score, then by list order,
 * as Fuse's default sort is.
 */
export interface FuzzyOptions {
    keys?: string[]
    threshold?: number
    ignoreLocation?: boolean
}

export interface FuzzyResult<T> {
    item: T
    refIndex: number
    score: number
}

/** Fewest edits that turn `pattern` into some substring of `text`. */
function substringDistance(pattern: string, text: string): number {
    const columns = text.length + 1
    let previous = new Array<number>(columns).fill(0)
    let current = new Array<number>(columns).fill(0)
    for (let row = 1; row <= pattern.length; row++) {
        current[0] = row
        for (let column = 1; column < columns; column++) {
            const cost = pattern[row - 1] === text[column - 1] ? 0 : 1
            current[column] = Math.min(
                previous[column - 1] + cost,
                previous[column] + 1,
                current[column - 1] + 1,
            )
        }
        ;[previous, current] = [current, previous]
    }
    return Math.min(...previous)
}

export default class Fuse<T> {
    constructor(
        private readonly list: readonly T[],
        private readonly options: FuzzyOptions = {},
    ) {}

    private valuesOf(item: T): string[] {
        if (typeof item === 'string') return [item]
        return (this.options.keys ?? []).map((key) => {
            const value = (item as Record<string, unknown>)[key]
            return typeof value === 'string' ? value : ''
        })
    }

    search(query: string): FuzzyResult<T>[] {
        const pattern = query.toLowerCase()
        if (!pattern) return []
        const threshold = this.options.threshold ?? 0.6
        const results: FuzzyResult<T>[] = []
        this.list.forEach((item, refIndex) => {
            let best = Infinity
            for (const value of this.valuesOf(item)) {
                if (!value) continue
                const score = substringDistance(pattern, value.toLowerCase()) / pattern.length
                if (score < best) best = score
            }
            if (best <= threshold) results.push({ item, refIndex, score: best })
        })
        return results.sort((a, b) => a.score - b.score || a.refIndex - b.refIndex)
    }
}
