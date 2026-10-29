import { expect, it } from 'vitest'

import { createLocaleLoader } from './locale-loader.ts'

it('loads only requested languages, shares concurrent loads and reuses loaded messages', async () => {
    const loaded: string[] = []
    const active: string[] = []
    const loader = createLocaleLoader(
        async (locale) => {
            loaded.push(locale)
        },
        (locale) => active.push(locale),
    )
    await Promise.all([loader.ensure('en-US'), loader.ensure('en-US')])
    await loader.select('zh-CN')
    await loader.select('en-US')
    expect(loaded).toEqual(['en-US', 'zh-CN'])
    expect(active).toEqual(['zh-CN', 'en-US'])
})

it('slow locale loads cannot override a later selection', async () => {
    let finish!: () => void
    const active: string[] = []
    const loader = createLocaleLoader(
        (locale) =>
            locale === 'de-DE'
                ? new Promise<void>((resolve) => {
                      finish = resolve
                  })
                : Promise.resolve(),
        (locale) => active.push(locale),
    )
    const slow = loader.select('de-DE')
    await Promise.resolve()
    await loader.select('en-US')
    finish()
    await slow
    expect(active).toEqual(['en-US'])
})

it('failed language loads preserve the active language and can be retried', async () => {
    let attempts = 0
    const active: string[] = []
    const loader = createLocaleLoader(
        async () => {
            if (++attempts === 1) throw new Error('chunk unavailable')
        },
        (locale) => active.push(locale),
    )
    await expect(loader.select('zh-CN')).rejects.toThrow(/chunk unavailable/)
    expect(active).toEqual([])
    await loader.select('zh-CN')
    expect(active).toEqual(['zh-CN'])
})

it('an eager fallback is reused without dispatching a language request', async () => {
    const loaded: string[] = []
    const active: string[] = []
    const loader = createLocaleLoader(
        async (locale) => {
            loaded.push(locale)
        },
        (locale) => active.push(locale),
        ['en-US'],
    )
    await loader.select('en-US')
    await loader.select('zh-CN')
    expect(loaded).toEqual(['zh-CN'])
    expect(active).toEqual(['en-US', 'zh-CN'])
})
