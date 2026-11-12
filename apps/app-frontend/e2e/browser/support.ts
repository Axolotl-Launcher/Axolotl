import { browser, expect } from '@wdio/globals'

afterEach(async () => {
    const diagnostics = await browser.execute(() => ({
        errors: (window as unknown as { __e2eErrors?: string[] }).__e2eErrors,
        missingCommands:
            (window as unknown as { __demoUnhandled?: string[] }).__demoUnhandled ?? [],
    }))
    expect(diagnostics.errors).toBeDefined()
    expect(diagnostics.errors).toEqual([])
    expect(diagnostics.missingCommands).toEqual([])
})

export async function waitForHome() {
    await browser.$('.home-floating-controls').waitForDisplayed({ timeout: 60_000 })
    await browser.waitUntil(async () => (await browser.execute(() => location.pathname)) === '/')
    await expect(browser.$('.nav-rail a[href="/"]')).toHaveElementClass('nav-router-active')
}

export async function openSettings() {
    const settings = browser.$('a[href="/settings"]')
    await settings.waitForClickable()
    await settings.click()
    await browser.$('#settings-target-appearance-color-theme').waitForDisplayed()
    await browser.waitUntil(
        async () => (await browser.execute(() => location.pathname)) === '/settings',
    )
}

export async function assertTheme(mode: 'light' | 'dark' | 'oled') {
    const expected = {
        light: { surface: '#ebebeb', text: '#1a202c' },
        dark: { surface: '#101215', text: '#ffffff' },
        oled: { surface: '#000000', text: '#ffffff' },
    }[mode]
    await browser.waitUntil(
        async () => {
            const result = await browser.execute(() => {
                const root = document.documentElement
                const style = getComputedStyle(root)
                return {
                    classes: [...root.classList].filter((value) =>
                        ['light-mode', 'dark-mode', 'oled-mode'].includes(value),
                    ),
                    surface: style.getPropertyValue('--surface-1').trim().toLowerCase(),
                    text: style.getPropertyValue('--color-text-primary').trim().toLowerCase(),
                }
            })
            return (
                result.classes.length === 1 &&
                result.classes[0] === `${mode}-mode` &&
                result.surface === expected.surface &&
                result.text === expected.text
            )
        },
        { timeoutMsg: `Theme class and surface/text tokens did not settle on ${mode}` },
    )
}
