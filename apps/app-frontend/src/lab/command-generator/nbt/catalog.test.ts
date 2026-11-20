import { describe, expect, it } from 'vitest'

import { availableFieldsFor, catalogFor, fieldsFor, findFieldByPath } from './catalog.ts'
import { groupSpec } from './groups.ts'
import {
    CATEGORY_ORDER,
    humanizeKey,
    isVersionInRange,
    keyForVersion,
    type NbtDomain,
    type NbtFieldSpec,
    searchFields,
} from './schema.ts'
import { parseNbtTag } from './snbt.ts'

const DOMAINS: NbtDomain[] = ['entity', 'item', 'block_entity']
const TYPES = new Set([
    'byte',
    'short',
    'int',
    'long',
    'float',
    'double',
    'string',
    'compound',
    'list',
    'byte_array',
    'int_array',
    'long_array',
])
const ARRAY_TYPES = new Set(['list', 'byte_array', 'int_array', 'long_array'])

function walk(
    fields: NbtFieldSpec[],
    visit: (field: NbtFieldSpec, path: string, root: string) => void,
    prefix = '',
    root = '',
) {
    for (const field of fields) {
        const path = prefix ? `${prefix}.${field.key}` : field.key
        const rootScope = root || JSON.stringify(field.appliesTo ?? {})
        visit(field, path, rootScope)
        walk(field.children ?? [], visit, path, rootScope)
    }
}

describe('NBT 目录数据', () => {
    it.each(DOMAINS)('%s 目录良构', (domain) => {
        const catalog = catalogFor(domain)
        expect(catalog.schemaVersion).toBe(1)
        expect(catalog.domain).toBe(domain)
        expect(catalog.sources.length).toBeGreaterThan(0)
        expect(catalog.fields.length).toBeGreaterThan(0)

        const seen = new Set<string>()
        walk(catalog.fields, (field, path, rootScope) => {
            expect(field.key.length, `${domain}:${path} 缺少键名`).toBeGreaterThan(0)
            expect(TYPES.has(field.type), `${domain}:${path} 类型非法`).toBe(true)
            expect(CATEGORY_ORDER.includes(field.category), `${domain}:${path} 分类非法`).toBe(true)
            if (field.since && field.until) {
                expect(field.since <= field.until, `${domain}:${path} 版本区间倒置`).toBe(true)
            }
            if (field.appliesTo?.group) {
                expect(
                    Object.keys(catalog.groups ?? {}),
                    `${domain}:${path} 引用了不存在的分组 ${field.appliesTo.group}`,
                ).toContain(field.appliesTo.group)
            }
            const identity = `${path}|${rootScope}`
            expect(seen.has(identity), `${domain}:${path} 重复`).toBe(false)
            seen.add(identity)

            if (field.default !== undefined) {
                const parsed = parseNbtTag(field.default)
                expect(parsed.type, `${domain}:${path} 默认值类型不符`).toBe(field.type)
            }
        })
    })

    it('实体目录包含通用字段与实体专属字段', () => {
        for (const key of ['Air', 'CustomName', 'NoGravity', 'Glowing', 'active_effects']) {
            expect(
                fieldsFor('entity').some((field) => field.key === key),
                `缺少通用字段 ${key}`,
            ).toBe(true)
        }
        expect(
            fieldsFor('entity').some(
                (field) => field.key === 'IsBaby' && field.appliesTo?.ids?.includes('zombie'),
            ),
            '缺少僵尸专属字段 IsBaby',
        ).toBe(true)
        expect(
            fieldsFor('entity').some(
                (field) => field.key === 'Item' && field.appliesTo?.ids?.includes('item'),
            ),
            '缺少掉落物专属字段 Item',
        ).toBe(true)
    })

    it('物品目录覆盖常用组件', () => {
        for (const key of ['custom_name', 'lore', 'enchantments', 'unbreakable', 'custom_data']) {
            expect(
                fieldsFor('item').some((field) => field.key === key),
                `缺少组件 ${key}`,
            ).toBe(true)
        }
        expect(findFieldByPath('item', 'custom_name')).toBeDefined()
    })

    it('方块实体目录包含公共字段', () => {
        const keys = fieldsFor('block_entity').map((field) => field.key)
        expect(keys.length).toBeGreaterThan(20)
        expect(keys).toContain('id')
    })

    /**
     * 这几个字段在 wiki 里是定长数组（`Entity format`：Pos / Motion 是 3 个 double，
     * Rotation 是 2 个 float，UUID 是 4 个 int）。曾经都标成标量，从面板插入会得到
     * 一个数字框，写不出 `[0d,64d,0d]` 这种结构。
     */
    it.each([
        { key: 'Pos', type: 'list', elementType: 'double', length: 3 },
        { key: 'Motion', type: 'list', elementType: 'double', length: 3 },
        { key: 'Rotation', type: 'list', elementType: 'float', length: 2 },
        { key: 'UUID', type: 'int_array', elementType: undefined, length: 4 },
    ])('$key 是定长数组', ({ key, type, elementType, length }) => {
        const field = fieldsFor('entity').find((entry) => entry.key === key)
        expect(field, `缺少字段 ${key}`).toBeDefined()
        expect(field!.type, `${key} 的标签类型`).toBe(type)
        expect(field!.elementType, `${key} 的元素类型`).toBe(elementType)
        expect(field!.length, `${key} 的长度`).toBe(length)
    })

    it('带长度的字段必须是数组', () => {
        for (const domain of DOMAINS) {
            walk(catalogFor(domain).fields, (field, path) => {
                if (field.length === undefined) return
                expect(ARRAY_TYPES.has(field.type), `${domain}:${path} 有长度但不是数组`).toBe(true)
                if (field.type !== 'list') return
                expect(field.elementType, `${domain}:${path} 是列表但缺元素类型`).toBeDefined()
            })
        }
    })

    /**
     * 数据里的 `editor` 必须真的存在，否则字段会静默退回裸标签树——用户看不出
     * "这里本来有表格"，只会觉得引导不够。`KNOWN_PENDING` 是已经声明、还没实现的
     * 编辑器：实现一个就从这里删一条，测试因此不会长期容忍缺口。
     */
    it('每个 editor 都有对应实现', () => {
        const KNOWN_PENDING = new Set(['block_predicates', 'written_book'])
        const missing = new Set<string>()
        for (const domain of DOMAINS) {
            walk(catalogFor(domain).fields, (field, path) => {
                if (!field.editor) return
                if (KNOWN_PENDING.has(field.editor)) return
                if (!groupSpec(field.editor)) missing.add(`${domain}:${path} → ${field.editor}`)
            })
        }
        expect([...missing]).toEqual([])
    })
})

describe('NBT 目录查询', () => {
    it('按实体过滤时保留通用字段、丢掉无关字段', () => {
        const zombie = availableFieldsFor({ domain: 'entity', version: '26.3', targetId: 'zombie' })
        const keys = zombie.map((field) => field.key)
        expect(keys).toContain('NoGravity')
        expect(keys).toContain('IsBaby')

        const item = availableFieldsFor({ domain: 'entity', version: '26.3', targetId: 'item' })
        const itemKeys = item.map((field) => field.key)
        expect(itemKeys).toContain('NoGravity')
        expect(itemKeys).not.toContain('IsBaby')
    })

    it('分组把船的变体都算进去', () => {
        const chestBoat = availableFieldsFor({
            domain: 'entity',
            version: '26.3',
            targetId: 'oak_chest_boat',
        })
        expect(chestBoat.map((field) => field.key)).toContain('Items')

        const plainBoat = availableFieldsFor({
            domain: 'entity',
            version: '26.3',
            targetId: 'oak_boat',
        })
        expect(plainBoat.map((field) => field.key)).not.toContain('Items')
    })

    it('搜索命中键名与可读名', () => {
        const fields = fieldsFor('entity')
        expect(searchFields(fields, 'nograv', '26.3')[0].spec.key).toBe('NoGravity')
        expect(searchFields(fields, '发光', '26.3').length).toBeGreaterThan(0)
        expect(searchFields(fields, 'zzzz', '26.3')).toEqual([])
    })

    it('版本门控按数字比较，26.1 高于 1.21', () => {
        const spec: NbtFieldSpec = {
            key: 'enchantments',
            type: 'compound',
            category: 'inventory',
            since: '1.20.5',
        }
        expect(isVersionInRange(spec, '1.20.4')).toBe(false)
        expect(isVersionInRange(spec, '26.1')).toBe(true)
        expect(isVersionInRange({ ...spec, until: '1.21' }, '26.1')).toBe(false)
    })

    it('旧版本键名按 until 回退', () => {
        const spec: NbtFieldSpec = {
            key: 'active_effects',
            type: 'list',
            category: 'effect',
            legacyKeys: [{ key: 'ActiveEffects', until: '1.20.1' }],
        }
        expect(keyForVersion(spec, '1.19')).toBe('ActiveEffects')
        expect(keyForVersion(spec, '1.20.2')).toBe('active_effects')
        expect(keyForVersion(spec, '26.3')).toBe('active_effects')
    })

    it('把键名转成可读标签', () => {
        expect(humanizeKey('fall_distance')).toBe('Fall distance')
        expect(humanizeKey('CustomNameVisible')).toBe('Custom name visible')
        expect(humanizeKey('UUID')).toBe('UUID')
    })
})

describe('版本差异', () => {
    it('玩家专属字段只出现在玩家上', () => {
        const player = availableFieldsFor({ domain: 'entity', version: '26.3', targetId: 'player' })
        expect(player.map((field) => field.key)).toContain('EnderItems')
        expect(player.map((field) => field.key)).toContain('XpLevel')

        const zombie = availableFieldsFor({ domain: 'entity', version: '26.3', targetId: 'zombie' })
        expect(zombie.map((field) => field.key)).not.toContain('EnderItems')
    })

    it('旧版物品 tag 在 1.20.4 之后不再出现', () => {
        const legacy = availableFieldsFor({ domain: 'item', version: '1.20.4' })
        expect(legacy.map((field) => field.key)).toContain('tag')
        expect(legacy.find((field) => field.key === 'tag')?.children?.length).toBeGreaterThan(20)

        const modern = availableFieldsFor({ domain: 'item', version: '1.20.5' })
        expect(modern.map((field) => field.key)).not.toContain('tag')
        expect(modern.map((field) => field.key)).toContain('custom_name')
    })

    it('状态效果在 1.20.2 前后用不同的键名', () => {
        const spec = fieldsFor('entity').find((field) => field.key === 'active_effects')!
        expect(keyForVersion(spec, '1.19.4')).toBe('ActiveEffects')
        expect(keyForVersion(spec, '1.20.2')).toBe('active_effects')
    })

    it('26.x 新增字段在老版本里不出现', () => {
        const spec = fieldsFor('entity').find((field) => field.key === 'invulnerable_time')!
        expect(isVersionInRange(spec, '26.2')).toBe(false)
        expect(isVersionInRange(spec, '26.3')).toBe(true)
    })
})
