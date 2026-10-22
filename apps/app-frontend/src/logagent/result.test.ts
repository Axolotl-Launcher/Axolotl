import assert from 'node:assert/strict'
import { test } from 'node:test'

import { parseLogAgentInsight } from './result.ts'

test('structured diagnostics preserve unknown confidence and unrelated code blocks', () => {
    const result = parseLogAgentInsight(
        'Explanation\n```json\n{"example":true}\n```\n```json\n{"rootCause":"Java mismatch","steps":["Change Java",17],"evidence":["latest.log:200"]}\n```',
    )
    assert.equal(result.insight?.confidence, null)
    assert.deepEqual(result.insight?.steps, ['Change Java'])
    assert.match(result.markdown, /example/)
    assert.doesNotMatch(result.markdown, /rootCause/)
})

test('malformed JSON and plain Markdown remain readable', () => {
    for (const text of [
        'Useful plain text',
        'Explanation\n```json\n{broken}\n```',
        '```json\nnull\n```',
    ]) {
        assert.deepEqual(parseLogAgentInsight(text), { markdown: text, insight: null })
    }
})
