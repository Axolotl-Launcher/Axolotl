import type { ModelStreamEvent } from './types.ts'

export interface ActivityEntry {
    id: string
    kind: 'model' | 'tool' | 'note'
    name?: string
    file?: string
    query?: string
    startLine?: number
    endLine?: number
    text?: string
    lines?: number
    state: 'processing' | 'ready' | 'error'
}

export function applyProgress(entries: ActivityEntry[], event: ModelStreamEvent): ActivityEntry[] {
    const next = entries.map((entry) => ({ ...entry }))
    const data = event.data
    if (event.event_type === 'model_start')
        next.push({ id: `model-${data.turn}`, kind: 'model', state: 'processing' })
    if (event.event_type === 'model_end') {
        const model = [...next]
            .reverse()
            .find((entry) => entry.kind === 'model' && entry.state === 'processing')
        if (model) model.state = 'ready'
        if (data.has_tools && typeof data.content === 'string' && data.content.trim())
            next.push({
                id: `note-${model?.id ?? crypto.randomUUID()}`,
                kind: 'note',
                state: 'ready',
                text: visibleDraft(data.content).slice(0, 1200),
            })
    }
    if (event.event_type === 'tool')
        next.push({
            id: String(data.action_id ?? crypto.randomUUID()),
            kind: 'tool',
            name: String(data.name ?? ''),
            file:
                typeof data.file === 'string'
                    ? data.file.slice(0, 200)
                    : typeof data.file_id === 'string'
                      ? data.file_id
                      : undefined,
            query: typeof data.query === 'string' ? data.query.slice(0, 100) : undefined,
            startLine: typeof data.start_line === 'number' ? data.start_line : undefined,
            endLine: typeof data.end_line === 'number' ? data.end_line : undefined,
            state: 'processing',
        })
    if (event.event_type === 'tool_result') {
        const tool = data.action_id
            ? next.find((entry) => entry.id === data.action_id)
            : [...next]
                  .reverse()
                  .find((entry) => entry.kind === 'tool' && entry.state === 'processing')
        if (tool) {
            tool.state = data.failed ? 'error' : 'ready'
            if (typeof data.file === 'string') tool.file = data.file.slice(0, 200)
            if (typeof data.lines === 'number') tool.lines = data.lines
        }
    }
    return next.slice(-60)
}

/** Hide the structured result trailer while it is incomplete. */
export function visibleDraft(value: string): string {
    return value
        .replace(/```json[\s\S]*$/i, '')
        .replace(/`{1,3}(?:j(?:s(?:o(?:n)?)?)?)?$/i, '')
        .trimEnd()
}
