import { expect, it } from 'vitest'

import { contentFavoriteKey, isFavoriteContentType } from './content-favorites.ts'

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
