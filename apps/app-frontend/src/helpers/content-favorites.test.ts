import { expect, it } from 'vitest'

import {
    contentFavoriteKey,
    contentItemFavoriteInput,
    isFavoriteContentType,
} from './content-favorites.ts'

type FavoriteItem = Parameters<typeof contentItemFavoriteInput>[0]

function installedItem(overrides: Partial<FavoriteItem>) {
    return {
        project_type: 'mod',
        origin_provider: 'modrinth',
        provider_refs: [{ provider: 'modrinth', project_id: 'AANobbMI', version_id: null }],
        project: { id: 'AANobbMI', slug: 'sodium', title: 'Sodium', icon_url: undefined },
        ...overrides,
    } as FavoriteItem
}

it('saves an installed row under its online project', () => {
    expect(contentItemFavoriteInput(installedItem({}))).toEqual({
        provider: 'modrinth',
        project_id: 'AANobbMI',
        content_type: 'mod',
        slug: 'sodium',
    })
    expect(
        contentItemFavoriteInput(
            installedItem({
                project_type: 'shader',
                origin_provider: 'curseforge',
                provider_refs: [
                    { provider: 'modrinth', project_id: 'other', version_id: null },
                    { provider: 'curseforge', project_id: 238222, file_id: null },
                ],
            }),
        ),
    ).toMatchObject({ provider: 'curseforge', project_id: '238222', content_type: 'shader' })
})

it('offers no favorite for local files or unsupported content types', () => {
    expect(
        contentItemFavoriteInput(installedItem({ origin_provider: null, provider_refs: [] })),
    ).toBeNull()
    expect(contentItemFavoriteInput(installedItem({ project_type: 'modpack' }))).toBeNull()
})

it('content favorites keep provider-qualified identities separate', () => {
    expect(contentFavoriteKey('modrinth', 'same-id')).not.toBe(
        contentFavoriteKey('curseforge', 'same-id'),
    )
})

it('content favorites only accept installable content types', () => {
    expect(isFavoriteContentType('mod')).toBe(true)
    expect(isFavoriteContentType('resourcepack')).toBe(true)
    expect(isFavoriteContentType('datapack')).toBe(true)
    expect(isFavoriteContentType('shader')).toBe(true)
    expect(isFavoriteContentType('modpack')).toBe(false)
    expect(isFavoriteContentType('world')).toBe(false)
})
