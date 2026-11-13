import { describe, expect, it } from 'vitest'

import { pendingOobeSteps, withCompletedOobeStep } from './progress'
import { OOBE_STEP_IDS } from './step-ids'

const steps = [{ id: 'language_v1' }, { id: 'account_v1' }, { id: 'welcome_v1' }]

describe('pendingOobeSteps', () => {
    it('shows every step to a new install', () => {
        expect(pendingOobeSteps(steps, []).map((step) => step.id)).toEqual([
            'language_v1',
            'account_v1',
            'welcome_v1',
        ])
        expect(pendingOobeSteps(steps, undefined)).toHaveLength(3)
    })

    it('shows only the steps a release added since the last start', () => {
        expect(
            pendingOobeSteps(steps, ['language_v1', 'welcome_v1']).map((step) => step.id),
        ).toEqual(['account_v1'])
    })

    it('shows nothing once every step was seen, whatever else the record holds', () => {
        expect(
            pendingOobeSteps(steps, ['welcome_v1', 'theme_v9', 'account_v1', 'language_v1']),
        ).toEqual([])
    })
})

describe('withCompletedOobeStep', () => {
    it('appends a step once', () => {
        const record = withCompletedOobeStep(['language_v1'], 'account_v1')
        expect(record).toEqual(['language_v1', 'account_v1'])
        expect(withCompletedOobeStep(record, 'account_v1')).toEqual(record)
    })

    it('keeps ids from a newer build', () => {
        expect(withCompletedOobeStep(['theme_v9'], 'language_v1')).toEqual([
            'theme_v9',
            'language_v1',
        ])
    })
})

describe('OOBE step ids', () => {
    it('are unique and versioned', () => {
        expect(new Set(OOBE_STEP_IDS).size).toBe(OOBE_STEP_IDS.length)
        for (const id of OOBE_STEP_IDS) expect(id).toMatch(/^[a-z][a-z0-9_]*_v[1-9][0-9]*$/)
    })
})
