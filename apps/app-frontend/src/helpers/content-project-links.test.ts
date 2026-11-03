import type { ContentItem } from '@modrinth/ui'
import { expect, it } from 'vitest'

import { contentProjectUrl } from './content-project-links.ts'

it('content sharing follows the actual source and never guesses a Modrinth slug for another provider', () => {
    const item = {
        project: { id: 'curseforge:42', slug: 'same-title' },
        origin_provider: 'curseforge',
        provider_refs: [
            { provider: 'modrinth', project_id: 'abcdefgh' },
            { provider: 'curseforge', project_id: 42 },
        ],
    } as ContentItem
    expect(contentProjectUrl(item)).toBe('https://www.curseforge.com/projects/42')
    expect(contentProjectUrl({ ...item, origin_provider: 'modrinth' })).toBe(
        'https://modrinth.com/project/abcdefgh',
    )
    expect(contentProjectUrl({ ...item, origin_provider: 'mcarchive' })).toBe(null)
    expect(contentProjectUrl({ ...item, project: { ...item.project!, id: 'local:entry' } })).toBe(
        null,
    )
})
