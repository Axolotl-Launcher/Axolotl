import { expect, it } from 'vitest'

import { shouldShowMinecraftCrash } from './process.js'

it('only shows the crash dialog for an explicitly crashed process', () => {
    expect(shouldShowMinecraftCrash(true)).toBe(true)
    expect(shouldShowMinecraftCrash(false)).toBe(false)
    expect(shouldShowMinecraftCrash(undefined)).toBe(false)
    expect(shouldShowMinecraftCrash(null)).toBe(false)
    expect(shouldShowMinecraftCrash(1)).toBe(false)
})
