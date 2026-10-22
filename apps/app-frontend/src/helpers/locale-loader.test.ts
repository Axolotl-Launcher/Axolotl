import assert from 'node:assert/strict'
import test from 'node:test'

import { createLocaleLoader } from './locale-loader.ts'

test('loads only requested languages, shares concurrent loads and reuses loaded messages', async () => {
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
    assert.deepEqual(loaded, ['en-US', 'zh-CN'])
    assert.deepEqual(active, ['zh-CN', 'en-US'])
})

test('slow locale loads cannot override a later selection', async () => {
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
    assert.deepEqual(active, ['en-US'])
})

test('failed language loads preserve the active language and can be retried', async () => {
    let attempts = 0
    const active: string[] = []
    const loader = createLocaleLoader(
        async () => {
            if (++attempts === 1) throw new Error('chunk unavailable')
        },
        (locale) => active.push(locale),
    )
    await assert.rejects(loader.select('zh-CN'), /chunk unavailable/)
    assert.deepEqual(active, [])
    await loader.select('zh-CN')
    assert.deepEqual(active, ['zh-CN'])
})

test('an eager fallback is reused without dispatching a language request', async () => {
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
    assert.deepEqual(loaded, ['zh-CN'])
    assert.deepEqual(active, ['en-US', 'zh-CN'])
})
