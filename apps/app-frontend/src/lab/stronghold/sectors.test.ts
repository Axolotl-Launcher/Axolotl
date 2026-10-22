import assert from 'node:assert/strict'
import test from 'node:test'

import { isGenerationChunk } from './core.ts'
import { calculateSectors, contributions, sectorWeight } from './sectors.ts'

test('linear angular weights handle boundaries, yaw wrap, backwards and the throw origin', () => {
    const eye = { x: 0, z: 0, angle: 0 }
    const point = (angle: number) => [
        -Math.sin((angle * Math.PI) / 180) * 2000,
        Math.cos((angle * Math.PI) / 180) * 2000,
    ]
    for (const [angle, expected] of [
        [0, 1],
        [2.5, 0.5],
        [-2.5, 0.5],
        [5, 0],
        [-5, 0],
        [180, 0],
    ]) {
        const [x, z] = point(angle)
        assert.ok(Math.abs(sectorWeight(eye, x, z, 5) - expected) < 1e-10)
    }
    assert.equal(sectorWeight(eye, 0, 0, 5), 0)
    assert.ok(
        Math.abs(
            sectorWeight({ ...eye, angle: 179 }, ...(point(-179) as [number, number]), 5) - 0.6,
        ) < 1e-10,
    )
})

test('independent opposing targets remain in the same heatmap, with additive scores', () => {
    const observations = [
        { x: 0, z: 0, angle: 0 },
        { x: 0, z: 0, angle: 180 },
    ]
    const input = { observations, halfAngle: 5, radius: 3000, version: 'post119' as const }
    const result = calculateSectors(input)
    assert.ok(result.distribution.some((c) => c.z > 100 && c.weight > 0.99))
    assert.ok(result.distribution.some((c) => c.z < -100 && c.weight > 0.99))
    for (const c of result.distribution) {
        assert.ok(isGenerationChunk(c.x, c.z))
        assert.ok(Math.hypot(c.x * 16, c.z * 16) <= 3000)
        assert.ok(
            Math.abs(
                contributions(input, c).reduce((sum, item) => sum + item.weight, 0) - c.weight,
            ) < 1e-12,
        )
    }
    assert.equal(
        calculateSectors({ ...input, observations: [observations[0], observations[0]] })
            .predictions[0].weight,
        2,
    )
    assert.throws(() => calculateSectors({ ...input, halfAngle: 0 }))
    assert.throws(() => calculateSectors({ ...input, radius: Infinity }))
})
