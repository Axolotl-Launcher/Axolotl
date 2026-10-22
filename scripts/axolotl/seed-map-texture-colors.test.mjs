import assert from 'node:assert/strict'
import test from 'node:test'

import { averageTextureColor, srgbToLinear } from './seed-map-texture-colors.mjs'

test('transparent pixels do not darken texture colors and partial alpha is weighted', () => {
    assert.deepEqual(
        averageTextureColor({ width: 2, height: 1, data: [255, 0, 0, 255, 0, 0, 255, 0] }),
        [1, 0, 0],
    )
    const color = averageTextureColor({
        width: 2,
        height: 1,
        data: [255, 0, 0, 255, 0, 0, 255, 128],
    })
    assert.ok(Math.abs(color[0] - 255 / 383) < 0.000001)
    assert.ok(Math.abs(color[2] - 128 / 383) < 0.000001)
    assert.equal(averageTextureColor({ width: 1, height: 1, data: [255, 255, 255, 0] }), null)
})

test('averages RGB in linear space rather than averaging encoded sRGB values', () => {
    assert.deepEqual(
        averageTextureColor({ width: 2, height: 1, data: [0, 0, 0, 255, 255, 255, 255, 255] }),
        [0.5, 0.5, 0.5],
    )
    assert.ok(Math.abs(srgbToLinear(0.5) - 0.214041) < 0.000001)
})

test('animation frame selection and display duration determine the color', () => {
    const image = { width: 1, height: 2, data: [255, 0, 0, 255, 0, 0, 255, 255] }
    assert.deepEqual(
        averageTextureColor(image, { frames: [{ index: 0, time: 3 }, 1] }),
        [0.75, 0, 0.25],
    )
    assert.deepEqual(averageTextureColor(image, { frames: [1] }), [0, 0, 1])
    assert.throws(() => averageTextureColor(image, { frames: [2] }))
})
