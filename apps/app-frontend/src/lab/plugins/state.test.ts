import { describe, expect, it } from 'vitest'

import {
    clearFailure,
    type LabPluginState,
    parseLabPluginState,
    setDisabled,
    setFailure,
} from './state'

const empty: LabPluginState = { disabled: [], failures: {} }

describe('parseLabPluginState', () => {
    it('falls back to an empty state when nothing is stored', () => {
        expect(parseLabPluginState(null)).toEqual(empty)
    })

    it('falls back to an empty state when storage holds invalid JSON', () => {
        expect(parseLabPluginState('{not json')).toEqual(empty)
    })

    it('drops entries that are not strings', () => {
        const parsed = parseLabPluginState('{"disabled":["a",3,null],"failures":{"b":"boom"}}')
        expect(parsed).toEqual({ disabled: ['a'], failures: { b: 'boom' } })
    })
})

describe('setDisabled', () => {
    it('adds an id without mutating the input', () => {
        const next = setDisabled(empty, 'plugin-a', true)
        expect(next.disabled).toEqual(['plugin-a'])
        expect(empty.disabled).toEqual([])
    })

    it('removes an id', () => {
        const next = setDisabled(
            { ...empty, disabled: ['plugin-a', 'plugin-b'] },
            'plugin-a',
            false,
        )
        expect(next.disabled).toEqual(['plugin-b'])
    })
})

describe('setFailure', () => {
    it('disables the plugin and stores the message', () => {
        const next = setFailure(empty, 'plugin-a', 'boom')
        expect(next.disabled).toEqual(['plugin-a'])
        expect(next.failures).toEqual({ 'plugin-a': 'boom' })
    })
})

describe('clearFailure', () => {
    it('removes the message and leaves the disabled list alone', () => {
        const state: LabPluginState = { disabled: ['plugin-a'], failures: { 'plugin-a': 'boom' } }
        const next = clearFailure(state, 'plugin-a')
        expect(next).toEqual({ disabled: ['plugin-a'], failures: {} })
    })

    it('returns the same object when there is nothing to clear', () => {
        expect(clearFailure(empty, 'plugin-a')).toBe(empty)
    })
})
