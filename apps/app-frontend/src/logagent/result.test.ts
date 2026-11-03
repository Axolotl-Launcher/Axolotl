import { expect, it } from 'vitest'

import { parseLogAgentInsight } from './result.ts'

it('structured diagnostics preserve unknown confidence and unrelated code blocks', () => {
    const result = parseLogAgentInsight(
        'Explanation\n```json\n{"example":true}\n```\n```json\n{"rootCause":"Java mismatch","steps":["Change Java",17],"evidence":["latest.log:200"]}\n```',
    )
    expect(result.insight?.confidence).toBe(null)
    expect(result.insight?.steps).toEqual(['Change Java'])
    expect(result.markdown).toMatch(/example/)
    expect(result.markdown).not.toMatch(/rootCause/)
})

it('malformed JSON and plain Markdown remain readable', () => {
    for (const text of [
        'Useful plain text',
        'Explanation\n```json\n{broken}\n```',
        '```json\nnull\n```',
    ]) {
        expect(parseLogAgentInsight(text)).toEqual({ markdown: text, insight: null })
    }
})
