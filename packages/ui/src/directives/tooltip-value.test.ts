import assert from 'node:assert/strict'
import test from 'node:test'

import { resolveTooltipContent } from './tooltip-value.ts'

/**
 * The suppression contract, pinned.
 *
 * Callers suppress a tooltip by passing something falsy -- a ternary that
 * yields `undefined` when a value is unavailable. About thirty call sites rely on that meaning "no
 * tooltip", so an empty string must not become an empty bubble.
 */

test('a plain string is the tooltip text', () => {
    assert.deepEqual(resolveTooltipContent('Hello'), { text: 'Hello', options: {} })
})

test('every falsy value suppresses the tooltip', () => {
    assert.equal(resolveTooltipContent(undefined), null)
    assert.equal(resolveTooltipContent(null), null)
    assert.equal(resolveTooltipContent(''), null)
})

test('an object with no content suppresses the tooltip', () => {
    assert.equal(resolveTooltipContent({}), null)
    assert.equal(resolveTooltipContent({ content: null }), null)
    assert.equal(resolveTooltipContent({ content: undefined }), null)
    assert.equal(resolveTooltipContent({ content: '' }), null)
})

test('an object carries its options through', () => {
    const value = { content: 'Tip', html: true, placement: 'bottom' as const }
    assert.deepEqual(resolveTooltipContent(value), { text: 'Tip', options: value })
})

test('suppression is independent of the extra options', () => {
    // A styled-but-empty tooltip would still render a stray bubble, so the
    // options must not keep one alive once the content is gone.
    assert.equal(resolveTooltipContent({ content: '', popperClass: 'storage-tooltip' }), null)
})
