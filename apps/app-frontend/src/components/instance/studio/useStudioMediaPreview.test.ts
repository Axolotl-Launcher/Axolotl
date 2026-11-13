import { expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'

import type { StudioDocument } from './useStudioDocuments'
import { useStudioMediaPreview } from './useStudioMediaPreview'

function documentFor(name: string, kind: StudioDocument['kind']): StudioDocument {
    return { kind, path: name, name, content: '', savedContent: '', saving: false }
}

function preview(name: string, kind: StudioDocument['kind'], bytes: Uint8Array) {
    const scope = effectScope()
    const state = scope.run(() =>
        useStudioMediaPreview(ref(documentFor(name, kind)), async () => bytes),
    )
    if (!state) throw new Error('the composable needs a scope')
    return { state, stop: () => scope.stop() }
}

it('keeps image bytes for the editor to draw', async () => {
    const bytes = Uint8Array.from([1, 2, 3])
    const { state, stop } = preview('block.png', 'image', bytes)
    try {
        await vi.waitFor(() => expect(state.url.value).toMatch(/^blob:/))
        expect(state.bytes.value).toBe(3)
        expect(state.content.value).toBe(bytes)
    } finally {
        stop()
    }
})

it('does not hold video bytes once the preview URL exists', async () => {
    const bytes = Uint8Array.from([1, 2, 3, 4])
    const { state, stop } = preview('clip.mp4', 'video', bytes)
    try {
        await vi.waitFor(() => expect(state.url.value).toMatch(/^blob:/))
        // The video plays from the URL; a second copy of the file in memory is
        // what makes a large recording exhaust the webview.
        expect(state.bytes.value).toBe(4)
        expect(state.content.value).toBeNull()
    } finally {
        stop()
    }
})
