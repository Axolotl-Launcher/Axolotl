import { expect, it } from 'vitest'

import {
    createDefaultHomeDashboard,
    normalizeHomeDashboard,
    setHomeMinimalOptions,
} from './home-dashboard.ts'
import {
    HOME_MINIMAL_PURE,
    HOME_MINIMAL_STANDARD,
    normalizeHomeMinimalOptions,
    resolveHomeMinimalLayout,
    setHomeMinimalPreset,
    updateHomeMinimalLayout,
} from './home-minimal.ts'

it('reads missing or broken options as the standard layout, so upgrades look the same', () => {
    for (const saved of [undefined, null, 'pure', [], {}]) {
        const options = normalizeHomeMinimalOptions(saved)
        expect(options.preset).toBe('standard')
        expect(resolveHomeMinimalLayout(options)).toEqual(HOME_MINIMAL_STANDARD)
    }
    // A config saved before the options existed has none, and the page uses the standard layout.
    const legacy = normalizeHomeDashboard(createDefaultHomeDashboard())
    expect(legacy?.minimal).toBe(undefined)
    expect(resolveHomeMinimalLayout(legacy?.minimal)).toEqual(HOME_MINIMAL_STANDARD)
})

it('keeps valid custom values and replaces unknown ones field by field', () => {
    const options = normalizeHomeMinimalOptions({
        preset: 'custom',
        show: { date: false, greeting: 'no', recent: false, extra: true },
        surface: 'plain',
        arrangement: 'diagonal',
        position: 'center',
        density: 'compact',
    })
    expect(options).toEqual({
        preset: 'custom',
        show: {
            date: false,
            greeting: true,
            status: true,
            meta: true,
            switch: true,
            recent: false,
        },
        surface: 'plain',
        arrangement: 'row',
        position: 'center',
        density: 'compact',
    })
    expect(normalizeHomeMinimalOptions({ preset: 'fancy' }).preset).toBe('standard')
})

it('draws a preset from its own values, whatever custom values were saved alongside', () => {
    const saved = normalizeHomeMinimalOptions({
        preset: 'pure',
        surface: 'card',
        show: { date: true },
    })
    expect(resolveHomeMinimalLayout(saved)).toEqual(HOME_MINIMAL_PURE)
    // Pure: nothing but the icon, the name and the launch button, centred.
    expect(Object.values(HOME_MINIMAL_PURE.show).every((shown) => !shown)).toBe(true)
    expect(HOME_MINIMAL_PURE).toMatchObject({ arrangement: 'stacked', position: 'center' })
})

it('turns a change into a custom layout that starts from what was on screen', () => {
    const pure = setHomeMinimalPreset(null, 'pure')
    const custom = updateHomeMinimalLayout(pure, { show: { switch: true } })
    expect(custom.preset).toBe('custom')
    expect(resolveHomeMinimalLayout(custom)).toEqual({
        ...HOME_MINIMAL_PURE,
        show: { ...HOME_MINIMAL_PURE.show, switch: true },
    })

    // Choosing "custom" itself copies the current preset, so the page does not jump.
    expect(resolveHomeMinimalLayout(setHomeMinimalPreset(pure, 'custom'))).toEqual(
        HOME_MINIMAL_PURE,
    )
    // Choosing a preset again restores it in full.
    expect(resolveHomeMinimalLayout(setHomeMinimalPreset(custom, 'standard'))).toEqual(
        HOME_MINIMAL_STANDARD,
    )
})

it('saves the options with the dashboard config and keeps them through a reload', () => {
    const config = setHomeMinimalOptions(
        createDefaultHomeDashboard(),
        updateHomeMinimalLayout(null, { density: 'compact', show: { recent: false } }),
    )
    const reloaded = normalizeHomeDashboard(JSON.parse(JSON.stringify(config)))
    expect(reloaded?.minimal).toEqual(config.minimal)
    expect(reloaded?.widgets).toEqual(config.widgets)
    expect(resolveHomeMinimalLayout(reloaded?.minimal)).toMatchObject({
        density: 'compact',
        show: { recent: false, date: true },
    })
})
