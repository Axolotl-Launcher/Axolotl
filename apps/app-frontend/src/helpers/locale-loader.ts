/** Deduplicates locale loads and prevents a slow selection from replacing a newer one. */
export function createLocaleLoader(
    load: (locale: string) => Promise<void>,
    activate: (locale: string) => void,
    initialLocales: readonly string[] = [],
) {
    const loaded = new Set(initialLocales)
    const pending = new Map<string, Promise<void>>()
    let selection = 0

    function ensure(locale: string): Promise<void> {
        if (loaded.has(locale)) return Promise.resolve()
        let request = pending.get(locale)
        if (!request) {
            request = Promise.resolve()
                .then(() => load(locale))
                .then(() => {
                    loaded.add(locale)
                })
                .finally(() => pending.delete(locale))
            pending.set(locale, request)
        }
        return request
    }

    return {
        ensure,
        async select(locale: string) {
            const request = ++selection
            await ensure(locale)
            if (request === selection) activate(locale)
        },
    }
}
