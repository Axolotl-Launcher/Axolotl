import { describe, expect, it } from 'vitest'

import {
    appendPathKey,
    formatNbtPath,
    parseNbtPath,
    pathCompletionPrefix,
    validateNbtPath,
} from './path.ts'

describe('NBT 路径解析', () => {
    it('逐层取子标签', () => {
        expect(parseNbtPath('Foo.Bar')).toEqual([
            { kind: 'key', name: 'Foo', quoted: false },
            { kind: 'key', name: 'Bar', quoted: false },
        ])
    })

    it('列表下标、通配与区间', () => {
        expect(parseNbtPath('Inventory[0]')).toEqual([
            { kind: 'key', name: 'Inventory', quoted: false },
            { kind: 'index', index: 0 },
        ])
        expect(parseNbtPath('Items[]')).toEqual([
            { kind: 'key', name: 'Items', quoted: false },
            { kind: 'all' },
        ])
        expect(parseNbtPath('Items[1..3]')).toEqual([
            { kind: 'key', name: 'Items', quoted: false },
            { kind: 'slice', from: 1, to: 3 },
        ])
        expect(parseNbtPath('Items[..3]')).toEqual([
            { kind: 'key', name: 'Items', quoted: false },
            { kind: 'slice', from: undefined, to: 3 },
        ])
    })

    it('复合标签筛选两种写法', () => {
        expect(parseNbtPath('Items[{id:"minecraft:stone"}]')[1]).toEqual({
            kind: 'filter',
            raw: '{id:"minecraft:stone"}',
        })
        expect(parseNbtPath('Items{id:"minecraft:stone"}')[1]).toEqual({
            kind: 'filter',
            raw: '{id:"minecraft:stone"}',
        })
    })

    it('带引号的键名', () => {
        expect(parseNbtPath('"foo.bar".baz')).toEqual([
            { kind: 'key', name: 'foo.bar', quoted: true },
            { kind: 'key', name: 'baz', quoted: false },
        ])
    })

    it('半截路径不抛异常', () => {
        // 空的半截方括号按「全部元素」读，比当作下标 0 更接近用户意图
        expect(parseNbtPath('Inventory[')).toEqual([
            { kind: 'key', name: 'Inventory', quoted: false },
            { kind: 'all' },
        ])
        expect(parseNbtPath('"unterminated')).toEqual([
            { kind: 'key', name: 'unterminated', quoted: true },
        ])
    })

    it('往返保持写法', () => {
        for (const path of ['Foo.Bar', 'Inventory[0]', 'Items[]', 'Items[1..3]', 'Items[..3]']) {
            expect(formatNbtPath(parseNbtPath(path))).toBe(path)
        }
        expect(formatNbtPath(parseNbtPath('"foo.bar".baz'))).toBe('"foo.bar".baz')
    })
})

describe('NBT 路径校验', () => {
    it('空路径合法', () => {
        expect(validateNbtPath('')).toEqual([])
    })

    it('括号与开头', () => {
        expect(validateNbtPath('.Foo').some((issue) => issue.level === 'error')).toBe(true)
        expect(validateNbtPath('Items[0').some((issue) => issue.level === 'error')).toBe(true)
        expect(validateNbtPath('Items]').some((issue) => issue.level === 'error')).toBe(true)
        expect(validateNbtPath('Items[0]')).toEqual([])
    })

    it('下标必须是整数', () => {
        expect(validateNbtPath('Items[a]').some((issue) => issue.level === 'error')).toBe(true)
        expect(validateNbtPath('Items[1.5]').some((issue) => issue.level === 'error')).toBe(true)
    })

    it('筛选要写成 key:value', () => {
        expect(validateNbtPath('Items[{id}]').some((issue) => issue.level === 'warning')).toBe(true)
        expect(validateNbtPath('Items[{id:"stone"}]')).toEqual([])
    })
})

describe('路径补全', () => {
    it('取最后一段作为前缀', () => {
        expect(pathCompletionPrefix('Inv')).toEqual({ prefix: 'Inv', depth: 1 })
        expect(pathCompletionPrefix('Inventory.')).toEqual({ prefix: '', depth: 2 })
        expect(pathCompletionPrefix('Items[0].')).toEqual({ prefix: '', depth: 2 })
    })

    it('补全接在末尾', () => {
        expect(appendPathKey('', 'Health')).toBe('Health')
        expect(appendPathKey('Foo.', 'Bar')).toBe('Foo.Bar')
        expect(appendPathKey('Foo', 'Bar')).toBe('Foo.Bar')
    })
})
