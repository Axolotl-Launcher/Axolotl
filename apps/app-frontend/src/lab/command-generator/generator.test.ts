import assert from 'node:assert/strict'
import { beforeEach, describe, it } from 'node:test'

import { findCommand, getAllCommands, initializeRegistry } from './commands/registry.ts'
import { buildCommand, getActiveArguments, getDefaultValues, validateCommand } from './generator.ts'

beforeEach(() => {
    initializeRegistry()
})

describe('command syntax regressions', () => {
    it('distinguishes boolean values from keyword switches', () => {
        const values = { ...getDefaultValues('effect'), target: '@s', effect: 'minecraft:speed' }
        assert.equal(
            buildCommand('effect', { ...values, hideParticles: true }),
            '/effect give @s minecraft:speed 30 0 true',
        )
        assert.equal(
            buildCommand('effect', { ...values, hideParticles: false }),
            '/effect give @s minecraft:speed 30 0 false',
        )
        assert.equal(
            buildCommand('spreadplayers', {
                center: '0 0',
                spreadDistance: 10,
                maxRange: 20,
                respectTeams: false,
                target: '@a',
            }),
            '/spreadplayers 0 0 10 20 false @a',
        )
        assert.ok(
            validateCommand('spreadplayers', {
                center: '0 0',
                spreadDistance: 10,
                maxRange: 20,
                target: '@a',
            }).length,
        )
    })

    it('excludes inactive effect arguments from the form, validation and output', () => {
        const values = {
            ...getDefaultValues('effect'),
            action: 'clear',
            target: '@s',
            effect: 'minecraft:speed',
            duration: 'invalid',
        }
        assert.deepEqual(
            getActiveArguments(findCommand('effect')!, values).map((arg) => arg.name),
            ['action', 'target', 'effect'],
        )
        assert.deepEqual(validateCommand('effect', values), [])
        assert.equal(buildCommand('effect', values), '/effect clear @s minecraft:speed')
    })

    it('includes item target and source literals for block and entity replacements', () => {
        const defaults = getDefaultValues('item')
        const block = {
            ...defaults,
            action: 'replace',
            pos: '~ ~ ~',
            slot: 'container.0',
            item: 'minecraft:stone',
            count: 1,
        }
        assert.deepEqual(validateCommand('item', block), [])
        assert.equal(
            buildCommand('item', block),
            '/item replace block ~ ~ ~ container.0 with minecraft:stone 1',
        )
        assert.equal(
            buildCommand('item', {
                ...block,
                targetType: 'entity',
                target: '@s',
                slot: 'weapon.mainhand',
                nbt: '[unbreakable={}]',
            }),
            '/item replace entity @s weapon.mainhand with minecraft:stone[unbreakable={}] 1',
        )
        const modify = { ...block, action: 'modify', modifier: 'minecraft:my_modifier' }
        assert.equal(
            buildCommand('item', modify),
            '/item modify block ~ ~ ~ container.0 minecraft:my_modifier',
        )
        assert.ok(validateCommand('item', { ...modify, modifier: '' }).length)
        assert.ok(validateCommand('item', { ...block, item: '' }).length)
    })

    it('includes advancement mode and requires an id only for modes that use it', () => {
        const values = {
            ...getDefaultValues('advancement'),
            action: 'grant',
            target: '@s',
            advancement: 'minecraft:story/root',
        }
        assert.equal(
            buildCommand('advancement', values),
            '/advancement grant @s only minecraft:story/root',
        )
        assert.equal(
            buildCommand('advancement', { ...values, mode: 'everything', criterion: 'ignored' }),
            '/advancement grant @s everything',
        )
        assert.deepEqual(
            validateCommand('advancement', { action: 'grant', target: '@s', mode: 'everything' }),
            [],
        )
        assert.ok(
            validateCommand('advancement', { action: 'grant', target: '@s', mode: 'only' }).length,
        )
    })

    it('uses historical locate syntax and introduces commands in their supported versions', () => {
        initializeRegistry('1.16')
        assert.equal(findCommand('item'), undefined)
        assert.equal(findCommand('jfr'), undefined)
        assert.equal(
            buildCommand('locate', { id: 'minecraft:village' }),
            '/locate minecraft:village',
        )
        assert.equal(
            buildCommand('locatebiome', { id: 'minecraft:plains' }),
            '/locatebiome minecraft:plains',
        )
        assert.ok(
            !findCommand('execute')!.chainCategories!.some((c) =>
                c.subcommands.some((s) => s.id === 'if-biome'),
            ),
        )
        initializeRegistry('1.17')
        assert.ok(findCommand('item'))
        assert.equal(findCommand('jfr'), undefined)
        initializeRegistry('1.18')
        assert.ok(findCommand('jfr'))
        initializeRegistry('1.19')
        assert.equal(findCommand('locatebiome'), undefined)
        assert.equal(
            buildCommand('locate', { type: 'structure', id: 'minecraft:village_plains' }),
            '/locate structure minecraft:village_plains',
        )
        assert.ok(
            findCommand('execute')!.chainCategories!.some((c) =>
                c.subcommands.some((s) => s.id === 'if-biome'),
            ),
        )
    })
})

describe('buildCommand', () => {
    it('generates a basic give command', () => {
        assert.equal(
            buildCommand('give', {
                target: '@s',
                item: 'minecraft:diamond',
                count: 64,
            }),
            '/give @s minecraft:diamond 64',
        )
    })

    it('generates a summon command with NBT', () => {
        assert.equal(
            buildCommand('summon', {
                entity: 'minecraft:axolotl',
                pos: '~ ~ ~',
                nbt: '{Variant:0}',
            }),
            '/summon minecraft:axolotl ~ ~ ~ {Variant:0}',
        )
    })

    it('generates a command with no arguments', () => {
        assert.equal(buildCommand('seed', {}), '/seed')
        assert.equal(buildCommand('reload', {}), '/reload')
    })

    it('appends boolean arguments only when true', () => {
        assert.equal(buildCommand('list', { uuids: true }), '/list uuids')
        assert.equal(buildCommand('list', { uuids: false }), '/list')
    })

    it('automatically quotes message arguments', () => {
        assert.equal(buildCommand('say', { message: 'Hello World' }), '/say "Hello World"')
    })

    it('skips empty optional arguments', () => {
        assert.equal(
            buildCommand('gamemode', { mode: 'creative', target: '' }),
            '/gamemode creative',
        )
    })

    it('throws an error when a required argument is missing', () => {
        assert.throws(() => buildCommand('gamemode', {}))
    })

    it('throws an error for an unknown command', () => {
        assert.throws(() => buildCommand('nonexistent', {}), /Unknown command/)
    })
})

describe('validateCommand', () => {
    it('returns an empty error array for valid arguments', () => {
        assert.equal(validateCommand('gamemode', { mode: 'creative' }).length, 0)
    })

    it('returns errors when required fields are missing', () => {
        assert.ok(validateCommand('give', {}).length > 0)
    })

    // Note: `gamemode` uses a custom "gamemode" type, so the enum validation in `generator.ts` does not apply.
    // If enum validation is needed for custom types, the `validateCommand` logic should be updated.

    it('returns errors for non-integer values passed to integer arguments', () => {
        assert.ok(
            validateCommand('give', {
                target: '@s',
                item: 'minecraft:stone',
                count: 1.5,
            }).length > 0,
        )
    })
})

describe('getDefaultValues', () => {
    it('returns preset default values', () => {
        assert.equal(getDefaultValues('give').count, 1)
    })

    it('returns an empty object for an unknown command', () => {
        assert.deepEqual(getDefaultValues('nonexistent'), {})
    })
})

describe('registry completeness', () => {
    it('all commands have unique ids', () => {
        const commands = getAllCommands()
        const ids = commands.map((c) => c.id)
        assert.equal(new Set(ids).size, ids.length)
    })

    it('all commands have descriptions and categories', () => {
        for (const cmd of getAllCommands()) {
            assert.ok(cmd.descriptionKey)
            assert.ok(cmd.category)
        }
    })
})
