import { describe, expect, it } from 'vitest'

import type { BannerDesign } from './types.ts'
import { withLayerReplaced } from './types.ts'

const design: BannerDesign = {
    baseColor: 'white',
    layers: [
        { id: 'bottom', pattern: 'stripe_bottom', color: 'red' },
        { id: 'middle', pattern: 'cross', color: 'blue' },
        { id: 'top', pattern: 'border', color: 'black' },
    ],
}

describe('withLayerReplaced', () => {
    it('keeps the slot of the layer it replaces', () => {
        const preview = withLayerReplaced(design, 'middle', {
            id: 'pending',
            pattern: 'creeper',
            color: 'lime',
        })
        expect(preview.layers.map((layer) => layer.id)).toEqual(['bottom', 'pending', 'top'])
        expect(preview.layers[1]).toMatchObject({ pattern: 'creeper', color: 'lime' })
    })

    it('appends when the layer is new, which is where it will be applied', () => {
        const preview = withLayerReplaced(design, null, {
            id: 'pending',
            pattern: 'creeper',
            color: 'lime',
        })
        expect(preview.layers.map((layer) => layer.id)).toEqual([
            'bottom',
            'middle',
            'top',
            'pending',
        ])
    })

    it('appends when the id is no longer in the design', () => {
        const preview = withLayerReplaced(design, 'gone', {
            id: 'pending',
            pattern: 'creeper',
            color: 'lime',
        })
        expect(preview.layers.map((layer) => layer.id)).toEqual([
            'bottom',
            'middle',
            'top',
            'pending',
        ])
    })

    it('leaves the original design untouched', () => {
        withLayerReplaced(design, 'top', { id: 'pending', pattern: 'creeper', color: 'lime' })
        expect(design.layers.map((layer) => layer.id)).toEqual(['bottom', 'middle', 'top'])
        expect(design.baseColor).toBe('white')
    })
})
