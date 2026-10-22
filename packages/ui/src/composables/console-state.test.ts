import assert from 'node:assert/strict'
import test from 'node:test'

import { createConsoleState } from './console-state.ts'

test('keeps the tail of a large short-line history within the line budget', async () => {
    const state = createConsoleState()
    await state.addLegacyLog(Array.from({ length: 100_000 }, (_, i) => `line-${i}`).join('\n'))
    assert.equal(state.output.value.length, 20_000)
    assert.equal(state.output.value[0].text, 'line-80000')
    assert.equal(state.output.value.at(-1)?.text, 'line-99999')
    await state.addLegacyLog('next')
    assert.equal(state.output.value[0].text, 'line-80001')
    assert.equal(state.output.value.at(-1)?.text, 'next')
})

test('preserves inherited error severity when the retained tail starts within a stack trace', async () => {
    for (const separator of ['\n', '\r', '\r\n']) {
        const state = createConsoleState()
        await state.addLegacyLog(
            `[12:00:00] [main/ERROR] failure${separator}` +
                `continuation${separator}`.repeat(21_000),
        )
        assert.equal(state.output.value.length, 20_000)
        assert.ok(state.output.value.every((line) => line.level === 'error'))
    }
})

test('bounds effective text size and individual lines while accepting CRLF and blank lines', async () => {
    const state = createConsoleState()
    await state.addLegacyLog(('a'.repeat(80_000) + '\r\n\r\n').repeat(100))
    assert.ok(state.output.value.every((line) => line.text.length <= 64 * 1024))
    assert.ok(
        state.output.value.reduce((sum, line) => sum + line.text.length, 0) <= 4 * 1024 * 1024,
    )
    assert.equal(state.output.value.length, 64)
    assert.ok(state.output.value[0].text.includes('truncated'))
})

test('clear discards pending live lines so they cannot reappear after a reset', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] })
    const state = createConsoleState()
    state.addLog4jEvent({ message: 'pending', level: 'INFO' })
    state.clear()
    await state.addLegacyLog('new history')
    t.mock.timers.tick(300)
    assert.deepEqual(
        state.output.value.map((line) => line.text),
        ['new history'],
    )
})
