import { LRUCache } from 'lru-cache'

import { injectI18n } from '../providers/i18n'

const formatterCache = new LRUCache<string, Intl.DateTimeFormat>({ max: 40 })

export function useFormatDateTime(options?: Intl.DateTimeFormatOptions) {
    const { locale } = injectI18n()

    function format(date?: Date | number | string | null): string {
        if (date === undefined || date === null) return ''

        const value = typeof date === 'number' || typeof date === 'string' ? new Date(date) : date
        if (Number.isNaN(value.getTime())) return ''

        const formatter = getFormatter(locale.value, options)
        return formatter!.format(value)
    }

    return format
}

function getFormatter(locale: string, options?: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
    let cacheKey = locale
    if (options) {
        const entries = Object.entries(options)
            .filter(([, value]) => value !== undefined)
            .sort()
            .map(([key, value]) => `${key}=${value}`)
        cacheKey = [locale, ...entries].join(':')
    }

    let formatter = formatterCache.get(cacheKey)
    if (!formatter) {
        formatter = new Intl.DateTimeFormat(locale, options)
        formatterCache.set(cacheKey, formatter)
    }
    return formatter
}
