import { unzipSync } from 'fflate'
import { expect, it } from 'vitest'

import {
    createDatapackBlob,
    createDatapackDescription,
    createDatapackFileName,
    createDatapackFiles,
    createPackMcmeta,
} from './datapack.ts'

it('creates pack.mcmeta with legacy and ranged pack formats', () => {
    expect(JSON.parse(createPackMcmeta(48)).pack.pack_format).toBe(48)
    expect(JSON.parse(createPackMcmeta([107, 1])).pack).toEqual({
        description: 'Axolotl Recipe Generator',
        min_format: [107, 1],
        max_format: [107, 1],
    })
})

it('creates a datapack file name with a timestamp', () => {
    expect(createDatapackFileName('1.21', new Date('2026-01-02T03:04:05'))).toBe(
        'axolotl-recipes-1.21-20260102-030405.zip',
    )
})

it('creates a description listing recipe product names in brackets', () => {
    expect(createDatapackDescription(['橡木活板门', '闪长岩台阶'])).toBe(
        'Axolotl Recipe Generator\n[橡木活板门] [闪长岩台阶]',
    )
})

it('builds datapack files with versioned recipe and tag directories', async () => {
    const files = createDatapackFiles(
        '1.21',
        [{ name: 'iron_bars', json: { type: 'minecraft:crafting_shaped' } }],
        [
            {
                namespace: 'crafting',
                id: 'my_tag',
                values: ['minecraft:oak_planks', '#minecraft:planks'],
            },
        ],
    )
    expect(files.map((file) => file.path)).toEqual([
        'pack.mcmeta',
        'pack.png',
        'data/crafting/recipe/iron_bars.json',
        'data/crafting/tags/item/my_tag.json',
    ])
    const blob = createDatapackBlob(files)
    const archive = unzipSync(new Uint8Array(await blob.arrayBuffer()))
    expect(archive['pack.png']?.length).toBeTruthy()
    expect(
        JSON.parse(new TextDecoder().decode(archive['data/crafting/recipe/iron_bars.json'])),
    ).toEqual({
        type: 'minecraft:crafting_shaped',
    })
    expect(
        JSON.parse(new TextDecoder().decode(archive['data/crafting/tags/item/my_tag.json'])),
    ).toEqual({ replace: false, values: ['minecraft:oak_planks', '#minecraft:planks'] })
})

it('uses recipes directory before 1.21', () => {
    const files = createDatapackFiles(
        '1.20',
        [{ name: 'iron_bars', json: { type: 'minecraft:crafting_shaped' } }],
        [],
    )
    expect(files.some((file) => file.path === 'data/crafting/recipes/iron_bars.json')).toBeTruthy()
})

it('rejects datapack export for 1.12', () => {
    expect(() => createDatapackFiles('1.12', [], [])).toThrow()
})
