import { describe, expect, it } from 'vitest'

import {
    emptyGroupRow,
    groupShapeOf,
    groupSpec,
    NBT_GROUPS,
    readGroupRows,
    writeGroupRows,
} from './groups.ts'
import { createNbtTag, type NbtTagNode } from './model.ts'

const enchantments = NBT_GROUPS.enchantments
const effects = NBT_GROUPS.active_effects
const lore = NBT_GROUPS.lore

describe('结构化组定义', () => {
    it('每列都有中文标签与默认值', () => {
        for (const spec of Object.values(NBT_GROUPS)) {
            expect(spec.columns.length).toBeGreaterThan(0)
            for (const column of spec.columns) {
                expect(column.zh.length).toBeGreaterThan(0)
                expect(column.default).toBeTypeOf('string')
            }
        }
    })

    it('按 id 取到定义', () => {
        expect(groupSpec('enchantments')).toBe(enchantments)
        expect(groupSpec('nope')).toBeUndefined()
        expect(groupSpec(undefined)).toBeUndefined()
    })
})

describe('附魔组', () => {
    it('1.20.5+ 的映射结构往返', () => {
        const node = createNbtTag('enchantments', 'compound', '', [
            createNbtTag('minecraft:sharpness', 'int', '5'),
            createNbtTag('minecraft:unbreaking', 'int', '3'),
        ])
        expect(readGroupRows(node, enchantments)).toEqual([
            { id: 'minecraft:sharpness', level: '5' },
            { id: 'minecraft:unbreaking', level: '3' },
        ])

        writeGroupRows(node, enchantments, [{ id: 'minecraft:mending', level: '1' }])
        expect(node.type).toBe('compound')
        expect(node.children?.map((child) => `${child.name}=${child.value}`)).toEqual([
            'minecraft:mending=1',
        ])
    })

    it('旧版的列表结构用 lvl 字段', () => {
        const node = createNbtTag('Enchantments', 'list', '', [
            createNbtTag('0', 'compound', '', [
                createNbtTag('id', 'string', 'minecraft:sharpness'),
                createNbtTag('lvl', 'short', '5s'),
            ]),
        ])
        expect(readGroupRows(node, enchantments)).toEqual([
            { id: 'minecraft:sharpness', level: '5' },
        ])

        writeGroupRows(node, enchantments, [{ id: 'minecraft:looting', level: '2' }])
        expect(node.type).toBe('list')
        const first = node.children?.[0]
        expect(first?.children?.map((child) => `${child.name}=${child.value}`)).toEqual([
            'id=minecraft:looting',
            'lvl=2',
        ])
    })

    it('结构不认识时返回空行', () => {
        expect(readGroupRows(createNbtTag('enchantments', 'string', '"x"'), enchantments)).toEqual(
            [],
        )
    })
})

describe('状态效果组', () => {
    it('读取与写回', () => {
        const node = createNbtTag('active_effects', 'list', '', [
            createNbtTag('0', 'compound', '', [
                createNbtTag('id', 'string', 'minecraft:speed'),
                createNbtTag('amplifier', 'byte', '1b'),
                createNbtTag('duration', 'int', '200'),
            ]),
        ])
        const rows = readGroupRows(node, effects)
        expect(rows[0].id).toBe('minecraft:speed')
        expect(rows[0].amplifier).toBe('1')
        expect(rows[0].duration).toBe('200')

        writeGroupRows(node, effects, rows)
        const first = node.children?.[0]
        expect(first?.children?.map((child) => child.name)).toEqual([
            'id',
            'amplifier',
            'duration',
            'ambient',
            'show_particles',
            'show_icon',
        ])
        expect(first?.children?.find((child) => child.name === 'amplifier')?.value).toBe('1')
        expect(first?.children?.find((child) => child.name === 'show_icon')?.value).toBe('1b')
    })
})

describe('描述文本组', () => {
    it('裸字符串与文本组件都能读', () => {
        const node = createNbtTag('lore', 'list', '', [
            createNbtTag('0', 'string', '"第一行"'),
            createNbtTag('1', 'compound', '', [createNbtTag('text', 'string', '"第二行"')]),
        ])
        expect(readGroupRows(node, lore)).toEqual([{ text: '第一行' }, { text: '第二行' }])
    })

    it('写回成文本组件', () => {
        const node = createNbtTag('lore', 'list', '')
        writeGroupRows(node, lore, [{ text: '一行' }])
        expect(node.elementType).toBe('string')
        expect(node.children?.[0].value).toBe('{"text":"一行"}')
    })
})

describe('结构判定', () => {
    it('空节点按定义里的形状创建', () => {
        const node: NbtTagNode = createNbtTag('enchantments', 'compound', '')
        expect(groupShapeOf(node)).toBe('map')
        writeGroupRows(node, enchantments, [emptyGroupRow(enchantments)])
        expect(node.children?.[0].name).toBe('minecraft:sharpness')
    })
})
