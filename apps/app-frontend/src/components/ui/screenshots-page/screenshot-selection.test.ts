import { expect, it } from 'vitest'

import {
    createScreenshotSelection,
    rectangleSelection,
    screenshotGridTargets,
} from './screenshot-selection.ts'

it('ranges follow displayed order, reverse cleanly, and allow additive modifiers', () => {
    const selection = createScreenshotSelection()
    const order = ['a', 'b', 'c', 'd', 'e']
    selection.markAnchor('d')
    selection.select('b', order, { shiftKey: true })
    expect([...selection.selectedKeys.value]).toEqual(['b', 'c', 'd'])
    selection.select('c', order, { shiftKey: true })
    expect([...selection.selectedKeys.value]).toEqual(['c', 'd'])
    selection.select('a', order, { ctrlKey: true })
    selection.select('b', order, { shiftKey: true, metaKey: true })
    expect(new Set(selection.selectedKeys.value)).toEqual(new Set(['a', 'b', 'c', 'd']))
})

it('group selection includes virtualized rows, toggles only that group, and clears hidden selections and anchors', () => {
    const selection = createScreenshotSelection()
    const group = Array.from({ length: 100 }, (_, index) => `row-${index}`)
    selection.select('other', [...group, 'other'])
    selection.toggleGroup(group)
    expect(selection.selectedKeys.value.size).toBe(101)
    selection.toggleGroup(group)
    expect([...selection.selectedKeys.value]).toEqual(['other'])
    selection.select('row-20', group)
    selection.reconcile(['row-50', 'row-51'])
    expect(selection.selectedKeys.value.size).toBe(0)
    selection.select('row-51', ['row-50', 'row-51'], { shiftKey: true })
    expect([...selection.selectedKeys.value]).toEqual(['row-51'])
})

it('rectangle selection uses full virtual geometry and never intersects collapsed rows or gaps', () => {
    const targets = screenshotGridTargets(
        [
            { keys: ['a', 'b', 'c', 'd', 'e', 'f'], gridTop: 50, isOpen: true },
            { keys: ['hidden'], gridTop: 400, isOpen: false },
        ],
        2,
        100,
        56,
        12,
    )
    expect(rectangleSelection({ left: 0, top: 50, width: 220, height: 200 }, targets)).toEqual([
        'a',
        'b',
        'c',
        'd',
        'e',
        'f',
    ])
    expect(rectangleSelection({ left: 100, top: 50, width: 12, height: 50 }, targets)).toEqual([])
    const selection = createScreenshotSelection()
    selection.box(['c', 'd'], new Set(['a']))
    expect([...selection.selectedKeys.value]).toEqual(['a', 'c', 'd'])
    selection.box(['c'], new Set(['a']))
    expect([...selection.selectedKeys.value]).toEqual(['a', 'c'])
    selection.clear()
    expect(selection.selectedKeys.value.size).toBe(0)
})

it('cancelling a rectangle preserves the click anchor and ignores obsolete rendered cards', () => {
    const selection = createScreenshotSelection()
    const order = ['a', 'b', 'c', 'd']
    selection.select('a', order)
    const original = new Set(selection.selectedKeys.value)
    selection.box(['c', 'd'], new Set())
    selection.box([], original)
    selection.select('b', order, { shiftKey: true })
    expect([...selection.selectedKeys.value]).toEqual(['a', 'b'])
    selection.reconcile(['a'])
    selection.select('d', ['a'])
    expect([...selection.selectedKeys.value]).toEqual(['a'])
})
