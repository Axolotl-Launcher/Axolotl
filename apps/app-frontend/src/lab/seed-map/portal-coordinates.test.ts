import assert from 'node:assert/strict'
import test from 'node:test'

import {
    netherFromOverworld,
    overworldFromNether,
    PORTAL_COORDINATE_SCALE,
    portalCounterpart,
} from './portal-coordinates.ts'

test('overworld coordinates are divided by eight', () => {
    assert.equal(netherFromOverworld(0), 0)
    assert.equal(netherFromOverworld(8), 1)
    assert.equal(netherFromOverworld(100), 12)
    assert.equal(netherFromOverworld(-8), -1)
})

test('negative overworld coordinates round down like Mth.floor', () => {
    assert.equal(netherFromOverworld(-1), -1)
    assert.equal(netherFromOverworld(-7), -1)
    assert.equal(netherFromOverworld(-9), -2)
})

test('nether coordinates are multiplied by eight', () => {
    assert.equal(overworldFromNether(0), 0)
    assert.equal(overworldFromNether(12), 96)
    assert.equal(overworldFromNether(-12), -96)
})

test('the overworld maps to the nether and back', () => {
    assert.deepEqual(portalCounterpart('overworld', 100, -200), {
        dimension: 'nether',
        x: 12,
        z: -25,
    })
    assert.deepEqual(portalCounterpart('nether', 12, -25), {
        dimension: 'overworld',
        x: 96,
        z: -200,
    })
})

test('the end has no counterpart', () => {
    assert.equal(portalCounterpart('end', 0, 0), null)
})

test('the scale constant matches the portal conversion', () => {
    assert.equal(PORTAL_COORDINATE_SCALE, 8)
    assert.equal(overworldFromNether(netherFromOverworld(96)), 96)
})
