import { browser, expect } from '@wdio/globals'

import { assertTheme, openSettings, waitForHome } from './support'

describe('desktop theme persistence', () => {
    it('saves themes chosen in Settings and restores them after a reload', async () => {
        await waitForHome()
        await openSettings()
        for (const mode of ['light', 'dark', 'oled'] as const) {
            const option = browser.$(`.theme-options button:has(.preview.${mode}-mode)`)
            await option.waitForClickable()
            await option.click()
            await expect(option).toHaveElementClass('selected')
            await assertTheme(mode)
            await browser.waitUntil(
                async () =>
                    (
                        await browser.execute(() =>
                            JSON.parse(localStorage.getItem('axolotl-e2e-settings') ?? '{}'),
                        )
                    ).theme === mode,
                { timeoutMsg: `Settings never saved the ${mode} theme` },
            )

            await browser.refresh()
            await browser.$('#settings-target-appearance-color-theme').waitForDisplayed()
            await assertTheme(mode)
            await expect(
                browser.$(`.theme-options button:has(.preview.${mode}-mode)`),
            ).toHaveElementClass('selected')
        }
    })
})
