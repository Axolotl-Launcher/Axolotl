import { NbtTag } from 'deepslate/nbt'
import { describe, expect, it } from 'vitest'

import { createNbtTag, parseNbt, serializeNbt, serializeNbtDocument } from './nbt-tree.ts'

describe('NBT tree round trips', () => {
    it('preserves nested tags, numeric types, typed arrays, long precision and escaped keys', () => {
        const input =
            '{Health:20f,Items:[{id:"minecraft:stone",Count:1b}],big:9223372036854775807L,UUID:[I;1,2,3,4],bytes:[B;1b,2b],longs:[L;9223372036854775807L],"quoted\\"key":"text"}'
        const parsed = parseNbt(input)
        expect(parsed.components).toBe(false)
        const output = serializeNbtDocument(parsed.nodes, parsed.components)
        expect(NbtTag.fromString(input).equals(NbtTag.fromString(output))).toBeTruthy()
    })

    it('round trips item data components without wrapping them in a string tag', () => {
        const input =
            '[custom_name=\'{"text":"Sword"}\',minecraft:enchantments={"minecraft:sharpness":5},lore=[\'{"text":"Lore"}\']]'
        const parsed = parseNbt(input)
        expect(parsed.components).toBe(true)
        const reparsed = parseNbt(serializeNbtDocument(parsed.nodes, parsed.components))
        expect(reparsed.nodes.map(({ name, type, value }) => ({ name, type, value }))).toEqual(
            parsed.nodes.map(({ name, type, value }) => ({ name, type, value })),
        )
        expect(parsed.nodes[0].value).toBe('{"text":"Sword"}')
    })

    it('rejects malformed documents and trailing content without inventing a value field', () => {
        for (const input of ['{Health:', '{Health:20f} trailing', '[custom_name=]', '[=1]'])
            expect(() => parseNbt(input)).toThrow()
        expect(parseNbt('')).toEqual({ nodes: [], components: false })
    })

    it('escapes both tag keys and string values', () => {
        const output = serializeNbt([createNbtTag('key"\\', 'string', 'value"\\')])
        const parsed = NbtTag.fromString(output)
        expect(parsed.isCompound()).toBeTruthy()
        expect(parsed.get('key"\\')?.getAsString()).toBe('value"\\')
    })
})
