import { expect, it } from 'vitest'

import { applyProgress, visibleDraft } from './progress.ts'

it('parallel tools update by identity and keep readable evidence metadata', () => {
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
    expect(entries.find((entry) => entry.id === 'a')?.state).toBe('ready')
    expect(entries.find((entry) => entry.id === 'b')?.state).toBe('processing')
    expect(entries.find((entry) => entry.id === 'a')?.file).toBe('latest.log')
    expect(entries[1].text).toBe('I will check the crash context.')
    expect(entries[0].state).toBe('ready')
})

it('streaming drafts hide incomplete machine-readable trailers', () => {
    expect(visibleDraft('Diagnosis\n```json\n{"rootCause":')).toBe('Diagnosis')
    expect(visibleDraft('Diagnosis\n```jso')).toBe('Diagnosis')
    expect(visibleDraft('Diagnosis')).toBe('Diagnosis')
})

it('activity storage stays bounded with unique identities after many cloud tool events', () => {
    let entries: import('./progress.ts').ActivityEntry[] = []
    for (let index = 0; index < 100; index++)
        entries = applyProgress(entries, { event_type: 'tool', data: { name: 'remote tool' } })
    expect(entries.length).toBe(60)
    expect(new Set(entries.map((entry) => entry.id)).size).toBe(60)
})
