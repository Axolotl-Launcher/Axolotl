import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { IntlMessageFormat } from 'intl-messageformat'

import en from '../../locales/en-US/index.json' with { type: 'json' }
import zh from '../../locales/zh-CN/index.json' with { type: 'json' }
import type { SeedMapStructureDetails, SeedMapStructurePiece } from './advanced.ts'
import { createSeedMapLabels, seedMapEffectDuration } from './structure-labels.ts'

type Variant = NonNullable<SeedMapStructureDetails['variant']>
const variant = (values: Partial<Variant> = {}): Variant => ({
    rotation: 90,
    mirror: 0,
    start: 1,
    biome: 1,
    abandoned: false,
    giant: false,
    underground: false,
    airPocket: false,
    basement: false,
    cracked: false,
    size: 0,
    offset: { x: 0, y: 0, z: 0 },
    size3d: { x: 0, y: 0, z: 0 },
    ...values,
})
function labels(locale = 'en-US') {
    const catalog: Record<string, { message: string }> = locale === 'zh-CN' ? zh : en
    return createSeedMapLabels((message, values) => {
        assert.ok(message.id && catalog[message.id], `Missing ${locale} message ${message.id}`)
        return String(new IntlMessageFormat(catalog[message.id].message, locale).format(values))
    }, locale)
}
function source(path: string) {
    return readFileSync(new URL(`../../../../app/vendor/cubiomes/${path}`, import.meta.url), 'utf8')
}

test('all generated stronghold, fortress and end-city pieces have bilingual names', () => {
    for (const [kind, file] of [
        ['stronghold', 'stronghold'],
        ['fortress', 'fortress'],
        ['end-city', 'end_city'],
    ] as const) {
        const names = [...source(`features/${file}.c`).matchAll(/"([A-Za-z0-9_]+)"\s*\}/g)].map(
            (match) => match[1],
        )
        assert.ok(names.length >= 12)
        for (const locale of ['en-US', 'zh-CN'])
            for (const name of names) {
                const result = labels(locale).piece(kind, name)
                assert.notEqual(
                    result,
                    labels(locale).piece(kind, 'not_a_piece'),
                    `${locale}: ${name}`,
                )
                assert.notEqual(result, name)
            }
    }
})

test('every current camp template and all shipwreck shapes have display names', () => {
    const camps = [
        ...source('features/abandoned_camp.c').matchAll(/(?:name\s*=\s*)"([^"]+)"/g),
    ].map((match) => match[1])
    const ships = [...source('finders.c').matchAll(/\{"(shipwreck\/[^"\s]+)"/g)].map(
        (match) => match[1],
    )
    assert.ok(camps.length > 100)
    assert.equal(ships.length, 20)
    for (const locale of ['en-US', 'zh-CN']) {
        const names = labels(locale)
        const unknown = names.piece('abandoned-camp', 'unknown')
        for (const name of camps) {
            assert.notEqual(
                names.piece('abandoned-camp', name, variant({ biome: 188 })),
                unknown,
                name,
            )
            assert.ok(!names.piece('abandoned-camp', name, variant()).includes('%s'))
        }
        for (const name of ships) assert.notEqual(names.piece('shipwreck', name), unknown, name)
    }
})

test('legacy codes and remaining supported template names are translated', () => {
    for (const locale of ['en-US', 'zh-CN'])
        for (const name of [
            'TeDP',
            'TeJP',
            'TeSH',
            'BTP',
            'RUPO',
            'igloo/top',
            'igloo/middle',
            'igloo/bottom',
            'pillager_outpost/watchtower',
            'bastion/units/walls/wall_base',
            'bastion/hoglin_stable/ramparts/ramparts_3',
            'bastion/treasure/ramparts/mid_wall_main',
            'bastion/bridge/starting_pieces/entrance',
        ])
            assert.notEqual(
                labels(locale).piece('bastion', name),
                labels(locale).piece('bastion', 'unknown'),
                name,
            )
    assert.equal(labels('zh-CN').piece('stronghold', 'SHPR'), '末地传送门房间')
    assert.equal(labels('zh-CN').piece('fortress', 'NeMT'), '烈焰人刷怪平台')
})

test('start variants follow structure-specific indices, biome and giant flag', () => {
    const names = labels()
    for (const biome of [1, 177, 2, 35, 5, 12]) {
        const min = biome === 1 || biome === 177 ? 0 : 1
        const max = biome === 35 ? 4 : biome === 5 ? 2 : 3
        for (let start = min; start <= max; start++)
            assert.ok(names.variant('village', variant({ biome, start }), [])?.id)
    }
    assert.equal(
        names.variant('ruined-portal', variant({ start: 3, giant: true }), [])?.id,
        'giant_ruined_portal_3',
    )
    assert.equal(names.variant('ruined-portal', variant({ start: 10 }), [])?.id, 'ruined_portal_10')
    assert.equal(names.variant('trial-chambers', variant({ start: 0 }), [])?.id, 'corridor/end_1')
    assert.equal(names.variant('nether-fossil', variant({ start: 13 }), [])?.id, 'fossil_14')
    assert.equal(names.variant('ancient-city', variant({ start: 3 }), [])?.id, 'city_center_3')
    assert.equal(names.variant('bastion', variant({ start: 0 }), [])?.name, 'Housing units')
    assert.equal(names.variant('igloo', variant({ start: 255 }), []), undefined)
    assert.equal(names.variant('village', variant({ start: 255 }), []), undefined)
    assert.equal(names.variant('ruined-portal', variant({ start: 99 }), []), undefined)
    assert.equal(names.variant('swamp-hut', variant({ start: 0 }), []), undefined)
})

test('shipwreck start indices use the beached pool instead of the ocean pool', () => {
    const names = labels()
    assert.equal(
        names.variant('shipwreck', variant({ biome: 0, start: 1 }), [])?.id,
        'shipwreck/upsidedown_full',
    )
    assert.equal(
        names.variant('shipwreck', variant({ biome: 16, start: 1 }), [])?.id,
        'shipwreck/sideways_full',
    )
    const pieces = [{ name: 'shipwreck/with_mast_degraded' }] as SeedMapStructurePiece[]
    assert.equal(
        names.variant('shipwreck', variant({ start: 1 }), pieces)?.name,
        'Shipwreck with mast (degraded)',
    )
})

test('camp placeholders resolve only with a known biome and raw IDs remain distinct', () => {
    const names = labels('zh-CN')
    const raw = 'abandoned_camp/tent/%s/tent_%s_7'
    assert.equal(
        names.pieceId(raw, variant({ biome: 188 })),
        'abandoned_camp/tent/dappled_forest/tent_dappled_forest_7',
    )
    assert.equal(names.pieceId(raw, variant({ biome: -1 })), undefined)
    assert.equal(names.pieceId('future/%s/unknown', variant()), undefined)
    assert.equal(names.pieceId('SHPR'), 'SHPR')
    assert.ok(names.piece('abandoned-camp', raw, variant({ biome: 188 })).endsWith('帐篷 7'))
    assert.equal(names.piece('end-city', 'future/room'), '结构部件')
})

test('every emitted container table has a bilingual display name', () => {
    const text =
        source('finders.c') + source('features/stronghold.c') + source('features/abandoned_camp.c')
    const tables = [
        ...new Set(
            [...text.matchAll(/"((?:chests|barrels)\/[a-z0-9_]+)"/g)].map((match) => match[1]),
        ),
    ]
    for (const locale of ['en-US', 'zh-CN']) {
        const names = labels(locale)
        for (const table of tables)
            assert.notEqual(names.table(table), names.table('unknown/table'), table)
    }
})

test('vanilla loot names translate enchantments, levels and effects with safe unknown fallbacks', () => {
    const names = labels('zh-CN')
    assert.equal(names.item('minecraft:diamond'), '钻石')
    assert.equal(names.enchantment('minecraft:fortune'), '时运')
    assert.equal(names.level(3), 'III')
    assert.equal(names.level(99), '99')
    assert.equal(names.effect('minecraft:regeneration'), '生命恢复')
    assert.equal(names.item('custom:unknown'), '未知物品')
    assert.equal(names.enchantment('custom:fortune'), '未知附魔')
    assert.equal(names.effect('minecraft:not_known'), '未知状态效果')
    const effects = [
        ...source('loot/loot_functions.c')
            .split('const struct Potion')[0]
            .matchAll(/"(minecraft:[a-z_]+)"/g),
    ].map((match) => match[1])
    for (const effect of effects) assert.notEqual(names.effect(effect), '未知状态效果', effect)
    const enchantments = [
        ...source('loot/loot_functions.c')
            .split('ENCHANT_NAMES[64] = {')[1]
            .split('};')[0]
            .matchAll(/"([a-z_]+)"/g),
    ]
        .map((match) => match[1])
        .filter((name) => name !== 'no_enchantment')
    for (const enchantment of enchantments)
        assert.notEqual(names.enchantment(enchantment), '未知附魔', enchantment)
})

test('mirror, rotation and effect duration never expose internal enum values or ticks', () => {
    const names = labels('zh-CN')
    assert.equal(names.mirror(0), '无镜像')
    assert.equal(names.mirror(1), '已镜像')
    assert.equal(names.mirror(9), '未知镜像方式')
    assert.equal(names.hasMirror('village'), false)
    assert.equal(names.hasMirror('ruined-portal'), true)
    assert.equal(names.rotation(-90), '270°')
    assert.equal(names.rotation(NaN), '未知旋转方式')
    assert.equal(seedMapEffectDuration(3600), '3:00')
    assert.equal(seedMapEffectDuration(160), '0:08')
    assert.equal(seedMapEffectDuration(-1), undefined)
})
