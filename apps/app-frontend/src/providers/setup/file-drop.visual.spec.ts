import { beforeAll, beforeEach, expect, it, vi } from 'vitest'

import { setupFileDropProvider } from './file-drop'

const fixture = vi.hoisted(() => ({ listen: vi.fn() }))
vi.mock('@modrinth/ui', () => ({ provideFileDrop: vi.fn() }))
vi.mock('@tauri-apps/api/webview', () => ({
    getCurrentWebview: () => ({ onDragDropEvent: fixture.listen }),
}))

let receive: (event: { payload: unknown }) => void
const handler = vi.fn()
beforeAll(async () => {
    fixture.listen.mockImplementation(async (callback) => {
        receive = callback
        return () => {}
    })
    await setupFileDropProvider().listenNativeFileDrop(handler)
})
beforeEach(() => {
    handler.mockClear()
    window.dispatchEvent(new Event('dragend'))
    receive({ payload: { type: 'leave' } })
    handler.mockClear()
})
function drop(type: 'enter' | 'drop', paths = ['/fixture/server.zip']) {
    receive({ payload: { type, paths, position: { x: 10, y: 20 } } })
}
it('accepts the same file again after the previous flow closes without another enter event', () => {
    drop('enter')
    drop('drop')
    // Closing/cancelling the import modal must not require a fresh native enter.
    drop('drop')
    expect(handler.mock.calls.filter(([event]) => event.type === 'drop')).toHaveLength(2)
    expect(handler.mock.lastCall?.[0].paths).toEqual(['/fixture/server.zip'])
})
it('accepts a complete native drop after leave, but keeps internal HTML drags filtered', () => {
    drop('enter')
    receive({ payload: { type: 'leave' } })
    drop('drop')
    expect(handler.mock.lastCall?.[0].type).toBe('drop')
    handler.mockClear()
    window.dispatchEvent(new Event('dragstart'))
    drop('drop')
    expect(handler).not.toHaveBeenCalled()
    window.dispatchEvent(new Event('dragend'))
    drop('drop')
    expect(handler.mock.lastCall?.[0].type).toBe('drop')
})
it('ignores empty or position-only events when there is no external drag session', () => {
    drop('drop', [])
    receive({ payload: { type: 'over', position: { x: 10, y: 20 } } })
    expect(handler).not.toHaveBeenCalled()
})
