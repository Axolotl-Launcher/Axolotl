import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import { MAX_WINDOW_SIZE, usableWindowSize } from './window-size.ts'

describe('usableWindowSize', () => {
    it('keeps finite numbers and rounds them', () => {
        assert.equal(usableWindowSize(1280), 1280)
        assert.equal(usableWindowSize(1920.4), 1920)
        assert.equal(usableWindowSize(0), 0)
        assert.equal(usableWindowSize(MAX_WINDOW_SIZE), MAX_WINDOW_SIZE)
    })

    it('rejects the values an empty or half-typed number input produces', () => {
        assert.equal(usableWindowSize(undefined), null)
        assert.equal(usableWindowSize(null), null)
        assert.equal(usableWindowSize(Number.NaN), null)
        assert.equal(usableWindowSize(Number.POSITIVE_INFINITY), null)
        assert.equal(usableWindowSize('1280'), null)
    })

    it('rejects values outside the u16 range', () => {
        assert.equal(usableWindowSize(-1), null)
        assert.equal(usableWindowSize(MAX_WINDOW_SIZE + 1), null)
    })
})
