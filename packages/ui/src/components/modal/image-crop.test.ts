import assert from 'node:assert/strict'
import test from 'node:test'

import {
    coverScale,
    extensionForMimeType,
    fitCropFrame,
    MAX_CROP_ZOOM,
    normalizeAspectRatio,
    outputSizeForSource,
    panView,
    resetView,
    sourceRect,
    zoomViewAt,
} from './image-crop.ts'

const image = { width: 4000, height: 3000 }
const frame = { width: 800, height: 450 }

test('fits a centered frame using the available width or height', () => {
    assert.deepEqual(fitCropFrame(800, 450, 16 / 9, 0), {
        left: 0,
        top: 0,
        width: 800,
        height: 450,
    })
    assert.deepEqual(fitCropFrame(800, 800, 16 / 9, 0), {
        left: 0,
        top: 175,
        width: 800,
        height: 450,
    })
    assert.deepEqual(fitCropFrame(800, 800, 1, 0), {
        left: 0,
        top: 0,
        width: 800,
        height: 800,
    })
})

test('applies frame padding symmetrically and clamps degenerate containers', () => {
    const padded = fitCropFrame(800, 450, 16 / 9, 24)
    assert.equal(padded.top, 24)
    assert.equal(padded.height, 402)
    assert.ok(Math.abs(padded.left - 42.66666666666667) < 0.0001)
    assert.ok(Math.abs(padded.width - 714.6666666666666) < 0.0001)

    const empty = fitCropFrame(0, 0, 16 / 9, 24)
    assert.ok(Number.isFinite(empty.left))
    assert.ok(Number.isFinite(empty.top))
    assert.ok(Number.isFinite(empty.width))
    assert.ok(Number.isFinite(empty.height))
    assert.ok(empty.width > 0)
    assert.ok(empty.height > 0)
})

test('normalizes invalid aspect ratios', () => {
    assert.equal(normalizeAspectRatio(4 / 3), 4 / 3)
    assert.equal(normalizeAspectRatio(0), 16 / 9)
    assert.equal(normalizeAspectRatio(Number.NaN), 16 / 9)
    assert.equal(normalizeAspectRatio(-1), 16 / 9)
})

test('uses the largest cover scale for portrait, landscape, and square sources', () => {
    assert.equal(coverScale({ width: 4000, height: 3000 }, frame), 0.2)
    assert.equal(coverScale({ width: 1000, height: 3000 }, frame), 0.8)
    assert.equal(coverScale({ width: 1000, height: 1000 }, frame), 0.8)
    assert.equal(coverScale({ width: 0, height: 0 }, frame), 1)
})

test('resets to the centered maximum crop', () => {
    assert.deepEqual(resetView(image), {
        zoom: 1,
        centerX: 2000,
        centerY: 1500,
    })
    assert.deepEqual(sourceRect(resetView(image), image, frame), {
        left: 0,
        top: 375,
        width: 4000,
        height: 2250,
    })
})

test('clamps panning so the frame never leaves the image', () => {
    const moved = panView(resetView(image), 0, 100, image, frame)
    assert.equal(moved.centerX, 2000)
    assert.equal(moved.centerY, 1125)

    const horizontal = panView(resetView(image), 100, 0, image, frame)
    assert.equal(horizontal.centerX, 2000)

    const zoomed = { zoom: 2, centerX: 2000, centerY: 1500 }
    assert.equal(panView(zoomed, 10000, 10000, image, frame).centerX, 1000)
    assert.equal(panView(zoomed, -10000, -10000, image, frame).centerY, 2437.5)
})

test('keeps the focal point under the cursor while zooming', () => {
    const zoomed = zoomViewAt(resetView(image), 2, 100, 0, image, frame)
    assert.equal(zoomed.zoom, 2)
    assert.equal(zoomed.centerX, 2250)
    assert.equal(zoomed.centerY, 1500)

    const clamped = zoomViewAt(resetView(image), 100, 0, 0, image, frame)
    assert.equal(clamped.zoom, MAX_CROP_ZOOM)
})

test('keeps the source rectangle inside the image at every zoom level', () => {
    for (const zoom of [1, 1.5, 2, 4, MAX_CROP_ZOOM]) {
        for (const centerX of [0, 1000, 2000, 4000]) {
            const rect = sourceRect({ zoom, centerX, centerY: 1500 }, image, frame)
            assert.ok(rect.left >= 0)
            assert.ok(rect.top >= 0)
            assert.ok(rect.left + rect.width <= image.width + 0.0001)
            assert.ok(rect.top + rect.height <= image.height + 0.0001)
        }
    }
})

test('caps output dimensions without upscaling', () => {
    assert.deepEqual(outputSizeForSource({ width: 4000, height: 2250 }, 3840), {
        width: 3840,
        height: 2160,
    })
    assert.deepEqual(outputSizeForSource({ width: 400, height: 225 }, 3840), {
        width: 400,
        height: 225,
    })
    assert.deepEqual(outputSizeForSource({ width: 0, height: 0 }, 3840), {
        width: 1,
        height: 1,
    })
    assert.deepEqual(outputSizeForSource({ width: 4000, height: 2250 }, 0), {
        width: 4000,
        height: 2250,
    })
})

test('maps only supported crop output MIME types to extensions', () => {
    assert.equal(extensionForMimeType('image/png'), 'png')
    assert.equal(extensionForMimeType('IMAGE/WEBP'), 'webp')
    assert.equal(extensionForMimeType('image/jpeg'), null)
    assert.equal(extensionForMimeType(''), null)
})
