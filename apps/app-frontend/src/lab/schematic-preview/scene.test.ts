import { expect, it } from 'vitest'

import { filterNativeWalkMouseDelta } from './scene.ts'

it('native walk mouse input ignores cursor-wrap spikes', () => {
    expect(filterNativeWalkMouseDelta(24)).toBe(24)
    expect(filterNativeWalkMouseDelta(-128)).toBe(-128)
    expect(filterNativeWalkMouseDelta(129)).toBe(0)
    expect(filterNativeWalkMouseDelta(Number.POSITIVE_INFINITY)).toBe(0)
})
