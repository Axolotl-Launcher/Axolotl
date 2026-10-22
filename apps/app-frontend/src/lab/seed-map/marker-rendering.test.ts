import assert from 'node:assert/strict'
import test from 'node:test'

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

test('pins end at the map coordinate and names stay above at both icon scales', () => {
    for (const scale of [1, 1.5]) {
        const [placement] = seedMapMarkerLayout(
            [marker('Home')],
            project,
            viewport,
            scale,
            undefined,
            measure,
        )
        assert.equal(placement.pin.y + placement.pin.height, placement.point.y)
        assert.equal(placement.pin.width, 22 * scale)
        assert.ok(placement.label)
        assert.ok(placement.label.y + placement.label.height < placement.pin.y)
        assert.equal(placement.label.height, 18)
        assert.equal(hitSeedMapMarker([placement], 100, 100)?.id, 'Home')
        assert.equal(hitSeedMapMarker([placement], 100, placement.pin.y + 5)?.id, 'Home')
        assert.equal(hitSeedMapMarker([placement], 100, placement.label.y + 9)?.id, 'Home')
        assert.equal(hitSeedMapMarker([placement], 100, 120), undefined)
    }
})

test('overlapping names favor the selected marker and clicks favor the frontmost pin', () => {
    const placements = seedMapMarkerLayout(
        [marker('selected'), marker('other')],
        project,
        viewport,
        1,
        'selected',
        measure,
    )
    assert.equal(placements.at(-1)?.marker.id, 'selected')
    assert.equal(placements.filter(({ label }) => label).length, 1)
    assert.equal(placements.at(-1)?.label?.text, 'selected')
    assert.equal(hitSeedMapMarker(placements, 100, 80)?.id, 'selected')
})

test('labels never cover another pin and offscreen names cannot be selected', () => {
    const placements = seedMapMarkerLayout(
        [marker('lower', 100, 145), marker('upper'), marker('offscreen', -100)],
        project,
        viewport,
        1,
        undefined,
        measure,
    )
    assert.equal(placements.length, 2)
    assert.equal(placements.find(({ marker }) => marker.id === 'lower')?.label, undefined)
    assert.equal(hitSeedMapMarker(placements, -100, 100), undefined)
})

test('long Unicode labels fit the viewport without changing saved names', () => {
    const original = marker('long', 2, 100, '基地😀'.repeat(40))
    const [placement] = seedMapMarkerLayout([original], project, viewport, 1, undefined, measure)
    assert.ok(placement.label?.text.endsWith('…'))
    assert.ok(measure(placement.label.text) <= 160)
    assert.ok(placement.label.x >= 4)
    assert.equal(original.name, '基地😀'.repeat(40))
    const [right] = seedMapMarkerLayout(
        [marker('right', 399)],
        project,
        viewport,
        1,
        undefined,
        measure,
    )
    assert.ok(right.label && right.label.x + right.label.width <= 396)
    const [top] = seedMapMarkerLayout(
        [marker('top', 100, 40)],
        project,
        viewport,
        1,
        undefined,
        measure,
    )
    assert.equal(top.label, undefined)
})
