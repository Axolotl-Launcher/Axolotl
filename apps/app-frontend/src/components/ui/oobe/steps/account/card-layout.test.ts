import { describe, expect, it } from 'vitest'

import { layoutCards, moveOf, withPickOnHiddenCard } from './card-layout'

describe('layoutCards', () => {
    it('puts heads on the back cards only once there are two or more accounts', () => {
        expect(layoutCards({}, [], undefined)).toEqual({})
        expect(layoutCards({}, ['alex'], 'alex')).toEqual({ front: 'alex' })
        expect(layoutCards({}, ['alex', 'notch'], 'alex')).toEqual({
            front: 'alex',
            left: 'notch',
        })
        expect(layoutCards({}, ['alex', 'notch', 'steve'], 'notch')).toEqual({
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

    it('hides a card behind the stack from four accounts', () => {
        expect(layoutCards({}, ['alex', 'notch', 'steve'], 'alex')).toEqual({
            front: 'alex',
            left: 'notch',
            right: 'steve',
        })
        expect(layoutCards({}, ['alex', 'notch', 'steve', 'zuri', 'kai'], 'alex')).toEqual({
            front: 'alex',
            left: 'notch',
            right: 'steve',
            hidden: 'zuri',
        })
    })

    it('draws an account without a visible card out of the hidden card', () => {
        const ids = ['alex', 'notch', 'steve', 'zuri', 'kai', 'ida']
        const shown = { front: 'alex', left: 'notch', right: 'steve', hidden: 'zuri' }
        // `kai` is on no card, so it takes the hidden card's place while that card is covered.
        const before = withPickOnHiddenCard(shown, ids, 'kai')
        expect(before).toEqual({ ...shown, hidden: 'kai' })
        const after = layoutCards(before, ids, 'kai')
        expect(after).toEqual({ front: 'kai', left: 'notch', right: 'steve', hidden: 'alex' })
        expect(moveOf(before, after, 'kai')).toBe('draw')
        expect(moveOf(before, after, 'alex')).toBe('stow')
        expect(moveOf(before, after, 'notch')).toBeUndefined()
    })

    it('leaves picks of visible cards and the first layout alone', () => {
        const ids = ['alex', 'notch', 'steve', 'zuri']
        const shown = { front: 'alex', left: 'notch', right: 'steve', hidden: 'zuri' }
        expect(withPickOnHiddenCard(shown, ids, 'notch')).toBe(shown)
        expect(withPickOnHiddenCard(shown, ids.slice(0, 3), 'zuri')).toBe(shown)
        expect(withPickOnHiddenCard({}, ids, 'zuri')).toEqual({})
    })

    it('tucks the front card into the hidden place when a new account signs in to a full stack', () => {
        const before = { front: 'alex', left: 'notch', right: 'steve' }
        const ids = ['alex', 'notch', 'steve', 'zuri']
        const after = layoutCards(withPickOnHiddenCard(before, ids, 'zuri'), ids, 'zuri')
        expect(after).toEqual({ front: 'zuri', left: 'notch', right: 'steve', hidden: 'alex' })
    })

    it('shows no account in front while none is selected', () => {
        expect(layoutCards({}, ['alex', 'notch'], undefined)).toEqual({
            left: 'alex',
            right: 'notch',
        })
    })
})
