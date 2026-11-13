import { browser, expect } from '@wdio/globals'

import { openSettings, waitForHome } from './support'

describe('desktop navigation', () => {
    it('opens settings from Home and returns through the navigation rail', async () => {
        await waitForHome()
        await openSettings()
        await expect(browser.$('.settings-fixed-render input[type="search"]')).toBeDisplayed()
        await expect(browser.$('.home-floating-controls')).not.toBeDisplayed()

        await browser.$('.nav-rail a[href="/"]').click()
        await waitForHome()
        await expect(browser.$('.settings-fixed-render')).not.toBeDisplayed()
    })
})
