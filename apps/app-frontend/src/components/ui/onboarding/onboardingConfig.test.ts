import assert from 'node:assert/strict'
import test from 'node:test'

import { onboardingTours } from './onboardingConfig.ts'

/**
 * The schematic tab ships its own two-step tour. Its targets live in the browse
 * component, so a rename there would silently leave the tour pointing at nothing
 * — these assertions pin the pairing.
 */

test('the schematic tour explains the source filter before the download action', () => {
    const steps = onboardingTours['schematic-browse']
    assert.ok(Array.isArray(steps), 'tour must be registered')
    assert.equal(steps.length, 2)

    const targets = steps.map((step) => step.targetId)
    assert.deepEqual(targets, ['schematic-source-filter', 'schematic-download-action'])
})

test('the schematic tour only inspects, never activates controls', () => {
    // Inspect steps let any click advance; an activate step would perform the
    // real action (start a download) just by walking the tour.
    for (const step of onboardingTours['schematic-browse']) {
        assert.equal(step.interaction, 'inspect')
    }
})

test('every registered tour names a target for each of its steps', () => {
    for (const [mode, steps] of Object.entries(onboardingTours)) {
        for (const step of steps) {
            assert.ok(step.id, `${mode} has a step without id`)
            if (step.interaction !== 'manual') {
                assert.ok(step.targetId, `${mode}/${step.id} has no target`)
            }
        }
    }
})
