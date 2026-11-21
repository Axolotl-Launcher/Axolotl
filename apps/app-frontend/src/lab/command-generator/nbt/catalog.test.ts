import { describe, expect, it } from 'vitest'

import {
    availableFieldsFor,
    catalogFor,
    fieldsFor,
    findFieldByPath,
    levelAtPath,
    rootFieldsFor,
} from './catalog.ts'
import { groupSpec } from './groups.ts'
import {
    CATEGORY_ORDER,
    childrenOf,
    formsFor,
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

/**
 * wiki 自己在这些条目上写的是 `[more information needed]`，没有可引用的说明。
 * wiki 补上之后从集合里删掉对应键，测试就会重新要求 desc。
 */
const UNDOCUMENTED = new Set([
    'current_impulse_context_reset_grace_time',
    'item_based_steering',
    'spawn_extra_particles_on_fall',
])

/**
 * wiki 明确列出子字段、因而必须在目录里也展开的组件。没有子字段时，面板里选中
 * `minecraft:food` 只能得到一个空 `{}`，用户仍要去 wiki 抄 `nutrition` 怎么拼。
 */
const EXPECTED_STRUCTURED = [
    'banner_patterns',
    'bees',
    'can_break',
    'can_place_on',
    'consumable',
    'container',
    'custom_model_data',
    'damage_resistant',
    'enchantable',
    'equippable',
    'food',
    'pot_decorations',
    'profile',
    'suspicious_stew_effects',
    'tool',
    'tooltip_display',
    'trim',
    'use_cooldown',
    'written_book_content',
]

/**
 * 实体 / 方块实体里在 wiki 上有明确子结构的复合字段。缺了子字段时，面板只能给一个
 * 空 `{}`，用户既不知道有哪些子项，也不知道哪些子项在什么情况下生效。
 */
const EXPECTED_STRUCTURED_BY_DOMAIN: [NbtDomain, string[]][] = [
    [
        'entity',
        [
            'Passengers',
            'Inventory',
            'EnderItems',
            'Items',
            'Item',
            'item',
            'attributes',
            'active_effects',
            'custom_effects',
            'stew_effects',
            'Gossips',
            'Offers',
            'SpawnData',
            'SpawnPotentials',
            'recipeBook',
            'equipment',
            'ShoulderEntityLeft',
            'ShoulderEntityRight',
            'anger',
            'Brain',
            'listener',
            'block_state',
            'BlockState',
        ],
    ],
    [
        'block_entity',
        [
            'Items',
            'item',
            'Book',
            'RecordItem',
            'front_text',
            'back_text',
            'patterns',
            'bees',
            'cursors',
            // `lock` 的值是物品谓词，wiki 把它放在 `conditions/item` 模板里，
            // 那份模板没抓进缓存；宁可空着，也不要把方块谓词的键名套上去。
            'loot_tables_to_eject',
            'spawn_data',
            'SpawnData',
            'SpawnPotentials',
            'blockState',
            'entity',
            'equipment',
            'listener',
        ],
    ],
]

/** 一个字段所有写法里能填的子字段。 */
function childFields(field: NbtFieldSpec): NbtFieldSpec[] {
    const seen = new Set<string>()
    const out: NbtFieldSpec[] = []
    for (const form of field.forms) {
        for (const child of [...(form.fields ?? []), ...(form.element?.fields ?? [])]) {
            if (seen.has(child.key)) continue
            seen.add(child.key)
            out.push(child)
        }
    }
    return out
}

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
        walk(childFields(field), visit, path, rootScope)
    }
}

describe('NBT 目录数据', () => {
    it.each(DOMAINS)('%s 目录良构', (domain) => {
        const catalog = catalogFor(domain)
        // 载入时归一化成 forms，所以运行时一律是 schemaVersion 2
        expect(catalog.schemaVersion).toBe(2)
        expect(catalog.domain).toBe(domain)
        expect(catalog.sources.length).toBeGreaterThan(0)
        expect(catalog.fields.length).toBeGreaterThan(0)

        const seen = new Set<string>()
        walk(catalog.fields, (field, path, rootScope) => {
            expect(field.key.length, `${domain}:${path} 缺少键名`).toBeGreaterThan(0)
            expect(field.forms.length, `${domain}:${path} 没有任何写法`).toBeGreaterThan(0)
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

            for (const form of field.forms) {
                expect(TYPES.has(form.type), `${domain}:${path} 写法类型非法`).toBe(true)
                if (form.default !== undefined) {
                    const parsed = parseNbtTag(form.default)
                    expect(parsed.type, `${domain}:${path} 默认值类型不符`).toBe(form.type)
                }
                if (form.type === 'list') {
                    expect(form.element, `${domain}:${path} 是列表但缺元素写法`).toBeDefined()
                }
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

    it.each(EXPECTED_STRUCTURED)('物品组件 %s 展开子字段', (key) => {
        const field = fieldsFor('item').find((entry) => entry.key === key)
        expect(field, `缺少组件 ${key}`).toBeDefined()
        expect(childFields(field!).length, `${key} 没有子字段`).toBeGreaterThan(0)
    })

    /**
     * 这些字段在 wiki 里都有明确的子结构（物品栈、实体、方块状态、告示牌文本……），
     * 只给一个空 `{}` 等于让用户自己去 wiki 抄键名。
     */
    it.each(EXPECTED_STRUCTURED_BY_DOMAIN)('%s 的关键复合字段展开子字段', (domain, keys) => {
        for (const key of keys) {
            const matches = fieldsFor(domain).filter((field) => field.key === key)
            expect(matches.length, `缺少字段 ${key}`).toBeGreaterThan(0)
            for (const field of matches) {
                expect(childFields(field).length, `${domain}:${key} 没有子字段`).toBeGreaterThan(0)
            }
        }
    })

    /**
     * 子字段是「这个母项里面能填什么」。标量列表 / 数组没有键名可填，
     * 给它挂子字段会让面板在 `Rotation` 里列出 `0`、`1` 这种不存在的标签。
     */
    it('子字段只挂在复合写法或列表元素上', () => {
        for (const domain of DOMAINS) {
            walk(catalogFor(domain).fields, (field, path) => {
                for (const form of field.forms) {
                    if (form.fields) {
                        expect(form.type, `${domain}:${path} 的标量写法挂了子字段`).toBe('compound')
                    }
                    if (form.type === 'list' && form.element?.fields) {
                        expect(
                            form.element.type,
                            `${domain}:${path} 的列表元素不是复合标签却挂了子字段`,
                        ).toBe('compound')
                    }
                }
            })
        }
    })

    /**
     * 同名不同义的子项必须跟着母项走：旗帜方块实体的 `patterns` 是 `{color, pattern}`，
     * 不是物品组件那边 `banner_patterns` 用的图案定义对象（`asset_id` / `translation_key`）。
     */
    it.each([
        ['block_entity', 'patterns', ['color', 'pattern']],
        ['entity', 'stew_effects', ['id', 'duration']],
        ['entity', 'SpawnPotentials', ['weight', 'data']],
        ['block_entity', 'SpawnPotentials', ['weight', 'data']],
    ] as const)('%s 的 %s 用自己母项的子项', (domain, key, expected) => {
        const field = fieldsFor(domain).find((entry) => entry.key === key)
        expect(field, `缺少字段 ${key}`).toBeDefined()
        expect(childFields(field!).map((child) => child.key)).toEqual([...expected])
    })

    /**
     * 子项自己还能是复合标签。旗帜图案的 `pattern` 既可以只写图案 ID，也可以写成
     * `{asset_id, translation_key}`：两种写法都要在目录里，否则面板只能造出一种，
     * 用户既写不出 `pattern:"minecraft:creeper"`，也写不出图案定义。
     */
    it.each([
        ['block_entity', 'patterns.pattern', ['asset_id', 'translation_key']],
        ['item', 'banner_patterns.pattern', ['asset_id', 'translation_key']],
        [
            'block_entity',
            'profile',
            ['name', 'id', 'properties', 'texture', 'cape', 'elytra', 'model'],
        ],
        ['entity', 'profile', ['name', 'id', 'properties', 'texture', 'cape', 'elytra', 'model']],
        ['entity', 'potion_contents', ['potion', 'custom_color', 'custom_name', 'custom_effects']],
        ['entity', 'TileEntityData', ['id', 'keepPacked', 'x', 'y', 'z', 'components']],
        ['item', 'container.item', ['id', 'count', 'components']],
        ['item', 'pot_decorations.left', ['id', 'count', 'components']],
        ['item', 'written_book_content.title', ['raw', 'filtered']],
    ] as const)('%s 的 %s 还能再往下展开', (domain, path, expected) => {
        const field = findFieldByPath(domain, path)
        expect(field, `缺少字段 ${path}`).toBeDefined()
        expect(childFields(field!).map((child) => child.key)).toEqual([...expected])
    })

    /** 试炼刷怪笼的配置里还套着生成候选，候选里又套着实体。 */
    it('刷怪配置一层层展开到实体', () => {
        for (const domain of ['entity', 'block_entity'] as const) {
            const data = findFieldByPath(domain, 'SpawnPotentials.data')
            expect(data, `${domain} 缺少 SpawnPotentials.data`).toBeDefined()
            expect(childFields(data!).map((child) => child.key)).toEqual([
                'entity',
                'custom_spawn_rules',
                'equipment',
            ])
        }
        const config = findFieldByPath('block_entity', 'normal_config')
        expect(childFields(config!).map((child) => child.key)).toContain('spawn_potentials')
    })

    /**
     * 同一个结构散落在很多地方（每个实体、每个刷怪配置都有一份），只要有一处
     * 写岔了，用户在不同入口看到的字段就对不上。
     */
    it('槽位掉落概率在各处结构一致', () => {
        const SLOTS = ['feet', 'legs', 'chest', 'head', 'body', 'mainhand', 'offhand']
        let count = 0
        for (const domain of DOMAINS) {
            walk(catalogFor(domain).fields, (field, path) => {
                if (field.key !== 'slot_drop_chances') return
                count++
                expect(field.forms[0].type, `${domain}:${path} 应该是复合标签`).toBe('compound')
                expect(
                    childFields(field).map((child) => child.key),
                    `${domain}:${path} 的槽位`,
                ).toEqual(SLOTS)
            })
        }
        expect(count).toBeGreaterThan(5)
    })

    it('物品栈在各处结构一致', () => {
        const STACK = ['id', 'count', 'components']
        const holders: string[] = []
        walk(catalogFor('item').fields, (field, path) => {
            const keys = childFields(field).map((child) => child.key)
            if (keys.length !== STACK.length || !STACK.every((key) => keys.includes(key))) return
            holders.push(path)
            expect(keys, `item:${path} 的物品栈结构不对`).toEqual(STACK)
        })
        expect(holders.length).toBeGreaterThanOrEqual(8)
    })

    it('levelAtPath 按母项取子字段', () => {
        const banner = levelAtPath(
            { domain: 'block_entity', version: '1.21', targetId: 'banner' },
            ['patterns'],
        )
        expect(banner?.fields.map((field) => field.key)).toEqual(['color', 'pattern'])
        // 列表：元素写法要一起给出来，面板据此只提供「添加元素」
        expect(banner?.element?.type).toBe('compound')

        // 根层给的是这一域的全部字段
        expect(
            levelAtPath({ domain: 'entity', version: '1.21', targetId: 'zombie' }, [])?.fields
                .length,
        ).toBeGreaterThan(100)
        // 走到一半断链：不要退回根字段，否则等于把母项属性塞进子层
        expect(levelAtPath({ domain: 'entity', version: '1.21' }, ['nope'])).toBeUndefined()
        expect(levelAtPath({ domain: 'entity', version: '1.21' }, ['Pos', 'nope'])).toBeUndefined()
    })

    /**
     * 1.20.5 之前物品的 NBT 不是组件列表，而是 `{id, Count, tag}` 里的 `tag`。
     * 面板在旧版模式下必须列 `tag` 的子键（`display`、`Enchantments`…），
     * 否则用户会把 `custom_name` 这类组件写进旧版标签里。
     */
    it('旧版物品的根字段是 tag 的子键', () => {
        const legacy = rootFieldsFor({ domain: 'item', version: '1.20.4', components: false })
        expect(legacy.map((field) => field.key)).toContain('display')
        expect(legacy.map((field) => field.key)).toContain('Enchantments')
        expect(legacy.map((field) => field.key)).not.toContain('custom_name')

        const modern = rootFieldsFor({ domain: 'item', version: '1.21', components: true })
        expect(modern.map((field) => field.key)).toContain('custom_name')
        expect(modern.map((field) => field.key)).not.toContain('display')
    })

    /** 同一字段的两种写法各有自己的版本区间：26.1 才有「图案 ID 列表」。 */
    it('写法按版本筛选', () => {
        const field = findFieldByPath('item', 'provides_banner_patterns')
        expect(field, '缺少 provides_banner_patterns').toBeDefined()
        expect(formsFor(field!, '1.21').map((form) => form.type)).toEqual(['string'])
        expect(formsFor(field!, '26.1').map((form) => form.type)).toEqual(['string', 'list'])
    })

    /** 图案与染料的取值是有限集合，界面要给下拉框而不是自由输入。 */
    it('旗帜图案与染料颜色带枚举来源', () => {
        const pattern = findFieldByPath('item', 'banner_patterns.pattern')
        expect(pattern, '缺少 banner_patterns.pattern').toBeDefined()
        const primary = pattern!.forms[0]
        expect(primary.type, 'pattern 默认写法应该是图案 ID 字符串').toBe('string')
        expect(primary.values).toBe('banner_pattern')

        const color = findFieldByPath('item', 'banner_patterns.color')
        expect(color?.forms[0].values).toBe('dye_color')
        expect(color?.forms[0].default).toBe('"white"')
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
        const form = field!.forms[0]
        expect(form.type, `${key} 的标签类型`).toBe(type)
        expect(form.element?.type, `${key} 的元素类型`).toBe(elementType)
        expect(form.length?.exactly, `${key} 的长度`).toBe(length)
    })

    it('带长度的字段必须是数组', () => {
        for (const domain of DOMAINS) {
            walk(catalogFor(domain).fields, (field, path) => {
                for (const form of field.forms) {
                    if (form.length?.exactly === undefined) continue
                    expect(ARRAY_TYPES.has(form.type), `${domain}:${path} 有长度但不是数组`).toBe(
                        true,
                    )
                    if (form.type !== 'list') continue
                    expect(form.element, `${domain}:${path} 是列表但缺元素写法`).toBeDefined()
                }
            })
        }
    })

    /**
     * 说明文字是面板唯一的引导来源：字段名大多是从 wiki 直译的 PascalCase 或
     * snake_case，用户光看键名猜不出含义。这里挡两类历史问题——照抄示例命令
     * （`Example: /give @s diamond`）和截断到 108 字符（结尾一个省略号）。
     */
    it('每个字段都有可用的说明文字', () => {
        for (const domain of DOMAINS) {
            walk(catalogFor(domain).fields, (field, path) => {
                if (UNDOCUMENTED.has(field.key)) return
                const desc = (field.desc ?? '').trim()
                expect(desc.length, `${domain}:${path} 缺少说明`).toBeGreaterThan(0)
                expect(/^Example/i.test(desc), `${domain}:${path} 的说明是示例命令`).toBe(false)
                expect(desc.endsWith('…'), `${domain}:${path} 的说明被截断`).toBe(false)
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
                for (const form of field.forms) {
                    if (!form.editor) continue
                    if (KNOWN_PENDING.has(form.editor)) continue
                    if (!groupSpec(form.editor)) missing.add(`${domain}:${path} → ${form.editor}`)
                }
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
            category: 'inventory',
            since: '1.20.5',
            forms: [{ type: 'compound' }],
        }
        expect(isVersionInRange(spec, '1.20.4')).toBe(false)
        expect(isVersionInRange(spec, '26.1')).toBe(true)
        expect(isVersionInRange({ ...spec, until: '1.21' }, '26.1')).toBe(false)
    })

    it('旧版本键名按 until 回退', () => {
        const spec: NbtFieldSpec = {
            key: 'active_effects',
            category: 'effect',
            legacyKeys: [{ key: 'ActiveEffects', until: '1.20.1' }],
            forms: [{ type: 'list', element: { type: 'compound' } }],
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

    it('childrenOf 取写法里的子字段', () => {
        const field = findFieldByPath('item', 'banner_patterns')
        expect(childrenOf(field?.forms[0]).map((child) => child.key)).toEqual(['color', 'pattern'])
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
        expect(childFields(legacy.find((field) => field.key === 'tag')!).length).toBeGreaterThan(20)

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
