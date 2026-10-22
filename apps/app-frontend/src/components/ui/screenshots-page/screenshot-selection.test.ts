import assert from 'node:assert/strict'
import test from 'node:test'

import {
    createScreenshotSelection,
    rectangleSelection,
    screenshotGridTargets,
} from './screenshot-selection.ts'

test('ranges follow displayed order, reverse cleanly, and allow additive modifiers', () => {
    const selection = createScreenshotSelection()
    const order = ['a', 'b', 'c', 'd', 'e']
    selection.markAnchor('d')
    selection.select('b', order, { shiftKey: true })
    assert.deepEqual([...selection.selectedKeys.value], ['b', 'c', 'd'])
    selection.select('c', order, { shiftKey: true })
    assert.deepEqual([...selection.selectedKeys.value], ['c', 'd'])
    selection.select('a', order, { ctrlKey: true })
    selection.select('b', order, { shiftKey: true, metaKey: true })
    assert.deepEqual(new Set(selection.selectedKeys.value), new Set(['a', 'b', 'c', 'd']))
})

test('group selection includes virtualized rows, toggles only that group, and clears hidden selections and anchors', () => {
    const selection = createScreenshotSelection()
    const group = Array.from({ length: 100 }, (_, index) => `row-${index}`)
    selection.select('other', [...group, 'other'])
    selection.toggleGroup(group)
    assert.equal(selection.selectedKeys.value.size, 101)
    selection.toggleGroup(group)
    assert.deepEqual([...selection.selectedKeys.value], ['other'])
    selection.select('row-20', group)
    selection.reconcile(['row-50', 'row-51'])
    assert.equal(selection.selectedKeys.value.size, 0)
    selection.select('row-51', ['row-50', 'row-51'], { shiftKey: true })
    assert.deepEqual([...selection.selectedKeys.value], ['row-51'])
})

test('rectangle selection uses full virtual geometry and never intersects collapsed rows or gaps', () => {
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
    assert.deepEqual(rectangleSelection({ left: 0, top: 50, width: 220, height: 200 }, targets), [
        'a',
        'b',
        'c',
        'd',
        'e',
        'f',
    ])
    assert.deepEqual(rectangleSelection({ left: 100, top: 50, width: 12, height: 50 }, targets), [])
    const selection = createScreenshotSelection()
    selection.box(['c', 'd'], new Set(['a']))
    assert.deepEqual([...selection.selectedKeys.value], ['a', 'c', 'd'])
    selection.box(['c'], new Set(['a']))
    assert.deepEqual([...selection.selectedKeys.value], ['a', 'c'])
    selection.clear()
    assert.equal(selection.selectedKeys.value.size, 0)
})

test('cancelling a rectangle preserves the click anchor and ignores obsolete rendered cards', () => {
    const selection = createScreenshotSelection()
    const order = ['a', 'b', 'c', 'd']
    selection.select('a', order)
    const original = new Set(selection.selectedKeys.value)
    selection.box(['c', 'd'], new Set())
    selection.box([], original)
    selection.select('b', order, { shiftKey: true })
    assert.deepEqual([...selection.selectedKeys.value], ['a', 'b'])
    selection.reconcile(['a'])
    selection.select('d', ['a'])
    assert.deepEqual([...selection.selectedKeys.value], ['a'])
})
