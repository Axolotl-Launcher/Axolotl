import { describe, expect, it } from 'vitest'

import { createNbtTag } from './model.ts'
import {
    readTextComponent,
    textComponentJson,
    textComponentNodes,
    textComponentSnbt,
} from './text-component.ts'

describe('文本组件读取', () => {
    it('JSON 字符串形式取出文本并保留其余字段', () => {
        const node = createNbtTag('Name', 'string', '{"text":"钻石剑","color":"gold","bold":true}')
        const { text, nodes } = readTextComponent(node)
        expect(text).toBe('钻石剑')
        expect(nodes.map((child) => child.name)).toEqual(['text', 'color', 'bold'])
    })

    it('内联复合标签形式', () => {
        const node = createNbtTag('custom_name', 'compound', '', [
            createNbtTag('text', 'string', '铁镐'),
            createNbtTag('italic', 'byte', '1'),
        ])
        const { text, nodes } = readTextComponent(node)
        expect(text).toBe('铁镐')
        expect(nodes.map((child) => child.name)).toEqual(['text', 'italic'])
    })

    it('不是 JSON 就当作纯文本', () => {
        expect(readTextComponent(createNbtTag('Name', 'string', 'Legacy Name')).text).toBe(
            'Legacy Name',
        )
    })

    it('没有节点时是空文本', () => {
        expect(readTextComponent(undefined)).toEqual({ text: '', nodes: [] })
    })
})

describe('文本组件写回', () => {
    it('只替换 text，颜色与样式保持不变', () => {
        const node = createNbtTag('Name', 'string', '{"text":"旧","color":"gold"}')
        const { nodes } = readTextComponent(node)
        const next = textComponentNodes(nodes, '新')

        expect(textComponentJson(next)).toBe('{"text":"新","color":"gold"}')
        expect(textComponentSnbt(next)).toBe('{text:"新",color:"gold"}')
    })

    it('没有原始格式时只写 text', () => {
        expect(textComponentJson(textComponentNodes([], '名字'))).toBe('{"text":"名字"}')
        expect(textComponentSnbt(textComponentNodes([], '名字'))).toBe('{text:"名字"}')
    })

    it('清空文本时不留下空组件', () => {
        expect(textComponentNodes([], '')).toEqual([])
    })

    it('布尔字段还原成 JSON 布尔值', () => {
        const nodes = [
            createNbtTag('text', 'string', 'x'),
            createNbtTag('bold', 'byte', '1'),
            createNbtTag('underlined', 'byte', '0'),
        ]
        expect(textComponentJson(nodes)).toBe('{"text":"x","bold":true,"underlined":false}')
    })

    it('数组按元素类型写成整型数组或列表', () => {
        const nodes = [
            createNbtTag('text', 'string', 'x'),
            createNbtTag('shadow_color', 'int_array', '1,2,3,4'),
        ]
        expect(textComponentJson(nodes)).toBe('{"text":"x","shadow_color":[1,2,3,4]}')
        expect(textComponentSnbt(nodes)).toBe('{text:"x",shadow_color:[I;1,2,3,4]}')
    })
})

describe('两种形式互转', () => {
    it('JSON 里的 extra 能变成内联列表再读回来', () => {
        const node = createNbtTag(
            'Name',
            'string',
            '{"text":"主","color":"gold","extra":[{"text":"从","color":"gray"}]}',
        )
        const { text, nodes } = readTextComponent(node)
        expect(text).toBe('主')

        const snbt = textComponentSnbt(nodes)
        expect(snbt).toContain('extra:[{text:"从",color:"gray"}]')
        expect(textComponentJson(nodes)).toBe(
            '{"text":"主","color":"gold","extra":[{"text":"从","color":"gray"}]}',
        )
    })
})
