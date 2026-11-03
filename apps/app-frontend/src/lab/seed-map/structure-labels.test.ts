import { readFileSync } from 'node:fs'

import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it } from 'vitest'

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
        expect(
            message.id && catalog[message.id],
            `Missing ${locale} message ${message.id}`,
        ).toBeTruthy()
        return String(new IntlMessageFormat(catalog[message.id].message, locale).format(values))
    }, locale)
}
function source(path: string) {
    return readFileSync(new URL(`../../../../app/vendor/cubiomes/${path}`, import.meta.url), 'utf8')
}

it('all generated stronghold, fortress and end-city pieces have bilingual names', () => {
    for (const [kind, file] of [
        ['stronghold', 'stronghold'],
        ['fortress', 'fortress'],
        ['end-city', 'end_city'],
    ] as const) {
        const names = [...source(`features/${file}.c`).matchAll(/"([A-Za-z0-9_]+)"\s*\}/g)].map(
            (match) => match[1],
        )
        expect(names.length >= 12).toBeTruthy()
        for (const locale of ['en-US', 'zh-CN'])
            for (const name of names) {
                const result = labels(locale).piece(kind, name)
                expect(result).not.toBe(labels(locale).piece(kind, 'not_a_piece'))
                expect(result).not.toBe(name)
            }
    }
})

it('every current camp template and all shipwreck shapes have display names', () => {
    const camps = [
        ...source('features/abandoned_camp.c').matchAll(/(?:name\s*=\s*)"([^"]+)"/g),
    ].map((match) => match[1])
    const ships = [...source('finders.c').matchAll(/\{"(shipwreck\/[^"\s]+)"/g)].map(
        (match) => match[1],
    )
    expect(camps.length > 100).toBeTruthy()
    expect(ships.length).toBe(20)
    for (const locale of ['en-US', 'zh-CN']) {
        const names = labels(locale)
        const unknown = names.piece('abandoned-camp', 'unknown')
        for (const name of camps) {
            expect(names.piece('abandoned-camp', name, variant({ biome: 188 }))).not.toBe(unknown)
            expect(!names.piece('abandoned-camp', name, variant()).includes('%s')).toBeTruthy()
        }
        for (const name of ships) expect(names.piece('shipwreck', name)).not.toBe(unknown)
    }
})

it('legacy codes and remaining supported template names are translated', () => {
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
            expect(labels(locale).piece('bastion', name)).not.toBe(
                labels(locale).piece('bastion', 'unknown'),
            )
    expect(labels('zh-CN').piece('stronghold', 'SHPR')).toBe('末地传送门房间')
    expect(labels('zh-CN').piece('fortress', 'NeMT')).toBe('烈焰人刷怪平台')
})

it('start variants follow structure-specific indices, biome and giant flag', () => {
    const names = labels()
    for (const biome of [1, 177, 2, 35, 5, 12]) {
        const min = biome === 1 || biome === 177 ? 0 : 1
        const max = biome === 35 ? 4 : biome === 5 ? 2 : 3
        for (let start = min; start <= max; start++)
            expect(names.variant('village', variant({ biome, start }), [])?.id).toBeTruthy()
    }
    expect(names.variant('ruined-portal', variant({ start: 3, giant: true }), [])?.id).toBe(
        'giant_ruined_portal_3',
    )
    expect(names.variant('ruined-portal', variant({ start: 10 }), [])?.id).toBe('ruined_portal_10')
    expect(names.variant('trial-chambers', variant({ start: 0 }), [])?.id).toBe('corridor/end_1')
    expect(names.variant('nether-fossil', variant({ start: 13 }), [])?.id).toBe('fossil_14')
    expect(names.variant('ancient-city', variant({ start: 3 }), [])?.id).toBe('city_center_3')
    expect(names.variant('bastion', variant({ start: 0 }), [])?.name).toBe('Housing units')
    expect(names.variant('igloo', variant({ start: 255 }), [])).toBe(undefined)
    expect(names.variant('village', variant({ start: 255 }), [])).toBe(undefined)
    expect(names.variant('ruined-portal', variant({ start: 99 }), [])).toBe(undefined)
    expect(names.variant('swamp-hut', variant({ start: 0 }), [])).toBe(undefined)
})

it('shipwreck start indices use the beached pool instead of the ocean pool', () => {
    const names = labels()
    expect(names.variant('shipwreck', variant({ biome: 0, start: 1 }), [])?.id).toBe(
        'shipwreck/upsidedown_full',
    )
    expect(names.variant('shipwreck', variant({ biome: 16, start: 1 }), [])?.id).toBe(
        'shipwreck/sideways_full',
    )
    const pieces = [{ name: 'shipwreck/with_mast_degraded' }] as SeedMapStructurePiece[]
    expect(names.variant('shipwreck', variant({ start: 1 }), pieces)?.name).toBe(
        'Shipwreck with mast (degraded)',
    )
})

it('camp placeholders resolve only with a known biome and raw IDs remain distinct', () => {
    const names = labels('zh-CN')
    const raw = 'abandoned_camp/tent/%s/tent_%s_7'
    expect(names.pieceId(raw, variant({ biome: 188 }))).toBe(
        'abandoned_camp/tent/dappled_forest/tent_dappled_forest_7',
    )
    expect(names.pieceId(raw, variant({ biome: -1 }))).toBe(undefined)
    expect(names.pieceId('future/%s/unknown', variant())).toBe(undefined)
    expect(names.pieceId('SHPR')).toBe('SHPR')
    expect(
        names.piece('abandoned-camp', raw, variant({ biome: 188 })).endsWith('帐篷 7'),
    ).toBeTruthy()
    expect(names.piece('end-city', 'future/room')).toBe('结构部件')
})

it('every emitted container table has a bilingual display name', () => {
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
            expect(names.table(table)).not.toBe(names.table('unknown/table'))
    }
})

it('vanilla loot names translate enchantments, levels and effects with safe unknown fallbacks', () => {
    const names = labels('zh-CN')
    expect(names.item('minecraft:diamond')).toBe('钻石')
    expect(names.enchantment('minecraft:fortune')).toBe('时运')
    expect(names.level(3)).toBe('III')
    expect(names.level(99)).toBe('99')
    expect(names.effect('minecraft:regeneration')).toBe('生命恢复')
    expect(names.item('custom:unknown')).toBe('未知物品')
    expect(names.enchantment('custom:fortune')).toBe('未知附魔')
    expect(names.effect('minecraft:not_known')).toBe('未知状态效果')
    const effects = [
        ...source('loot/loot_functions.c')
            .split('const struct Potion')[0]
            .matchAll(/"(minecraft:[a-z_]+)"/g),
    ].map((match) => match[1])
    for (const effect of effects) expect(names.effect(effect)).not.toBe('未知状态效果')
    const enchantments = [
        ...source('loot/loot_functions.c')
            .split('ENCHANT_NAMES[64] = {')[1]
            .split('};')[0]
            .matchAll(/"([a-z_]+)"/g),
    ]
        .map((match) => match[1])
        .filter((name) => name !== 'no_enchantment')
    for (const enchantment of enchantments)
        expect(names.enchantment(enchantment)).not.toBe('未知附魔')
})

it('mirror, rotation and effect duration never expose internal enum values or ticks', () => {
    const names = labels('zh-CN')
    expect(names.mirror(0)).toBe('无镜像')
    expect(names.mirror(1)).toBe('已镜像')
    expect(names.mirror(9)).toBe('未知镜像方式')
    expect(names.hasMirror('village')).toBe(false)
    expect(names.hasMirror('ruined-portal')).toBe(true)
    expect(names.rotation(-90)).toBe('270°')
    expect(names.rotation(NaN)).toBe('未知旋转方式')
    expect(seedMapEffectDuration(3600)).toBe('3:00')
    expect(seedMapEffectDuration(160)).toBe('0:08')
    expect(seedMapEffectDuration(-1)).toBe(undefined)
})
