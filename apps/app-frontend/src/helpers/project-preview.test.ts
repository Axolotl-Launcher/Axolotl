import { expect, it } from 'vitest'

import { createProjectPreviewState, readProjectPreviewTitle } from './project-preview.ts'

it('project previews match the complete provider-qualified destination', () => {
    const state = createProjectPreviewState('/project/curseforge/12', 'Known name')
    expect(readProjectPreviewTitle('/project/curseforge/12', state)).toBe('Known name')
    expect(readProjectPreviewTitle('/project/12', state)).toBe(undefined)
    expect(readProjectPreviewTitle('/project/curseforge/13', state)).toBe(undefined)
    expect(readProjectPreviewTitle('/resource/12', state)).toBe(undefined)
})

it('direct navigation and malformed history do not invent project titles', () => {
    for (const state of [
        null,
        {},
        { projectPreview: null },
        { projectPreview: { targetPath: '/project/12', title: 12 } },
        createProjectPreviewState('/project/12', ' '),
    ]) {
        expect(readProjectPreviewTitle('/project/12', state)).toBe(undefined)
    }
})
