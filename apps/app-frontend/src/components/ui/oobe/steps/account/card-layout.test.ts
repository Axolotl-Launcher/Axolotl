import { describe, expect, it } from 'vitest'

import { layoutCards, moveOf } from './card-layout'

describe('layoutCards', () => {
    it('puts heads on the back cards only once there are two or more accounts', () => {
        expect(layoutCards({}, [], undefined)).toEqual({})
        expect(layoutCards({}, ['alex'], 'alex')).toEqual({ front: 'alex' })
        expect(layoutCards({}, ['alex', 'notch'], 'alex')).toEqual({
            front: 'alex',
            left: 'notch',
        })
        expect(layoutCards({}, ['alex', 'notch', 'steve', 'zuri'], 'notch')).toEqual({
            front: 'notch',
            left: 'alex',
            right: 'steve',
        })
    })

    it('trades the picked back card with the front one', () => {
        const before = { front: 'alex', left: 'notch', right: 'steve' }
        const after = layoutCards(before, ['alex', 'notch', 'steve'], 'steve')
        expect(after).toEqual({ front: 'steve', left: 'notch', right: 'alex' })
        expect(moveOf(before, after, 'steve')).toBe('rise')
        expect(moveOf(before, after, 'alex')).toBe('sink')
        expect(moveOf(before, after, 'notch')).toBeUndefined()
    })

    it('sends the front card to a free slot when a new account signs in', () => {
        const before = { front: 'alex' }
        const after = layoutCards(before, ['alex', 'notch'], 'notch')
        expect(after).toEqual({ front: 'notch', left: 'alex' })
        expect(moveOf(before, after, 'notch')).toBe('enter')
        expect(moveOf(before, after, 'alex')).toBe('sink')
    })

    it('replaces the left card when a hidden account is picked and both slots are taken', () => {
        const before = { front: 'alex', left: 'notch', right: 'steve' }
        expect(layoutCards(before, ['alex', 'notch', 'steve', 'zuri'], 'zuri')).toEqual({
            front: 'zuri',
            left: 'alex',
            right: 'steve',
        })
    })

    it('shows no account in front while none is selected', () => {
        expect(layoutCards({}, ['alex', 'notch'], undefined)).toEqual({
            left: 'alex',
            right: 'notch',
        })
    })
})
