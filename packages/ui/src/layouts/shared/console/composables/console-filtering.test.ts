import assert from 'node:assert/strict'
import test from 'node:test'

import type { LogLine } from '../types.ts'
import { createConsoleLineFilter } from './console-filtering.ts'

test('filters only appended immutable lines and rebuilds after trim, query or source changes', () => {
    const filter = createConsoleLineFilter()
    const lines: LogLine[] = Array.from({ length: 1000 }, (_, i) => ({
        text: `${i}`,
        level: 'info',
    }))
    let calls = 0
    const predicate = (line: LogLine) => {
        calls++
        return line.text !== 'hidden'
    }
    assert.equal(filter(lines, 'all', predicate).length, 1000)
    assert.equal(calls, 1000)
    lines.push({ text: 'appended', level: 'info' })
    assert.equal(filter(lines, 'all', predicate).at(-1)?.originalIndex, 1000)
    assert.equal(calls, 1001)
    lines.shift()
    assert.equal(filter(lines, 'all', predicate)[0].line.text, '1')
    assert.equal(calls, 2001)
    filter(lines, 'changed', predicate)
    assert.equal(calls, 3001)
    assert.equal(filter([{ text: 'hidden', level: 'info' }], 'changed', predicate).length, 0)
    assert.equal(filter([], 'changed', predicate).length, 0)
})
