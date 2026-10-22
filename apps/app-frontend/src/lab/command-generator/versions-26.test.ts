import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import { findCommand, getAllCommands, initializeRegistry } from './commands/registry.ts'
import { buildCommand, getDefaultValues, validateCommand } from './generator.ts'
import { loadCommandsForVersion, loadManifest } from './loader.ts'
import { createItemNbtTree, parseNbt, serializeNbtDocument } from './nbt-tree.ts'

describe('Java 26 release command syntax', () => {
    it('loads each release without leaking new commands into earlier versions', () => {
        assert.ok(
            ['26.1', '26.2', '26.3'].every((id) =>
                loadManifest().versions.some((entry) => entry.id === id),
            ),
        )
        initializeRegistry('1.21')
        assert.equal(findCommand('swing'), undefined)
        assert.equal(findCommand('compute'), undefined)
        initializeRegistry('26.1')
        assert.ok(findCommand('swing'))
        assert.ok(findCommand('fetchprofile'))
        assert.equal(findCommand('unpublish'), undefined)
        initializeRegistry('26.2')
        assert.equal(buildCommand('unpublish', {}), '/unpublish')
        assert.equal(findCommand('compute'), undefined)
        initializeRegistry('26.3')
        assert.ok(findCommand('compute'))
        assert.ok(findCommand('posteffect'))
        assert.equal(
            new Set(getAllCommands().map((command) => command.id)).size,
            getAllCommands().length,
        )
    })

    it('builds world-clock time commands, including pause, rate and timeline queries', () => {
        initializeRegistry('26.1')
        assert.equal(buildCommand('time', { action: 'set', value: 'day' }), '/time set day')
        assert.equal(
            buildCommand('time', {
                clock: 'minecraft:overworld',
                action: 'pause',
                value: 'ignored',
            }),
            '/time of minecraft:overworld pause',
        )
        assert.equal(
            buildCommand('time', { clock: 'minecraft:overworld', action: 'rate', rate: 2 }),
            '/time of minecraft:overworld rate 2',
        )
        assert.equal(
            buildCommand('time', { action: 'query', query: 'minecraft:day repetition' }),
            '/time query minecraft:day repetition',
        )
        assert.equal(
            buildCommand('time', { action: 'query', query: 'gametime' }),
            '/time query gametime',
        )
        assert.equal(
            buildCommand('time', {
                clock: 'minecraft:overworld',
                action: 'query',
                query: 'gametime',
            }),
            '/time query gametime',
        )
        assert.deepEqual(validateCommand('time', { action: 'resume' }), [])
        for (const rate of [0, -1, 1001])
            assert.ok(validateCommand('time', { action: 'rate', rate }).length)
        assert.deepEqual(validateCommand('time', { action: 'rate', rate: 0.5 }), [])
        initializeRegistry('1.21')
        assert.ok(validateCommand('time', { action: 'pause' }).length)
    })

    it('uses namespaced gamerules and modern attribute identifiers', () => {
        initializeRegistry('26.1')
        const presets = findCommand('gamerule')!.arguments[0].presets!
        assert.ok(presets.every((preset) => String(preset.value).startsWith('minecraft:')))
        assert.ok(presets.some((preset) => preset.value === 'minecraft:advance_time'))
        assert.ok(
            presets.some((preset) => preset.value === 'minecraft:fire_spread_radius_around_player'),
        )
        assert.equal(
            buildCommand('gamerule', { rule: 'minecraft:keep_inventory', value: 'true' }),
            '/gamerule minecraft:keep_inventory true',
        )
        assert.equal(
            buildCommand('attribute', {
                target: '@s',
                attribute: 'minecraft:max_health',
                action: 'base set',
                value: 40,
            }),
            '/attribute @s minecraft:max_health base set 40',
        )
        assert.ok(
            findCommand('attribute')!.arguments[1].presets!.every(
                (preset) => !String(preset.value).includes('generic.'),
            ),
        )
    })

    it('preserves release publish argument order and excludes snapshot-only online flags', () => {
        for (const version of ['26.1', '26.2']) {
            initializeRegistry(version)
            assert.equal(
                buildCommand('publish', {
                    ...getDefaultValues('publish'),
                    allowCheats: true,
                    gamemode: 'creative',
                    port: 25566,
                }),
                '/publish true creative 25566',
            )
            assert.ok(!findCommand('publish')!.arguments.some((arg) => arg.name === 'online'))
        }
        initializeRegistry('26.3')
        assert.equal(
            buildCommand('publish', { allowCheats: true, gamemode: 'creative', port: 25566 }),
            '/publish true 25566',
        )
        assert.ok(!findCommand('publish')!.arguments.some((arg) => arg.name === 'gamemode'))
    })

    it('builds swing, fetchprofile, dialog, waypoint and stopwatch branches', () => {
        initializeRegistry('26.1')
        assert.equal(buildCommand('swing', {}), '/swing')
        assert.equal(
            buildCommand('fetchprofile', { source: 'entity', profile: '@s' }),
            '/fetchprofile entity @s',
        )
        assert.equal(
            buildCommand('dialog', { action: 'clear', target: '@s', dialog: 'ignored' }),
            '/dialog clear @s',
        )
        assert.equal(
            buildCommand('waypoint', {
                action: 'modify',
                waypoint: '@s',
                property: 'color hex',
                value: 'ff8800',
            }),
            '/waypoint modify @s color hex ff8800',
        )
        assert.equal(
            buildCommand('stopwatch', {
                ...getDefaultValues('stopwatch'),
                action: 'create',
                id: 'test:timer',
            }),
            '/stopwatch create test:timer',
        )
        initializeRegistry('26.3')
        assert.equal(
            buildCommand('swing', {
                ...getDefaultValues('swing'),
                animation: 'stab',
                duration: '10t',
            }),
            '/swing @s mainhand stab 10t',
        )
    })

    it('builds integer and float computations and computed data modifications', () => {
        initializeRegistry('26.3')
        assert.equal(
            buildCommand('compute', {
                context: 'default',
                type: 'integer',
                provider: '5',
                scale: 3,
            }),
            '/compute default integer 5',
        )
        assert.equal(
            buildCommand('compute', {
                context: 'block',
                pos: '~ ~ ~',
                type: 'float',
                provider: 'test:provider',
                scale: 100,
            }),
            '/compute block ~ ~ ~ float test:provider 100',
        )
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
        assert.deepEqual(validateCommand('data', { ...getDefaultValues('data'), ...values }), [])
        assert.equal(
            buildCommand('data', { ...getDefaultValues('data'), ...values }),
            '/data modify storage test:result score set compute entity @s integer {type:score,target:{type:context,target:target_entity},score:points}',
        )
        assert.ok(validateCommand('data', { ...values, path: '' }).length)
        assert.equal(
            buildCommand('posteffect', {
                action: 'add',
                target: '@s',
                effect: 'minecraft:creeper',
            }),
            '/posteffect add @s minecraft:creeper',
        )
    })

    it('adds slots conditions and item fill/override only for 26.3, without changing cached base data', () => {
        const before = loadCommandsForVersion('26.2').find((command) => command.id === 'execute')!
        initializeRegistry('26.3')
        for (const action of ['fill', 'override'])
            assert.equal(
                buildCommand('item', {
                    action,
                    targetType: 'entity',
                    target: '@s',
                    slot: 'container.*',
                    item: 'minecraft:stone',
                }),
                `/item ${action} entity @s container.* with minecraft:stone`,
            )
        const conditions = findCommand('execute')!.chainCategories!.flatMap(
            (category) => category.subcommands,
        )
        assert.ok(conditions.some((condition) => condition.id === 'if-slots'))
        assert.ok(conditions.some((condition) => condition.id === 'unless-slots'))
        assert.ok(
            !before.chainCategories!.some((category) =>
                category.subcommands.some((sub) => sub.id === 'if-slots'),
            ),
        )
        assert.ok(
            !loadCommandsForVersion('26.2')
                .find((command) => command.id === 'item')!
                .arguments[0].enumValues!.includes('fill'),
        )
    })

    it('provides inline text components in advanced item presets for 26.x', () => {
        const nodes = createItemNbtTree('inline')
        const output = serializeNbtDocument(nodes, true)
        const parsed = parseNbt(output)
        assert.equal(parsed.nodes.find((node) => node.name === 'custom_name')!.type, 'compound')
        assert.equal(
            parsed.nodes.find((node) => node.name === 'enchantments')!.children![0].name,
            'minecraft:sharpness',
        )
        assert.ok(!output.includes('display'))
    })

    it('keeps argument names unique in every supported version', () => {
        for (const version of loadManifest().versions) {
            for (const command of loadCommandsForVersion(version.id)) {
                const names = command.arguments.map((arg) => arg.name)
                assert.equal(new Set(names).size, names.length, `${version.id}: ${command.id}`)
            }
        }
    })
})
