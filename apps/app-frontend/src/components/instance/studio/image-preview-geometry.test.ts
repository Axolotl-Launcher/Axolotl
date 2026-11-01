import { expect, it } from 'vitest'

import {
    clampPreviewScale,
    fitPreviewScale,
    MAX_PREVIEW_SCALE,
    MIN_PREVIEW_SCALE,
} from './image-preview-geometry'

it('scales a screenshot down until its larger side fits the stage', () => {
    expect(fitPreviewScale({ width: 3840, height: 2160 }, { width: 960, height: 540 })).toBe(0.25)
    expect(fitPreviewScale({ width: 2160, height: 3840 }, { width: 960, height: 540 })).toBe(
        0.140625,
    )
})

it('scales a texture up so a 16×16 tile is legible', () => {
    expect(fitPreviewScale({ width: 16, height: 16 }, { width: 640, height: 480 })).toBe(30)
})

it('never scales past the zoom range', () => {
    expect(fitPreviewScale({ width: 1, height: 1 }, { width: 4000, height: 4000 })).toBe(
        MAX_PREVIEW_SCALE,
    )
    expect(clampPreviewScale(0)).toBe(MIN_PREVIEW_SCALE)
    expect(clampPreviewScale(Number.NaN)).toBe(1)
    expect(clampPreviewScale(1000)).toBe(MAX_PREVIEW_SCALE)
})

it('falls back to actual size before the image or the stage has a size', () => {
    expect(fitPreviewScale({ width: 0, height: 0 }, { width: 640, height: 480 })).toBe(1)
    expect(fitPreviewScale({ width: 64, height: 64 }, { width: 0, height: 0 })).toBe(1)
})
