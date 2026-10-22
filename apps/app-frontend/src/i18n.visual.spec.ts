import { expect, it } from 'vitest'

import i18n, {
    applyLocalePreference,
    getSystemResolvedLocale,
    initializeLocale,
    setAppLocale,
} from './i18n.config'

it('renders with English immediately and loads the selected language on demand', async () => {
    expect(i18n.global.availableLocales).toEqual(['en-US'])
    expect(Object.keys(i18n.global.getLocaleMessage('en-US')).length).toBeGreaterThan(0)
    const systemLocale = getSystemResolvedLocale()
    await initializeLocale()
    expect(i18n.global.availableLocales.sort()).toEqual(
        [...new Set(['en-US', systemLocale])].sort(),
    )
    expect(i18n.global.locale.value).toBe(systemLocale)
    expect(Object.keys(i18n.global.getLocaleMessage('en-US')).length).toBeGreaterThan(0)
    await setAppLocale('zh-CN')
    expect(i18n.global.locale.value).toBe('zh-CN')
    expect(Object.keys(i18n.global.getLocaleMessage('zh-CN')).length).toBeGreaterThan(0)
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
