import { describe, expect, it } from 'vitest'

import { findFieldByPath } from './catalog.ts'
import {
    appendElement,
    appendField,
    canInsert,
    insertKindFor,
    listIsFull,
    renumberListElements,
} from './edit.ts'
import { nodeFromField, nodeFromForm } from './from-spec.ts'
import { createNbtTag } from './model.ts'
import { formFor, type NbtFieldSpec } from './schema.ts'
import { serializeNbtValue } from './snbt.ts'

const VERSION = '1.21'

function field(path: string): NbtFieldSpec {
    const found = findFieldByPath('item', path)
    if (!found) throw new Error(`missing field ${path}`)
    return found
}

describe('插入规则', () => {
    it('复合标签插字段，列表插元素，标量什么都不插', () => {
        expect(insertKindFor(createNbtTag('a', 'compound', ''))).toBe('field')
        expect(insertKindFor(createNbtTag('a', 'list', ''))).toBe('element')
        expect(insertKindFor(createNbtTag('a', 'string', ''))).toBe('none')
        expect(insertKindFor(createNbtTag('a', 'int_array', ''))).toBe('none')
    })

    it('同名子项替换而不是并列', () => {
        const parent = createNbtTag('p', 'compound', '', [createNbtTag('a', 'string', 'old')])
        appendField(parent, createNbtTag('a', 'string', 'new'))
        expect(parent.children).toHaveLength(1)
        expect(parent.children?.[0].value).toBe('new')
    })

    it('往复合标签里追加元素会被拒绝', () => {
        expect(appendElement(createNbtTag('p', 'compound', ''), { type: 'string' }, VERSION)).toBe(
            undefined,
        )
    })

    it('列表元素沿用已有类型，不写混型列表', () => {
        const list = createNbtTag('l', 'list', '', [createNbtTag('0', 'compound', '')])
        list.elementType = 'compound'
        // 目录给的是字符串写法，但列表里已经有复合标签，必须跟着现状走
        const added = appendElement(list, { type: 'string' }, VERSION)
        expect(added?.type).toBe('compound')
        expect(list.children?.map((child) => child.type)).toEqual(['compound', 'compound'])
    })

    it('空列表采用目录给的元素写法', () => {
        const list = createNbtTag('l', 'list', '')
        appendElement(list, { type: 'compound', fields: [] }, VERSION)
        expect(list.elementType).toBe('compound')
    })

    it('定长列表满了就插不进去', () => {
        const spec = findFieldByPath('entity', 'Pos')!
        const form = formFor(spec, VERSION)
        const list = nodeFromField(spec, VERSION)
        expect(list.children).toHaveLength(3)
        expect(listIsFull(list, form)).toBe(true)
        expect(canInsert(list, form)).toBe(false)
        expect(appendElement(list, form, VERSION)).toBeUndefined()
    })

    it('列表元素的名字只是序号，增删后重新编号', () => {
        const list = createNbtTag('l', 'list', '')
        appendElement(list, { type: 'string' }, VERSION)
        appendElement(list, { type: 'string' }, VERSION)
        list.children?.splice(0, 1)
        renumberListElements(list)
        expect(list.children?.map((child) => child.name)).toEqual(['0'])
    })
})

describe('按写法建节点', () => {
    it('banner_patterns 是列表，元素带 color 与 pattern', () => {
        const node = nodeFromField(field('banner_patterns'), VERSION)
        expect(node.type).toBe('list')
        expect(node.elementType).toBe('compound')

        const element = appendElement(node, formFor(field('banner_patterns'), VERSION), VERSION)
        expect(element?.children?.map((child) => `${child.name}=${child.value}`)).toEqual([
            'color=white',
            'pattern=minecraft:cross',
        ])
    })

    it('pattern 默认写成图案 ID，不是图案定义', () => {
        const node = nodeFromField(field('banner_patterns.pattern'), VERSION)
        expect(node.type).toBe('string')
        // 节点存裸值，引号由序列化补；存 `"minecraft:cross"` 会输出 `"\"minecraft:cross\""`
        expect(node.value).toBe('minecraft:cross')
    })

    it('另一种写法由界面显式选：图案定义复合标签', () => {
        const spec = field('banner_patterns.pattern')
        const compound = formFor(spec, VERSION, 'compound')
        expect(compound?.type).toBe('compound')
        const node = nodeFromForm('pattern', compound, VERSION)
        expect(node.children?.map((child) => child.name)).toEqual(['asset_id', 'translation_key'])
    })

    it('custom_name 是文本字符串，不是空复合标签', () => {
        const node = nodeFromField(field('custom_name'), VERSION)
        expect(node.type).toBe('string')
        expect(node.value).toBe('')
    })

    it('定长数组按长度预填', () => {
        const pos = nodeFromField(findFieldByPath('entity', 'Pos')!, VERSION)
        expect(pos.children).toHaveLength(3)
        expect(pos.children?.every((child) => child.type === 'double')).toBe(true)
    })

    it('不预填多层结构', () => {
        const node = nodeFromField(field('container'), VERSION)
        expect(node.type).toBe('list')
        expect(node.children).toEqual([])
    })

    /** 默认值在目录里是 SNBT 文本，节点存裸值：整条链路走完不能出现双层引号。 */
    it('新建的旗帜图案直接能写出合法 SNBT', () => {
        const spec = field('banner_patterns')
        const node = nodeFromField(spec, VERSION)
        appendElement(node, formFor(spec, VERSION), VERSION)
        expect(serializeNbtValue(node)).toBe('[{color:"white",pattern:"minecraft:cross"}]')

        const definition = nodeFromForm(
            'pattern',
            formFor(field('banner_patterns.pattern'), VERSION, 'compound'),
            VERSION,
        )
        expect(serializeNbtValue(definition)).toBe('{asset_id:"",translation_key:""}')
    })
})
