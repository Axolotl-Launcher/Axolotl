import type { ContentItem } from '@modrinth/ui'
import { expect, it, vi } from 'vitest'

import { createContentTitleEnrichment } from './content-title-enrichment'

vi.mock('./content-search', () => ({ translateContentItemTitles: vi.fn() }))

function item(id: string, title = 'Original'): ContentItem {
    return {
        id,
        file_name: `${id}.jar`,
        project_type: 'mod',
        enabled: true,
        origin_provider: 'modrinth',
        project: { id: 'project', slug: 'slug', title },
    }
}

it('deduplicates pending title inputs, preserves titles on patches, and rechecks changed inputs', async () => {
    let complete!: () => void
    const translate = vi.fn(
        (items: ContentItem[]) =>
            new Promise<ContentItem[]>((resolve) => {
                complete = () =>
                    resolve(
                        items.map((item) => ({
                            ...item,
                            project: { ...item.project!, title: `中文 (${item.project!.title})` },
                        })),
                    )
            }),
    )
    const enrichment = createContentTitleEnrichment(translate)
    const first = enrichment.enrich([item('one'), item('same-project')], 'zh-CN')
    const patch = enrichment.enrich([item('one')], 'zh-CN')
    expect(translate).toHaveBeenCalledTimes(1)
    expect(translate.mock.calls[0][0]).toHaveLength(1)
    complete()
    await Promise.all([first, patch])
    expect(enrichment.cached([item('one')], 'zh-CN')[0].project?.title).toBe('中文 (Original)')
    await enrichment.enrich([item('one')], 'zh-CN')
    expect(translate).toHaveBeenCalledTimes(1)
    expect(enrichment.cached([item('one')], 'en-US')[0].project?.title).toBe('Original')
    const changed = enrichment.enrich([item('one', 'Changed')], 'zh-CN')
    expect(translate).toHaveBeenCalledTimes(2)
    complete()
    await changed
    expect(enrichment.cached([item('one', 'Changed')], 'zh-CN')[0].project?.title).toBe(
        '中文 (Changed)',
    )
})

it('does not restore cleared titles from an earlier pending lookup', async () => {
    let complete!: (items: ContentItem[]) => void
    const enrichment = createContentTitleEnrichment(
        () =>
            new Promise((resolve) => {
                complete = resolve
            }),
    )
    const pending = enrichment.enrich([item('one')], 'zh-CN')
    enrichment.clear()
    complete([{ ...item('one'), project: { ...item('one').project!, title: '旧翻译' } }])
    await pending
    expect(enrichment.cached([item('one')], 'zh-CN')[0].project?.title).toBe('Original')
})
