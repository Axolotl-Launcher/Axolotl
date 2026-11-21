import { describe, expect, it } from 'vitest'

import {
    ATTRIBUTE_SLOTS,
    emptyGroupRow,
    EQUIPMENT_SLOTS,
    groupShapeOf,
    groupSpec,
    NBT_GROUPS,
    readGroupHeader,
    readGroupRows,
    shapeMatches,
    writeGroupHeader,
    writeGroupRows,
} from './groups.ts'
import { createNbtTag, type NbtTagNode } from './model.ts'

const enchantments = NBT_GROUPS.enchantments
const effects = NBT_GROUPS.active_effects
const lore = NBT_GROUPS.lore
const itemList = NBT_GROUPS.item_list
const equipment = NBT_GROUPS.equipment
const attributeModifiers = NBT_GROUPS.attribute_modifiers
const fireworks = NBT_GROUPS.fireworks
const potionContents = NBT_GROUPS.potion_contents

/** 断言一组子标签的 `名字=值`，顺带固定字段顺序。 */
function pairs(node: NbtTagNode | undefined): string[] {
    return (node?.children ?? []).map((child) => `${child.name}=${child.value}`)
}

describe('结构化组定义', () => {
    it('每列都有 i18n 标签与默认值', () => {
        for (const spec of Object.values(NBT_GROUPS)) {
            expect(spec.columns.length).toBeGreaterThan(0)
            for (const column of [...spec.columns, ...(spec.header ?? [])]) {
                expect(
                    column.label.id.startsWith('app.lab.command-generator.nbt.group.column.'),
                ).toBe(true)
                expect(column.label.defaultMessage.length).toBeGreaterThan(0)
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
        // 布尔存裸值，后缀由 SNBT 序列化补，写成 `1b` 会输出 `1bb`
        expect(first?.children?.find((child) => child.name === 'show_icon')?.value).toBe('1')
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

    it('复合标签、列表以外的节点不算结构化', () => {
        expect(groupShapeOf(createNbtTag('a', 'string', 'x'))).toBeUndefined()
    })

    /**
     * 表格只能套在结构相符的节点上：用列表表格写回复合标签，会把
     * `{levels:…,show_in_tooltip:…}` 整体改成列表，等于替用户改写标签。
     */
    it('形状不符时不套表格', () => {
        expect(shapeMatches(createNbtTag('e', 'compound', ''), enchantments)).toBe(true)
        expect(shapeMatches(createNbtTag('e', 'list', ''), enchantments)).toBe(false)
        expect(shapeMatches(createNbtTag('e', 'compound', ''), attributeModifiers)).toBe(false)
        expect(shapeMatches(createNbtTag('e', 'list', ''), attributeModifiers)).toBe(true)
        // listKey 结构：行在子键里，节点本身必须是复合标签
        expect(shapeMatches(createNbtTag('e', 'compound', ''), potionContents)).toBe(true)
        expect(shapeMatches(createNbtTag('e', 'list', ''), potionContents)).toBe(false)
    })
})

describe('写回时保留表格没建模的键', () => {
    it('attribute_modifiers 每条记录里的 display 原样留下', () => {
        const node = createNbtTag('attribute_modifiers', 'list', '', [
            createNbtTag('0', 'compound', '', [
                createNbtTag('type', 'string', 'minecraft:scale'),
                createNbtTag('amount', 'double', '1'),
                createNbtTag('display', 'compound', '', [createNbtTag('type', 'string', 'hidden')]),
            ]),
        ])
        writeGroupRows(node, attributeModifiers, [
            {
                type: 'minecraft:scale',
                amount: '2',
                operation: 'add_value',
                slot: 'any',
                id: '',
            },
        ])
        const row = node.children?.[0]
        expect(row?.children?.map((child) => child.name)).toContain('display')
        expect(row?.children?.find((child) => child.name === 'amount')?.value).toBe('2')
    })

    /**
     * 1.20.5–1.21.4 的附魔是 `{levels:…,show_in_tooltip:…}`。没建模这两种包装时
     * 也不能把它们当成附魔行，更不能写回时抹掉。
     */
    it('附魔映射里的 levels / show_in_tooltip 既不当行也不被抹掉', () => {
        const node = createNbtTag('enchantments', 'compound', '', [
            createNbtTag('levels', 'compound', '', [
                createNbtTag('minecraft:sharpness', 'int', '3'),
            ]),
            createNbtTag('show_in_tooltip', 'byte', '1'),
        ])
        expect(readGroupRows(node, enchantments)).toEqual([])

        writeGroupRows(node, enchantments, [{ id: 'minecraft:mending', level: '1' }])
        expect(node.children?.map((child) => child.name)).toEqual([
            'minecraft:mending',
            'levels',
            'show_in_tooltip',
        ])
    })
})

describe('物品列表组', () => {
    it('读出现代结构', () => {
        const node = createNbtTag('Items', 'list', '', [
            createNbtTag('0', 'compound', '', [
                createNbtTag('Slot', 'byte', '0'),
                createNbtTag('id', 'string', 'minecraft:diamond'),
                createNbtTag('count', 'int', '3'),
            ]),
        ])
        expect(readGroupRows(node, itemList)).toEqual([
            { Slot: '0', id: 'minecraft:diamond', count: '3' },
        ])
    })

    it('沿用旧版的 Count 键名与 byte 类型', () => {
        const node = createNbtTag('Items', 'list', '', [
            createNbtTag('0', 'compound', '', [
                createNbtTag('Slot', 'byte', '1'),
                createNbtTag('id', 'string', 'minecraft:stone'),
                createNbtTag('Count', 'byte', '2'),
            ]),
        ])
        expect(readGroupRows(node, itemList)[0].count).toBe('2')

        writeGroupRows(node, itemList, [{ Slot: '1', id: 'minecraft:stone', count: '2' }])
        expect(pairs(node.children?.[0])).toEqual(['Slot=1', 'id=minecraft:stone', 'Count=2'])
        // 旧版 Count 是 byte，不能顺手改成 int
        expect(node.children?.[0].children?.[2].type).toBe('byte')
    })

    it('新节点用现代键名', () => {
        const node = createNbtTag('Items', 'list', '')
        writeGroupRows(node, itemList, [emptyGroupRow(itemList)])
        expect(pairs(node.children?.[0])).toEqual(['Slot=0', 'id=minecraft:stone', 'count=1'])
    })
})

describe('装备组', () => {
    it('槽位名是键，值是物品栈', () => {
        const node = createNbtTag('equipment', 'compound', '', [
            createNbtTag('mainhand', 'compound', '', [
                createNbtTag('id', 'string', 'minecraft:iron_sword'),
                createNbtTag('count', 'int', '1'),
            ]),
            createNbtTag('head', 'compound', '', [
                createNbtTag('id', 'string', 'minecraft:iron_helmet'),
            ]),
        ])
        expect(readGroupRows(node, equipment)).toEqual([
            { slot: 'mainhand', id: 'minecraft:iron_sword', count: '1' },
            { slot: 'head', id: 'minecraft:iron_helmet', count: '1' },
        ])

        writeGroupRows(node, equipment, [{ slot: 'feet', id: 'minecraft:gold_boots', count: '1' }])
        expect(node.type).toBe('compound')
        expect(node.children?.[0].name).toBe('feet')
        expect(pairs(node.children?.[0])).toEqual(['id=minecraft:gold_boots', 'count=1'])
    })

    it('槽位集合与 wiki 一致', () => {
        expect(EQUIPMENT_SLOTS).toEqual([
            'mainhand',
            'offhand',
            'head',
            'chest',
            'legs',
            'feet',
            'body',
            'saddle',
        ])
    })
})

describe('属性修饰符组', () => {
    it('可选的 id 留空时不写这条键', () => {
        const node = createNbtTag('attribute_modifiers', 'list', '')
        writeGroupRows(node, attributeModifiers, [
            {
                type: 'minecraft:scale',
                amount: '4',
                operation: 'add_multiplied_base',
                slot: 'hand',
                id: '',
            },
        ])
        expect(pairs(node.children?.[0])).toEqual([
            'type=minecraft:scale',
            'amount=4',
            'operation=add_multiplied_base',
            'slot=hand',
        ])
        expect(node.children?.[0].children?.[1].type).toBe('double')
    })

    it('写了 id 就带上', () => {
        const node = createNbtTag('attribute_modifiers', 'list', '')
        writeGroupRows(node, attributeModifiers, [
            {
                type: 'minecraft:scale',
                amount: '2',
                operation: 'add_value',
                slot: 'any',
                id: 'example:grow',
            },
        ])
        expect(node.children?.[0].children?.at(-1)?.name).toBe('id')
        expect(node.children?.[0].children?.at(-1)?.value).toBe('example:grow')
        expect(ATTRIBUTE_SLOTS).toContain('saddle')
    })
})

describe('烟花组', () => {
    it('飞行时长在父标签上，爆炸效果逐行编辑', () => {
        const node = createNbtTag('fireworks', 'compound', '')
        writeGroupHeader(node, fireworks, { flight_duration: '2' })
        writeGroupRows(node, fireworks, [
            { shape: 'star', colors: '255,0,0', fade_colors: '', has_trail: '1', has_twinkle: '0' },
        ])

        expect(pairs(node)).toEqual(['flight_duration=2', 'explosions='])
        const explosion = node.children?.[1].children?.[0]
        expect(pairs(explosion)).toEqual([
            'shape=star',
            'colors=',
            'fade_colors=',
            'has_trail=1',
            'has_twinkle=0',
        ])
        expect(explosion?.children?.[1].type).toBe('list')
        expect(explosion?.children?.[1].elementType).toBe('int')

        expect(readGroupHeader(node, fireworks)).toEqual({ flight_duration: '2' })
        expect(readGroupRows(node, fireworks)[0]).toMatchObject({
            shape: 'star',
            colors: '255,0,0',
            has_trail: '1',
        })
    })

    it('没有 explosions 时读出空行', () => {
        expect(readGroupRows(createNbtTag('fireworks', 'compound', ''), fireworks)).toEqual([])
    })
})

describe('药水内容组', () => {
    it('种类可省略，自定义效果复用状态效果的列', () => {
        const node = createNbtTag('potion_contents', 'compound', '')
        writeGroupHeader(node, potionContents, { potion: '', custom_color: '' })
        writeGroupRows(node, potionContents, [
            {
                id: 'minecraft:poison',
                amplifier: '0',
                duration: '100',
                ambient: '0',
                show_particles: '1',
                show_icon: '1',
            },
        ])

        // 两个头部字段都是可选键，留空就整条省略
        expect(node.children?.map((child) => child.name)).toEqual(['custom_effects'])
        expect(readGroupRows(node, potionContents)[0].id).toBe('minecraft:poison')
    })

    it('写成字符串时不做结构化编辑', () => {
        const node = createNbtTag('potion_contents', 'string', 'minecraft:swiftness')
        expect(readGroupRows(node, potionContents)).toEqual([])
        expect(groupShapeOf(node)).toBeUndefined()
    })
})
