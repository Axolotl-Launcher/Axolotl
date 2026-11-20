import { describe, expect, it } from 'vitest'

import {
    cloneNbtNode,
    coerceNbtValue,
    createNbtTag,
    defaultNbtValue,
    findNbtNode,
    findNbtNodePath,
    isNbtArray,
    isNbtContainer,
    moveNbtNode,
    nbtSiblings,
    removeNbtNode,
    unquoteSnbtText,
} from './model.ts'

function sampleTree() {
    const inner = createNbtTag('inner', 'int', '1')
    const first = createNbtTag('first', 'compound', '', [inner])
    const second = createNbtTag('second', 'string', 'x')
    const third = createNbtTag('third', 'byte', '1')
    return { nodes: [first, second, third], inner, first, second, third }
}

describe('NBT 节点模型', () => {
    it('按 id 查找节点与路径', () => {
        const { nodes, inner, second } = sampleTree()
        expect(findNbtNode(nodes, inner.id)).toBe(inner)
        expect(findNbtNodePath(nodes, inner.id)).toEqual([0, 0])
        expect(findNbtNodePath(nodes, second.id)).toEqual([1])
        expect(findNbtNodePath(nodes, 'missing')).toBeUndefined()
    })

    it('在所属同级数组里移动与删除', () => {
        const { nodes, inner, second, third } = sampleTree()
        expect(nbtSiblings(nodes, inner.id)).toBe(nodes[0].children)
        expect(moveNbtNode(nodes, inner.id, -1)).toBe(false)
        expect(moveNbtNode(nodes, third.id, -1)).toBe(true)
        expect(nodes.map((node) => node.name)).toEqual(['first', 'third', 'second'])
        expect(moveNbtNode(nodes, second.id, 1)).toBe(false)
        expect(removeNbtNode(nodes, third.id)).toBe(true)
        expect(removeNbtNode(nodes, 'missing')).toBe(false)
        expect(nodes.map((node) => node.name)).toEqual(['first', 'second'])
    })

    it('复制节点时分配新 id 并保留结构', () => {
        const { nodes, first } = sampleTree()
        const copy = cloneNbtNode(first)
        expect(copy.id).not.toBe(first.id)
        expect(copy.children?.[0].id).not.toBe(nodes[0].children?.[0].id)
        expect(copy.children?.[0].name).toBe('inner')
    })

    it('容器与数组判定', () => {
        expect(isNbtContainer({ type: 'compound' })).toBe(true)
        expect(isNbtContainer({ type: 'list' })).toBe(true)
        expect(isNbtContainer({ type: 'string' })).toBe(false)
        expect(isNbtArray({ type: 'long_array' })).toBe(true)
        expect(isNbtArray({ type: 'list' })).toBe(false)
    })

    it('每种标量类型都有可用的默认值', () => {
        expect(defaultNbtValue('byte')).toBe('0b')
        expect(defaultNbtValue('long')).toBe('0L')
        expect(defaultNbtValue('string')).toBe('""')
        expect(defaultNbtValue('compound')).toBe('')
    })

    it('改类型时把值清洗成裸值', () => {
        expect(coerceNbtValue('5b', 'int')).toBe('5')
        expect(coerceNbtValue('5', 'byte')).toBe('5')
        expect(coerceNbtValue('abc', 'int')).toBe('0')
        expect(coerceNbtValue('"text"', 'string')).toBe('text')
        expect(coerceNbtValue("'text'", 'string')).toBe('text')
        expect(coerceNbtValue('text', 'string')).toBe('text')
        expect(coerceNbtValue('1,2', 'int_array')).toBe('')
    })

    it('去掉用户连引号一起输入的字符串', () => {
        expect(unquoteSnbtText('"a\\"b"')).toBe('a"b')
        expect(unquoteSnbtText("'a'")).toBe('a')
        expect(unquoteSnbtText('a"b')).toBe('a"b')
        expect(unquoteSnbtText('"')).toBe('"')
    })
})
