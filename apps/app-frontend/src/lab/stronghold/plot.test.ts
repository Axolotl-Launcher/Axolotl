import assert from 'node:assert/strict'
import test from 'node:test'

import { calculate, generationBands, isGenerationChunk } from './core.ts'
import { chunkGridStep, direction, fitView, packHeatmap, worldAt, zoomAt } from './plot.ts'

test('generation support uses the base ring prior, independent of observations', () => {
    assert.equal(generationBands().length, 8)
    assert.equal(isGenerationChunk(0, 0), false)
    assert.equal(isGenerationChunk(128, 0), true)
    assert.equal(isGenerationChunk(-128, 0), true)
    assert.equal(isGenerationChunk(210, 0), false)
    const bands = generationBands()
    for (const ring of bands) {
        assert.ok(ring.inner > 0 && ring.outer > ring.inner)
        const x = Math.round((ring.inner + ring.outer) / 32)
        assert.equal(isGenerationChunk(x, 0), true)
    }
})

test('Minecraft yaw directions and cursor-anchored zoom preserve world orientation', () => {
    assert.deepEqual(direction(0), { x: -0, z: 1 })
    assert.ok(Math.abs(direction(90).x + 1) < 1e-12)
    assert.ok(Math.abs(direction(-90).x - 1) < 1e-12)
    const view = { x: -1000, z: 500, span: 4096 }
    const anchor = worldAt(view, 800, 400, 175, 290)
    const zoomed = zoomAt(view, 0.5, anchor)
    assert.deepEqual(worldAt(zoomed, 800, 400, 175, 290), anchor)
    assert.equal(zoomed.span, 2048)
    assert.equal(zoomAt(view, 1e-10, anchor).span, 32)
})

test('packing retains finite positive posterior weights and fit includes observations and credible candidates', () => {
    const packed = packHeatmap([
        { x: -100, z: 40, weight: 0.999 },
        { x: 100000, z: 100000, weight: 0.001 },
        { x: 1, z: 2, weight: NaN },
        { x: 3, z: 4, weight: 0 },
    ])
    assert.equal(packed.length, 6)
    const view = fitView([{ x: 20, z: -40, angle: 0 }], packed, 2)
    for (const point of [
        { x: 20, z: -40 },
        { x: -1592, z: 648 },
    ]) {
        assert.ok(Math.abs(point.x - view.x) <= view.span / 2)
        assert.ok(Math.abs(point.z - view.z) <= view.span / 4)
    }
    assert.ok(view.span < 10000)
    assert.deepEqual(fitView([], new Float64Array(), 2), { x: 0, z: 0, span: 4096 })
})

const liveSamples = [
    {
        observations: [
            [9.5, 44.5, 65.1],
            [-141.15, -108.92, 59.85],
            [-402.7, -363.38, 48.45],
            [-611.82, -443.66, 42],
            [-845.81, -440.52, 36],
            [-1110.35, -443.71, 27],
            [-1368.35, -406.09, 18.75],
        ],
        expected: [-1788, 868],
    },
    {
        observations: [
            [10000.5, 10000.5, -127.5],
            [10246.82, 10030.02, -135.45],
            [10584.35, 10030.92, -149.55],
            [10809.73, 10027.92, -161.1],
        ],
        expected: [11108, 9156],
    },
]
for (const [i, sample] of liveSamples.entries()) {
    test(`26.3 user-verified sample ${i + 1} retains the prediction and complete plotted distribution`, () => {
        const input = {
            version: 'post119' as const,
            advanced: true,
            eyes: sample.observations.map(([x, z, angle]) => ({ x, z, angle, sigma: 0.1 })),
        }
        const ordinary = calculate(input)
        const { distribution, ...result } = calculate(input, true)
        assert.deepEqual(result, ordinary)
        const best = result.predictions[0]
        assert.deepEqual([best.x * 16 + 4, best.z * 16 + 4], sample.expected)
        const packed = packHeatmap(distribution!)
        assert.deepEqual(Array.from(packed.slice(0, 3)), [best.x, best.z, best.weight])
        let mass = 0
        for (let j = 2; j < packed.length; j += 3) mass += packed[j]
        assert.ok(Math.abs(mass - 1) < 1e-10)
    })
}

test('map ticks follow the chunk lattice across overview and close zoom levels', () => {
    for (const span of [32, 128, 1000, 4096, 53000, 120000000]) {
        const step = chunkGridStep(span)
        assert.equal(step % 16, 0)
        assert.ok(Number.isInteger(Math.log2(step / 16)))
        assert.ok(step >= 16)
    }
    assert.equal(chunkGridStep(4096), 1024)
    assert.equal(chunkGridStep(32), 16)
})
