import { expect, it } from 'vitest'

import { isModpackFileName } from './modpack-file.ts'

it('accepts modpack archives whatever case the shell reported', () => {
    expect(isModpackFileName(String.raw`C:\packs\Sky.mrpack`)).toBe(true)
    expect(isModpackFileName(String.raw`C:\packs\SKY.MRPACK`)).toBe(true)
    expect(isModpackFileName('/home/me/packs/Sky.Zip')).toBe(true)
    expect(isModpackFileName(String.raw`C:\packs\house.schem`)).toBe(false)
    expect(isModpackFileName(String.raw`C:\packs\notes.txt`)).toBe(false)
    expect(isModpackFileName('mrpack')).toBe(false)
})
