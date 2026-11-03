import type { ContentItem } from '@modrinth/ui'

import { translateContentItemTitles } from './content-search'

/** A page owns this cache: unrelated entry patches must not repeat dictionary IPC. */
export function createContentTitleEnrichment(translate = translateContentItemTitles<ContentItem>) {
    const titles = new Map<string, string>()
    const pending = new Map<string, Promise<void>>()
    let generation = 0

    function key(item: ContentItem, locale: string) {
        const project = item.project
        if (
            locale !== 'zh-CN' ||
            !project?.slug ||
            !['modrinth', 'curseforge'].includes(item.origin_provider ?? '')
        )
            return undefined
        return JSON.stringify([
            locale,
            item.origin_provider,
            project.id,
            project.slug,
            project.title,
        ])
    }

    function cached(items: ContentItem[], locale: string) {
        return items.map((item) => {
            const input = key(item, locale)
            const title = input === undefined ? undefined : titles.get(input)
            return title === undefined || title === item.project?.title
                ? item
                : { ...item, project: { ...item.project!, title } }
        })
    }

    async function enrich(items: ContentItem[], locale: string) {
        const missing = new Map<string, ContentItem>()
        const waits = new Set<Promise<void>>()
        for (const item of items) {
            const input = key(item, locale)
            if (input === undefined || titles.has(input)) continue
            const inFlight = pending.get(input)
            if (inFlight) waits.add(inFlight)
            else missing.set(input, item)
        }
        if (missing.size) {
            const inputs = [...missing.keys()]
            const current = generation
            const batch = translate([...missing.values()], locale)
                .then((translated) => {
                    if (generation !== current) return
                    translated.forEach((item, index) => {
                        if (item.project) titles.set(inputs[index], item.project.title)
                    })
                })
                .finally(() => {
                    if (generation !== current) return
                    for (const input of inputs) pending.delete(input)
                })
            for (const input of inputs) pending.set(input, batch)
            waits.add(batch)
        }
        await Promise.all(waits)
        return cached(items, locale)
    }

    function clear() {
        generation += 1
        titles.clear()
        pending.clear()
    }

    return { cached, enrich, clear }
}
