import { expect, it } from 'vitest'

import { changelogTranslationRequest } from './resource-changelog.ts'
import type { ResourceText, SourceReleaseRef } from './resources.ts'

it('changelog translations isolate source, project, release, body format and target language', () => {
    const reference: SourceReleaseRef = {
        project: { source: 'modrinth', projectId: 'project' },
        releaseId: 'release',
    }
    const text: ResourceText = { source: 'modrinth', format: 'markdown', body: 'Changes' }
    const request = changelogTranslationRequest(reference, text, 'zh-CN', [])
    expect(request.context).not.toEqual(
        changelogTranslationRequest(
            { ...reference, project: { source: 'curseforge', projectId: 'project' } },
            text,
            'zh-CN',
            [],
        ).context,
    )
    expect(request.context).not.toEqual(
        changelogTranslationRequest({ ...reference, releaseId: 'another' }, text, 'zh-CN', [])
            .context,
    )
    expect(request.context).not.toEqual(
        changelogTranslationRequest(reference, { ...text, body: 'New changes' }, 'zh-CN', [])
            .context,
    )
    expect(request.context).not.toEqual(
        changelogTranslationRequest(reference, { ...text, format: 'html' }, 'zh-CN', []).context,
    )
    expect(request.target_language).not.toBe(
        changelogTranslationRequest(reference, text, 'ja-JP', []).target_language,
    )
})
