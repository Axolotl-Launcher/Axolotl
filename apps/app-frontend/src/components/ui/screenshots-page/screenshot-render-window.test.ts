import { expect, it } from 'vitest'

import { screenshotRenderWindows } from './screenshot-render-window.ts'

it('renders visible rows and only two following images in a large screenshot collection', () => {
    const grids = [{ gridTop: 0, gridHeight: 250000, count: 10000, isOpen: true }]
    expect(screenshotRenderWindows(grids, 1000, 1250, 4, 100)).toEqual([
        { start: 40, end: 54, top: 1000 },
    ])
})

it('shares lookahead across groups and skips closed and empty groups', () => {
    const grids = [
        { gridTop: 0, gridHeight: 100, count: 2, isOpen: true },
        { gridTop: 140, gridHeight: 100, count: 1, isOpen: true },
        { gridTop: 280, gridHeight: 100, count: 99, isOpen: false },
        { gridTop: 420, gridHeight: 0, count: 0, isOpen: true },
        { gridTop: 560, gridHeight: 5000, count: 100, isOpen: true },
    ]
    const windows = screenshotRenderWindows(grids, 0, 100, 2, 100)
    expect(windows.map(({ start, end }) => end - start)).toEqual([2, 1, 0, 0, 1])
})

it('does not render offscreen originals or keep old ranges after a large scroll', () => {
    const grids = [
        { gridTop: 0, gridHeight: 100, count: 2, isOpen: true },
        { gridTop: 140, gridHeight: 500000, count: 10000, isOpen: true },
    ]
    expect(screenshotRenderWindows(grids, 10000, 10200, 2, 100)).toEqual([
        { start: 0, end: 0, top: 0 },
        { start: 196, end: 204, top: 9800 },
    ])
})
