import assert from 'node:assert/strict'
import test from 'node:test'

import { createProjectPreviewState, readProjectPreviewTitle } from './project-preview.ts'

test('project previews match the complete provider-qualified destination', () => {
    const state = createProjectPreviewState('/project/curseforge/12', 'Known name')
    assert.equal(readProjectPreviewTitle('/project/curseforge/12', state), 'Known name')
    assert.equal(readProjectPreviewTitle('/project/12', state), undefined)
    assert.equal(readProjectPreviewTitle('/project/curseforge/13', state), undefined)
    assert.equal(readProjectPreviewTitle('/resource/12', state), undefined)
})

test('direct navigation and malformed history do not invent project titles', () => {
    for (const state of [
        null,
        {},
        { projectPreview: null },
        { projectPreview: { targetPath: '/project/12', title: 12 } },
        createProjectPreviewState('/project/12', ' '),
    ]) {
        assert.equal(readProjectPreviewTitle('/project/12', state), undefined)
    }
})
