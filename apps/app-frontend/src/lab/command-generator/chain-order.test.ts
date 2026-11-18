import { describe, expect, it } from 'vitest'

import { moveChainItem } from './chain-order.ts'

const chain = ['as', 'at', 'facing']

describe('moveChainItem', () => {
    it('moves an entry to a later position', () => {
        expect(moveChainItem(chain, 0, 2)).toEqual(['at', 'facing', 'as'])
    })

    it('moves an entry to an earlier position', () => {
        expect(moveChainItem(chain, 2, 0)).toEqual(['facing', 'as', 'at'])
    })

    it('keeps the original list untouched', () => {
        const next = moveChainItem(chain, 0, 1)
        expect(next).toEqual(['at', 'as', 'facing'])
        expect(chain).toEqual(['as', 'at', 'facing'])
    })

    it('ignores moves that would leave the list', () => {
        expect(moveChainItem(chain, 0, 3)).toEqual(chain)
        expect(moveChainItem(chain, -1, 0)).toEqual(chain)
        expect(moveChainItem(chain, 1, 1)).toEqual(chain)
    })

    it('handles an empty list', () => {
        expect(moveChainItem([], 0, 1)).toEqual([])
    })
})
