import { expect, it } from 'vitest'

import { hitSeedMapMarker, seedMapMarkerLayout } from './marker-rendering.ts'
import type { SeedMapMarker } from './workspace.ts'

const marker = (id: string, x = 100, z = 100, name = id): SeedMapMarker => ({
    id,
    x,
    z,
    name,
    color: '#22C55E',
    dimension: 'overworld',
})
const project = (x: number, z: number) => ({ x, y: z })
const measure = (text: string) => Array.from(text).length * 7
const viewport = { width: 400, height: 300 }

it('pins end at the map coordinate and names stay above at both icon scales', () => {
    for (const scale of [1, 1.5]) {
        const [placement] = seedMapMarkerLayout(
            [marker('Home')],
            project,
            viewport,
            scale,
            undefined,
            measure,
        )
        expect(placement.pin.y + placement.pin.height).toBe(placement.point.y)
        expect(placement.pin.width).toBe(22 * scale)
        expect(placement.label).toBeTruthy()
        expect(placement.label.y + placement.label.height < placement.pin.y).toBeTruthy()
        expect(placement.label.height).toBe(18)
        expect(hitSeedMapMarker([placement], 100, 100)?.id).toBe('Home')
        expect(hitSeedMapMarker([placement], 100, placement.pin.y + 5)?.id).toBe('Home')
        expect(hitSeedMapMarker([placement], 100, placement.label.y + 9)?.id).toBe('Home')
        expect(hitSeedMapMarker([placement], 100, 120)).toBe(undefined)
    }
})

it('overlapping names favor the selected marker and clicks favor the frontmost pin', () => {
    const placements = seedMapMarkerLayout(
        [marker('selected'), marker('other')],
        project,
        viewport,
        1,
        'selected',
        measure,
    )
    expect(placements.at(-1)?.marker.id).toBe('selected')
    expect(placements.filter(({ label }) => label).length).toBe(1)
    expect(placements.at(-1)?.label?.text).toBe('selected')
    expect(hitSeedMapMarker(placements, 100, 80)?.id).toBe('selected')
})

it('labels never cover another pin and offscreen names cannot be selected', () => {
    const placements = seedMapMarkerLayout(
        [marker('lower', 100, 145), marker('upper'), marker('offscreen', -100)],
        project,
        viewport,
        1,
        undefined,
        measure,
    )
    expect(placements.length).toBe(2)
    expect(placements.find(({ marker }) => marker.id === 'lower')?.label).toBe(undefined)
    expect(hitSeedMapMarker(placements, -100, 100)).toBe(undefined)
})

it('long Unicode labels fit the viewport without changing saved names', () => {
    const original = marker('long', 2, 100, '基地😀'.repeat(40))
    const [placement] = seedMapMarkerLayout([original], project, viewport, 1, undefined, measure)
    expect(placement.label?.text.endsWith('…')).toBeTruthy()
    expect(measure(placement.label.text) <= 160).toBeTruthy()
    expect(placement.label.x >= 4).toBeTruthy()
    expect(original.name).toBe('基地😀'.repeat(40))
    const [right] = seedMapMarkerLayout(
        [marker('right', 399)],
        project,
        viewport,
        1,
        undefined,
        measure,
    )
    expect(right.label && right.label.x + right.label.width <= 396).toBeTruthy()
    const [top] = seedMapMarkerLayout(
        [marker('top', 100, 40)],
        project,
        viewport,
        1,
        undefined,
        measure,
    )
    expect(top.label).toBe(undefined)
})
