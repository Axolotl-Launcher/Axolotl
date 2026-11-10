import { ref } from 'vue'

import type { LogLevel, LogLine } from '../types'

export type FilterPredicate = (line: LogLine) => boolean

export type ConditionalLevel = 'debug' | 'trace'

/** Console streams append immutable lines; trimming and source changes rebuild the index. */
export function createConsoleLineFilter() {
    let key: string | undefined
    let source: LogLine[] | undefined
    let count = 0
    let first: LogLine | undefined
    let last: LogLine | undefined
    let result: Array<{ line: LogLine; originalIndex: number }> = []
    return (lines: LogLine[], filterKey: string, predicate: FilterPredicate | null) => {
        const append =
            source === lines &&
            key === filterKey &&
            lines.length >= count &&
            lines[0] === first &&
            (count === 0 || lines[count - 1] === last)
        const start = append ? count : 0
        if (!append) result = []
        for (let i = start; i < lines.length; i++) {
            if (!predicate || predicate(lines[i]!))
                result.push({ line: lines[i]!, originalIndex: i })
        }
        source = lines
        key = filterKey
        count = lines.length
        first = lines[0]
        last = lines.at(-1)
        return result.slice()
    }
}

export function useConsoleFilters() {
    const activeFilters = ref<Set<LogLevel>>(new Set(['error', 'warn', 'info']))

    function toggleFilter(level: LogLevel) {
        const next = new Set(activeFilters.value)
        if (next.has(level)) {
            next.delete(level)
        } else {
            next.add(level)
        }
        activeFilters.value = next
    }

    function buildFilterPredicate(): FilterPredicate | null {
        if (activeFilters.value.size === 0) return () => false
        const allowed = activeFilters.value
        return (line: LogLine) => {
            return allowed.has(line.level ?? 'info')
        }
    }

    return { activeFilters, toggleFilter, buildFilterPredicate }
}
