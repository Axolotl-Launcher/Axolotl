import assert from 'node:assert/strict'
import test from 'node:test'

import type { ContentItem } from '@modrinth/ui'

import { contentProjectUrl } from './content-project-links.ts'

test('content sharing follows the actual source and never guesses a Modrinth slug for another provider', () => {
    const item = {
        project: { id: 'curseforge:42', slug: 'same-title' },
        origin_provider: 'curseforge',
        provider_refs: [
            { provider: 'modrinth', project_id: 'abcdefgh' },
            { provider: 'curseforge', project_id: 42 },
        ],
    } as ContentItem
    assert.equal(contentProjectUrl(item), 'https://www.curseforge.com/projects/42')
    assert.equal(
        contentProjectUrl({ ...item, origin_provider: 'modrinth' }),
        'https://modrinth.com/project/abcdefgh',
    )
    assert.equal(contentProjectUrl({ ...item, origin_provider: 'mcarchive' }), null)
    assert.equal(
        contentProjectUrl({ ...item, project: { ...item.project!, id: 'local:entry' } }),
        null,
    )
})
