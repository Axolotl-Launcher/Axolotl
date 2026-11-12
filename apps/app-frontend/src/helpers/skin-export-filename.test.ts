import { expect, it } from 'vitest'

import { skinExportFilename } from './skins.ts'

it('names an exported skin after the skin and adds the PNG extension', () => {
    expect(skinExportFilename({ name: 'My Skin', texture_key: 'abc' })).toBe('My Skin.png')
    expect(skinExportFilename({ name: 'Skin.PNG', texture_key: 'abc' })).toBe('Skin.PNG')
})

it('falls back to the texture key and then to a generic name', () => {
    expect(skinExportFilename({ name: '  ', texture_key: 'abc' })).toBe('abc.png')
    expect(skinExportFilename(null)).toBe('skin.png')
})

it('replaces characters that a file name cannot contain', () => {
    expect(skinExportFilename({ name: 'a/b:c*?"<>|d', texture_key: 'abc' })).toBe('a-b-c-d.png')
})
