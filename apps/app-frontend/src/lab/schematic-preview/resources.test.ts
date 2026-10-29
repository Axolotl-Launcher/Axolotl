import { CanvasTexture, LinearFilter, NearestFilter } from 'three'
import { expect, it } from 'vitest'

import {
    configureSchematicTexture,
    minecraftVersionFromDataVersion,
    resolveSchematicBlockName,
    resolveSchematicMaterialTexture,
} from './resources.ts'

it('block names follow the current locale and fall back without exposing internal IDs', () => {
    const names = {
        en_us: {
            'block.minecraft.stone': 'Stone',
            'block.example.machine.frame': 'Machine Frame',
        },
        zh_cn: { 'block.minecraft.stone': '\u77f3\u5934' },
    }

    expect(resolveSchematicBlockName('minecraft:stone', names, 'zh-CN')).toBe('\u77f3\u5934')
    expect(resolveSchematicBlockName('example:machine/frame', names, 'zh-CN')).toBe('Machine Frame')
    expect(resolveSchematicBlockName('example:polished_tile', names, 'zh-CN')).toBe('Polished Tile')
})

it('schematic textures preserve Deepslate atlas row coordinates', () => {
    const texture = new CanvasTexture({} as HTMLCanvasElement)
    texture.flipY = true
    texture.magFilter = LinearFilter
    texture.minFilter = LinearFilter
    texture.generateMipmaps = true
    const previousVersion = texture.version

    configureSchematicTexture(texture)

    expect(texture.flipY).toBe(false)
    expect(texture.magFilter).toBe(NearestFilter)
    expect(texture.minFilter).toBe(NearestFilter)
    expect(texture.generateMipmaps).toBe(false)
    expect(texture.version > previousVersion).toBeTruthy()
})

it('material previews resolve blockstate models and texture variables', () => {
    const sideUv: [number, number, number, number] = [0.25, 0.5, 0.5, 0.75]
    const resources = {
        blockDefinitions: {
            'minecraft:grass_block': {
                variants: { 'snowy=false': [{ model: 'minecraft:block/grass_block' }] },
            },
        },
        blockModels: {
            'minecraft:block/grass_block': {
                parent: 'minecraft:block/cube_bottom_top',
                textures: {
                    side: '#side_texture',
                    side_texture: 'minecraft:block/grass_block_side',
                    top: 'minecraft:block/grass_block_top',
                },
            },
            'minecraft:block/cube_bottom_top': {
                textures: { particle: 'minecraft:block/dirt' },
            },
        },
        defaultBlockProperties: {},
        textureUvs: {
            'minecraft:block/grass_block_side': sideUv,
            'minecraft:block/grass_block_top': [0, 0, 0.25, 0.25] as [
                number,
                number,
                number,
                number,
            ],
            'minecraft:block/dirt': [0.5, 0.5, 0.75, 0.75] as [number, number, number, number],
        },
        missingTextureUv: [0, 0, 0.1, 0.1] as [number, number, number, number],
    }

    expect(resolveSchematicMaterialTexture('minecraft:grass_block', resources)).toEqual(sideUv)
})

it('material previews fall back to conventional texture names', () => {
    const uv: [number, number, number, number] = [0, 0.25, 0.25, 0.5]
    const resources = {
        blockDefinitions: {},
        blockModels: {},
        defaultBlockProperties: {},
        textureUvs: { 'example:block/polished_tile': uv },
        missingTextureUv: [0, 0, 0.1, 0.1] as [number, number, number, number],
    }

    expect(resolveSchematicMaterialTexture('example:polished_tile', resources)).toEqual(uv)
})

it('26.3 DataVersion is recognized without promoting snapshot or future worlds', () => {
    expect(minecraftVersionFromDataVersion(5023)).toBe('26.3')
    expect(minecraftVersionFromDataVersion(4903)).toBe('26.2')
    expect(minecraftVersionFromDataVersion(5005)).toBe(undefined)
    expect(minecraftVersionFromDataVersion(9999)).toBe(undefined)
})
