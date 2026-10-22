import assert from 'node:assert/strict'
import test from 'node:test'

import { changelogTranslationRequest } from './resource-changelog.ts'
import type { ResourceText, SourceReleaseRef } from './resources.ts'

test('changelog translations isolate source, project, release, body format and target language', () => {
    const reference: SourceReleaseRef = {
        project: { source: 'modrinth', projectId: 'project' },
        releaseId: 'release',
    }
    const text: ResourceText = { source: 'modrinth', format: 'markdown', body: 'Changes' }
    const request = changelogTranslationRequest(reference, text, 'zh-CN', [])
    assert.notDeepEqual(
        request.context,
        changelogTranslationRequest(
            { ...reference, project: { source: 'curseforge', projectId: 'project' } },
            text,
            'zh-CN',
            [],
        ).context,
    )
    assert.notDeepEqual(
        request.context,
        changelogTranslationRequest({ ...reference, releaseId: 'another' }, text, 'zh-CN', [])
            .context,
    )
    assert.notDeepEqual(
        request.context,
        changelogTranslationRequest(reference, { ...text, body: 'New changes' }, 'zh-CN', [])
            .context,
    )
    assert.notDeepEqual(
        request.context,
        changelogTranslationRequest(reference, { ...text, format: 'html' }, 'zh-CN', []).context,
    )
    assert.notEqual(
        request.target_language,
        changelogTranslationRequest(reference, text, 'ja-JP', []).target_language,
    )
})
