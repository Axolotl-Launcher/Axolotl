import { NbtTag } from 'deepslate/nbt'
import { describe, expect, it } from 'vitest'

import { createNbtTag } from './model.ts'
import {
    formatSnbtKey,
    listElementType,
    parseNbt,
    parseNbtTag,
    quoteSnbt,
    serializeNbt,
    serializeNbtDocument,
    serializeNbtValue,
} from './snbt.ts'

function roundTrip(input: string): string {
    const parsed = parseNbt(input)
    return serializeNbtDocument(parsed.nodes, parsed.components)
}

describe('SNBT 序列化', () => {
    it('只在必要时给键加引号', () => {
        expect(formatSnbtKey('NoGravity')).toBe('NoGravity')
        expect(formatSnbtKey('fall_distance')).toBe('fall_distance')
        // 冒号不在裸写字符集里，命名空间键必须加引号
        expect(formatSnbtKey('minecraft:sharpness')).toBe('"minecraft:sharpness"')
        expect(formatSnbtKey('a b')).toBe('"a b"')
        expect(formatSnbtKey('')).toBe('""')
        expect(formatSnbtKey('quote"inside')).toBe('"quote\\"inside"')
    })

    it('字符串值始终加引号并转义', () => {
        expect(quoteSnbt('Sword')).toBe('"Sword"')
        expect(quoteSnbt('a"b\\c')).toBe('"a\\"b\\\\c"')
        expect(serializeNbtValue(createNbtTag('x', 'string', 'Sword'))).toBe('"Sword"')
    })

    it('给数字标签补上正确后缀，int 不带后缀', () => {
        expect(serializeNbtValue(createNbtTag('a', 'byte', '1'))).toBe('1b')
        expect(serializeNbtValue(createNbtTag('a', 'short', '1'))).toBe('1s')
        expect(serializeNbtValue(createNbtTag('a', 'int', '1'))).toBe('1')
        expect(serializeNbtValue(createNbtTag('a', 'long', '1'))).toBe('1L')
        expect(serializeNbtValue(createNbtTag('a', 'float', '1.5'))).toBe('1.5f')
        expect(serializeNbtValue(createNbtTag('a', 'double', '1.5'))).toBe('1.5d')
    })

    it('三种整型数组按 [B;]/[I;]/[L;] 写出', () => {
        expect(serializeNbtValue(createNbtTag('a', 'byte_array', '1,2'))).toBe('[B;1b,2b]')
        expect(serializeNbtValue(createNbtTag('a', 'int_array', '1,2'))).toBe('[I;1,2]')
        expect(serializeNbtValue(createNbtTag('a', 'long_array', '1,2'))).toBe('[L;1L,2L]')
        expect(serializeNbtValue(createNbtTag('a', 'byte_array', ''))).toBe('[B;]')
    })

    it('保留超出安全整数范围的长整型', () => {
        const input =
            '{big:9223372036854775807L,arr:[L;9223372036854775807L,-9223372036854775808L]}'
        expect(roundTrip(input)).toBe(input)
    })
})

describe('SNBT 解析', () => {
    it('区分列表与整型数组', () => {
        const list = parseNbt('{a:[1,2,3]}').nodes[0]
        expect(list.type).toBe('list')
        expect(list.elementType).toBe('int')

        const array = parseNbt('{a:[I;1,2,3]}').nodes[0]
        expect(array.type).toBe('int_array')
        expect(array.value).toBe('1,2,3')
    })

    it('记下列表元素类型，空列表也有明确类型', () => {
        expect(listElementType(parseNbt('{a:[{b:1b}]}').nodes[0])).toBe('compound')
        const empty = parseNbt('{a:[]}').nodes[0]
        expect(empty.type).toBe('list')
        expect(empty.children).toEqual([])
        expect(listElementType(empty)).toBeUndefined()
    })

    it('保留嵌套复合标签与组件语法', () => {
        const compound = parseNbt('{display:{Name:\'{"text":"Sword"}\'}}')
        expect(compound.components).toBe(false)
        expect(compound.nodes[0].children?.[0].value).toBe('{"text":"Sword"}')

        const components = parseNbt('[custom_name=\'{"text":"Sword"}\',max_damage=10]')
        expect(components.components).toBe(true)
        expect(components.nodes.map((node) => node.type)).toEqual(['string', 'int'])
    })

    it('可以解析单个非复合标签', () => {
        expect(parseNbtTag('42').type).toBe('int')
        expect(parseNbtTag('[1,2]').type).toBe('list')
        expect(() => parseNbtTag('{a:1b} trailing')).toThrow()
    })

    it('拒绝畸形输入', () => {
        for (const input of ['{Health:', '{Health:20f} trailing', '[custom_name=]', '[=1]'])
            expect(() => parseNbt(input)).toThrow()
        expect(parseNbt('')).toEqual({ nodes: [], components: false })
    })

    it('往返后与原标签语义相同', () => {
        const input =
            '{Health:20f,Items:[{id:"minecraft:stone",Count:1b}],UUID:[I;1,2,3,4],bytes:[B;1b,2b],"quoted\\"key":"text",empty:{},list:[]}'
        expect(NbtTag.fromString(input).equals(NbtTag.fromString(roundTrip(input)))).toBe(true)
    })

    it('序列化空文档为 {}', () => {
        expect(serializeNbt([])).toBe('{}')
        expect(serializeNbtDocument([], true)).toBe('[]')
    })
})
