import assert from 'node:assert/strict'
import { test } from 'node:test'

import { applyProgress, visibleDraft } from './progress.ts'

test('parallel tools update by identity and keep readable evidence metadata', () => {
    let entries = applyProgress([], { event_type: 'model_start', data: { turn: 1 } })
    entries = applyProgress(entries, {
        event_type: 'model_end',
        data: { has_tools: true, content: 'I will check the crash context.' },
    })
    for (const id of ['a', 'b'])
        entries = applyProgress(entries, {
            event_type: 'tool',
            data: { name: 'grep_log_file', action_id: id, file_id: 'log-1', query: id },
        })
    entries = applyProgress(entries, {
        event_type: 'tool_result',
        data: { action_id: 'a', name: 'grep_log_file', file: 'latest.log', lines: 8 },
    })
    assert.equal(entries.find((entry) => entry.id === 'a')?.state, 'ready')
    assert.equal(entries.find((entry) => entry.id === 'b')?.state, 'processing')
    assert.equal(entries.find((entry) => entry.id === 'a')?.file, 'latest.log')
    assert.equal(entries[1].text, 'I will check the crash context.')
    assert.equal(entries[0].state, 'ready')
})

test('streaming drafts hide incomplete machine-readable trailers', () => {
    assert.equal(visibleDraft('Diagnosis\n```json\n{"rootCause":'), 'Diagnosis')
    assert.equal(visibleDraft('Diagnosis\n```jso'), 'Diagnosis')
    assert.equal(visibleDraft('Diagnosis'), 'Diagnosis')
})

test('activity storage stays bounded with unique identities after many cloud tool events', () => {
    let entries: import('./progress.ts').ActivityEntry[] = []
    for (let index = 0; index < 100; index++)
        entries = applyProgress(entries, { event_type: 'tool', data: { name: 'remote tool' } })
    assert.equal(entries.length, 60)
    assert.equal(new Set(entries.map((entry) => entry.id)).size, 60)
})
