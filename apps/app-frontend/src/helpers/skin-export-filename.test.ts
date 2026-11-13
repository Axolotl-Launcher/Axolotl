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

it('removes control characters and trailing dots or spaces', () => {
    expect(skinExportFilename({ name: 'bad\u0000name', texture_key: 'abc' })).toBe('badname.png')
    expect(skinExportFilename({ name: 'name. ', texture_key: 'abc' })).toBe('name.png')
    expect(skinExportFilename({ name: '\u0001', texture_key: 'abc' })).toBe('skin.png')
})

it('escapes names that Windows reserves for devices', () => {
    expect(skinExportFilename({ name: 'CON', texture_key: 'abc' })).toBe('_CON.png')
    expect(skinExportFilename({ name: 'lpt1.png', texture_key: 'abc' })).toBe('_lpt1.png')
    expect(skinExportFilename({ name: 'console', texture_key: 'abc' })).toBe('console.png')
})

it.each(['COM¹', 'com².png', 'LPT³.PNG', 'lpt¹.extra'])(
    'escapes Windows superscript device name %s',
    (name) => {
        const expected = name.toLowerCase().endsWith('.png') ? `_${name}` : `_${name}.png`
        expect(skinExportFilename({ name, texture_key: 'abc' })).toBe(expected)
    },
)

it.each(['COM⁴', 'COM10', 'lpt0', 'COM¹-skin'])(
    'preserves non-reserved device-like name %s',
    (name) => {
        expect(skinExportFilename({ name, texture_key: 'abc' })).toBe(`${name}.png`)
    },
)
