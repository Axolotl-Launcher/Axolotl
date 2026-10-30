import { expect, it } from 'vitest'
import { computed, ref } from 'vue'

import {
    bulkResolutionAction,
    filterBulkResolutionIds,
    initialCustomizeStrategy,
    UPGRADE_ACTIVE_STEPS,
    upgradeControlEnabled,
    upgradeProgressModel,
} from './flow-controls.ts'

it('registered upgrade control reads live ref values without re-registration', () => {
    const canPlan = ref(false)
    const control = computed(() => canPlan.value)
    expect(upgradeControlEnabled(control)).toBe(false)
    canPlan.value = true
    expect(upgradeControlEnabled(control)).toBe(true)
})

it('missing controls remain disabled', () => {
    expect(upgradeControlEnabled(undefined)).toBe(false)
})

it('upgrade progress maps five active routes and terminal result', () => {
    expect(UPGRADE_ACTIVE_STEPS.length).toBe(5)
    for (const [index, route] of UPGRADE_ACTIVE_STEPS.entries()) {
        expect(upgradeProgressModel(`/instance/example/upgrade/${route}`)).toEqual({
            currentIndex: index,
            complete: false,
            steps: UPGRADE_ACTIVE_STEPS,
        })
    }
    expect(upgradeProgressModel('/instance/example/upgrade/result').complete).toBe(true)
})

it('customize strategy prefers flow UI state over selected backend solution', () => {
    expect(initialCustomizeStrategy('custom', 'newest', 'custom')).toBe('custom')
    expect(initialCustomizeStrategy(null, 'minimal_change', 'custom')).toBe('minimal_change')
    expect(initialCustomizeStrategy(null, null, 'custom')).toBe('custom')
})

it('bulk resolution state and no-op filtering use authoritative actions', () => {
    expect(bulkResolutionAction(['keep', 'keep'])).toBe('keep')
    expect(bulkResolutionAction(['disable', 'disable'])).toBe('disable')
    expect(bulkResolutionAction(['keep', 'disable'])).toBe(null)
    expect(
        filterBulkResolutionIds(
            [
                { contentId: 'a', action: 'keep' },
                { contentId: 'b', action: 'disable' },
            ],
            'keep',
        ),
    ).toEqual(['b'])
})
