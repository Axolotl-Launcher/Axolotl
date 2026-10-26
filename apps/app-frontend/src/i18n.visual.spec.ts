import { expect, it, vi } from 'vitest'

import i18n, {
    applyLocalePreference,
    getSystemResolvedLocale,
    initializeLocale,
    setAppLocale,
} from './i18n.config'

it('ships no dictionary in the first render and loads the selected language on demand', async () => {
    // Until a dictionary loads, messages render their English defaultMessage.
    expect(i18n.global.availableLocales).toEqual([])
    const systemLocale = getSystemResolvedLocale()
    await initializeLocale()
    expect(i18n.global.locale.value).toBe(systemLocale)
    expect(Object.keys(i18n.global.getLocaleMessage(systemLocale)).length).toBeGreaterThan(0)
    await setAppLocale('zh-CN')
    expect(i18n.global.locale.value).toBe('zh-CN')
    expect(Object.keys(i18n.global.getLocaleMessage('zh-CN')).length).toBeGreaterThan(0)
    // English backs up keys another dictionary lacks, loaded once the app is idle.
    await vi.waitFor(
        () => expect(Object.keys(i18n.global.getLocaleMessage('en-US')).length).toBeGreaterThan(0),
        { timeout: 15_000 },
    )
    await setAppLocale('not-a-locale')
    expect(i18n.global.locale.value).toBe('en-US')
})

it('reuses a cached explicit preference before the database becomes available', async () => {
    await applyLocalePreference('de-DE')
    await setAppLocale('en-US')
    await initializeLocale()
    expect(i18n.global.locale.value).toBe('de-DE')
    localStorage.removeItem('axolotl.startup-locale')
})
