import assert from 'node:assert/strict'
import { test } from 'node:test'

import { shouldShowScrollToTop } from './scroll-to-top-visibility.ts'

test('shows the control only while scrolling down past the threshold', () => {
    assert.equal(shouldShowScrollToTop(100, undefined), false)
    assert.equal(shouldShowScrollToTop(400, undefined), true)
    assert.equal(shouldShowScrollToTop(400, 300), true)
    assert.equal(shouldShowScrollToTop(300, 400), false)
    assert.equal(shouldShowScrollToTop(0, 400), false)
})
