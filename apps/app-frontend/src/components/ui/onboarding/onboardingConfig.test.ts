import { expect, it } from 'vitest'

import { onboardingTours } from './onboardingConfig.ts'

/**
 * The schematic tab ships its own two-step tour. Its targets live in the browse
 * component, so a rename there would silently leave the tour pointing at nothing
 * — these assertions pin the pairing.
 */

it('the schematic tour explains the source filter before the download action', () => {
    const steps = onboardingTours['schematic-browse']
    expect(Array.isArray(steps)).toBe(true)
    expect(steps).toHaveLength(2)
    expect(steps.map((step) => step.targetId)).toEqual([
        'schematic-source-filter',
        'schematic-download-action',
    ])
})

it('the schematic tour only inspects, never activates controls', () => {
    // Inspect steps let any click advance; an activate step would perform the
    // real action (start a download) just by walking the tour.
    for (const step of onboardingTours['schematic-browse']) {
        expect(step.interaction).toBe('inspect')
    }
})

it('every registered tour names a target for each of its steps', () => {
    for (const [mode, steps] of Object.entries(onboardingTours)) {
        for (const step of steps) {
            expect(step.id, `${mode} has a step without id`).toBeTruthy()
            if (step.interaction !== 'manual') {
                expect(step.targetId, `${mode}/${step.id} has no target`).toBeTruthy()
            }
        }
    }
})
