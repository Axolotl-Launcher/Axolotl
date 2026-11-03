import { describe, expect, it } from 'vitest'

import { findCommand, getAllCommands, initializeRegistry } from './commands/registry.ts'
import { buildCommand, getDefaultValues, validateCommand } from './generator.ts'
import { loadCommandsForVersion, loadManifest } from './loader.ts'
import { createItemNbtTree, parseNbt, serializeNbtDocument } from './nbt-tree.ts'

describe('Java 26 release command syntax', () => {
    it('loads each release without leaking new commands into earlier versions', () => {
        expect(
            ['26.1', '26.2', '26.3'].every((id) =>
                loadManifest().versions.some((entry) => entry.id === id),
            ),
        ).toBeTruthy()
        initializeRegistry('1.21')
        expect(findCommand('swing')).toBe(undefined)
        expect(findCommand('compute')).toBe(undefined)
        initializeRegistry('26.1')
        expect(findCommand('swing')).toBeTruthy()
        expect(findCommand('fetchprofile')).toBeTruthy()
        expect(findCommand('unpublish')).toBe(undefined)
        initializeRegistry('26.2')
        expect(buildCommand('unpublish', {})).toBe('/unpublish')
        expect(findCommand('compute')).toBe(undefined)
        initializeRegistry('26.3')
        expect(findCommand('compute')).toBeTruthy()
        expect(findCommand('posteffect')).toBeTruthy()
        expect(new Set(getAllCommands().map((command) => command.id)).size).toBe(
            getAllCommands().length,
        )
    })

    it('builds world-clock time commands, including pause, rate and timeline queries', () => {
        initializeRegistry('26.1')
        expect(buildCommand('time', { action: 'set', value: 'day' })).toBe('/time set day')
        expect(
            buildCommand('time', {
                clock: 'minecraft:overworld',
                action: 'pause',
                value: 'ignored',
            }),
        ).toBe('/time of minecraft:overworld pause')
        expect(
            buildCommand('time', { clock: 'minecraft:overworld', action: 'rate', rate: 2 }),
        ).toBe('/time of minecraft:overworld rate 2')
        expect(buildCommand('time', { action: 'query', query: 'minecraft:day repetition' })).toBe(
            '/time query minecraft:day repetition',
        )
        expect(buildCommand('time', { action: 'query', query: 'gametime' })).toBe(
            '/time query gametime',
        )
        expect(
            buildCommand('time', {
                clock: 'minecraft:overworld',
                action: 'query',
                query: 'gametime',
            }),
        ).toBe('/time query gametime')
        expect(validateCommand('time', { action: 'resume' })).toEqual([])
        for (const rate of [0, -1, 1001])
            expect(validateCommand('time', { action: 'rate', rate }).length).toBeTruthy()
        expect(validateCommand('time', { action: 'rate', rate: 0.5 })).toEqual([])
        initializeRegistry('1.21')
        expect(validateCommand('time', { action: 'pause' }).length).toBeTruthy()
    })

    it('uses namespaced gamerules and modern attribute identifiers', () => {
        initializeRegistry('26.1')
        const presets = findCommand('gamerule')!.arguments[0].presets!
        expect(
            presets.every((preset) => String(preset.value).startsWith('minecraft:')),
        ).toBeTruthy()
        expect(presets.some((preset) => preset.value === 'minecraft:advance_time')).toBeTruthy()
        expect(
            presets.some((preset) => preset.value === 'minecraft:fire_spread_radius_around_player'),
        ).toBeTruthy()
        expect(buildCommand('gamerule', { rule: 'minecraft:keep_inventory', value: 'true' })).toBe(
            '/gamerule minecraft:keep_inventory true',
        )
        expect(
            buildCommand('attribute', {
                target: '@s',
                attribute: 'minecraft:max_health',
                action: 'base set',
                value: 40,
            }),
        ).toBe('/attribute @s minecraft:max_health base set 40')
        expect(
            findCommand('attribute')!.arguments[1].presets!.every(
                (preset) => !String(preset.value).includes('generic.'),
            ),
        ).toBeTruthy()
    })

    it('preserves release publish argument order and excludes snapshot-only online flags', () => {
        for (const version of ['26.1', '26.2']) {
            initializeRegistry(version)
            expect(
                buildCommand('publish', {
                    ...getDefaultValues('publish'),
                    allowCheats: true,
                    gamemode: 'creative',
                    port: 25566,
                }),
            ).toBe('/publish true creative 25566')
            expect(
                !findCommand('publish')!.arguments.some((arg) => arg.name === 'online'),
            ).toBeTruthy()
        }
        initializeRegistry('26.3')
        expect(
            buildCommand('publish', { allowCheats: true, gamemode: 'creative', port: 25566 }),
        ).toBe('/publish true 25566')
        expect(
            !findCommand('publish')!.arguments.some((arg) => arg.name === 'gamemode'),
        ).toBeTruthy()
    })

    it('builds swing, fetchprofile, dialog, waypoint and stopwatch branches', () => {
        initializeRegistry('26.1')
        expect(buildCommand('swing', {})).toBe('/swing')
        expect(buildCommand('fetchprofile', { source: 'entity', profile: '@s' })).toBe(
            '/fetchprofile entity @s',
        )
        expect(buildCommand('dialog', { action: 'clear', target: '@s', dialog: 'ignored' })).toBe(
            '/dialog clear @s',
        )
        expect(
            buildCommand('waypoint', {
                action: 'modify',
                waypoint: '@s',
                property: 'color hex',
                value: 'ff8800',
            }),
        ).toBe('/waypoint modify @s color hex ff8800')
        expect(
            buildCommand('stopwatch', {
                ...getDefaultValues('stopwatch'),
                action: 'create',
                id: 'test:timer',
            }),
        ).toBe('/stopwatch create test:timer')
        initializeRegistry('26.3')
        expect(
            buildCommand('swing', {
                ...getDefaultValues('swing'),
                animation: 'stab',
                duration: '10t',
            }),
        ).toBe('/swing @s mainhand stab 10t')
    })

    it('builds integer and float computations and computed data modifications', () => {
        initializeRegistry('26.3')
        expect(
            buildCommand('compute', {
                context: 'default',
                type: 'integer',
                provider: '5',
                scale: 3,
            }),
        ).toBe('/compute default integer 5')
        expect(
            buildCommand('compute', {
                context: 'block',
                pos: '~ ~ ~',
                type: 'float',
                provider: 'test:provider',
                scale: 100,
            }),
        ).toBe('/compute block ~ ~ ~ float test:provider 100')
        const values = {
            action: 'modify',
            targetType: 'storage',
            target: 'test:result',
            path: 'score',
            method: 'set',
            source: 'compute',
            context: 'entity',
            type: 'integer',
            provider: '{type:score,target:{type:context,target:target_entity},score:points}',
        }
        expect(validateCommand('data', { ...getDefaultValues('data'), ...values })).toEqual([])
        expect(buildCommand('data', { ...getDefaultValues('data'), ...values })).toBe(
            '/data modify storage test:result score set compute entity @s integer {type:score,target:{type:context,target:target_entity},score:points}',
        )
        expect(validateCommand('data', { ...values, path: '' }).length).toBeTruthy()
        expect(
            buildCommand('posteffect', {
                action: 'add',
                target: '@s',
                effect: 'minecraft:creeper',
            }),
        ).toBe('/posteffect add @s minecraft:creeper')
    })

    it('adds slots conditions and item fill/override only for 26.3, without changing cached base data', () => {
        const before = loadCommandsForVersion('26.2').find((command) => command.id === 'execute')!
        initializeRegistry('26.3')
        for (const action of ['fill', 'override'])
            expect(
                buildCommand('item', {
                    action,
                    targetType: 'entity',
                    target: '@s',
                    slot: 'container.*',
                    item: 'minecraft:stone',
                }),
            ).toBe(`/item ${action} entity @s container.* with minecraft:stone`)
        const conditions = findCommand('execute')!.chainCategories!.flatMap(
            (category) => category.subcommands,
        )
        expect(conditions.some((condition) => condition.id === 'if-slots')).toBeTruthy()
        expect(conditions.some((condition) => condition.id === 'unless-slots')).toBeTruthy()
        expect(
            !before.chainCategories!.some((category) =>
                category.subcommands.some((sub) => sub.id === 'if-slots'),
            ),
        ).toBeTruthy()
        expect(
            !loadCommandsForVersion('26.2')
                .find((command) => command.id === 'item')!
                .arguments[0].enumValues!.includes('fill'),
        ).toBeTruthy()
    })

    it('provides inline text components in advanced item presets for 26.x', () => {
        const nodes = createItemNbtTree('inline')
        const output = serializeNbtDocument(nodes, true)
        const parsed = parseNbt(output)
        expect(parsed.nodes.find((node) => node.name === 'custom_name')!.type).toBe('compound')
        expect(parsed.nodes.find((node) => node.name === 'enchantments')!.children![0].name).toBe(
            'minecraft:sharpness',
        )
        expect(!output.includes('display')).toBeTruthy()
    })

    it('keeps argument names unique in every supported version', () => {
        for (const version of loadManifest().versions) {
            for (const command of loadCommandsForVersion(version.id)) {
                const names = command.arguments.map((arg) => arg.name)
                expect(new Set(names).size, `${version.id}: ${command.id}`).toBe(names.length)
            }
        }
    })
})
