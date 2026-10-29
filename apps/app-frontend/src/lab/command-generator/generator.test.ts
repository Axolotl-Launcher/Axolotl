import { beforeEach, describe, expect, it } from 'vitest'

import { findCommand, getAllCommands, initializeRegistry } from './commands/registry.ts'
import { buildCommand, getActiveArguments, getDefaultValues, validateCommand } from './generator.ts'

beforeEach(() => {
    initializeRegistry()
})

describe('command syntax regressions', () => {
    it('distinguishes boolean values from keyword switches', () => {
        const values = { ...getDefaultValues('effect'), target: '@s', effect: 'minecraft:speed' }
        expect(buildCommand('effect', { ...values, hideParticles: true })).toBe(
            '/effect give @s minecraft:speed 30 0 true',
        )
        expect(buildCommand('effect', { ...values, hideParticles: false })).toBe(
            '/effect give @s minecraft:speed 30 0 false',
        )
        expect(
            buildCommand('spreadplayers', {
                center: '0 0',
                spreadDistance: 10,
                maxRange: 20,
                respectTeams: false,
                target: '@a',
            }),
        ).toBe('/spreadplayers 0 0 10 20 false @a')
        expect(
            validateCommand('spreadplayers', {
                center: '0 0',
                spreadDistance: 10,
                maxRange: 20,
                target: '@a',
            }).length,
        ).toBeTruthy()
    })

    it('excludes inactive effect arguments from the form, validation and output', () => {
        const values = {
            ...getDefaultValues('effect'),
            action: 'clear',
            target: '@s',
            effect: 'minecraft:speed',
            duration: 'invalid',
        }
        expect(getActiveArguments(findCommand('effect')!, values).map((arg) => arg.name)).toEqual([
            'action',
            'target',
            'effect',
        ])
        expect(validateCommand('effect', values)).toEqual([])
        expect(buildCommand('effect', values)).toBe('/effect clear @s minecraft:speed')
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
        expect(validateCommand('item', block)).toEqual([])
        expect(buildCommand('item', block)).toBe(
            '/item replace block ~ ~ ~ container.0 with minecraft:stone 1',
        )
        expect(
            buildCommand('item', {
                ...block,
                targetType: 'entity',
                target: '@s',
                slot: 'weapon.mainhand',
                nbt: '[unbreakable={}]',
            }),
        ).toBe('/item replace entity @s weapon.mainhand with minecraft:stone[unbreakable={}] 1')
        const modify = { ...block, action: 'modify', modifier: 'minecraft:my_modifier' }
        expect(buildCommand('item', modify)).toBe(
            '/item modify block ~ ~ ~ container.0 minecraft:my_modifier',
        )
        expect(validateCommand('item', { ...modify, modifier: '' }).length).toBeTruthy()
        expect(validateCommand('item', { ...block, item: '' }).length).toBeTruthy()
    })

    it('includes advancement mode and requires an id only for modes that use it', () => {
        const values = {
            ...getDefaultValues('advancement'),
            action: 'grant',
            target: '@s',
            advancement: 'minecraft:story/root',
        }
        expect(buildCommand('advancement', values)).toBe(
            '/advancement grant @s only minecraft:story/root',
        )
        expect(
            buildCommand('advancement', { ...values, mode: 'everything', criterion: 'ignored' }),
        ).toBe('/advancement grant @s everything')
        expect(
            validateCommand('advancement', { action: 'grant', target: '@s', mode: 'everything' }),
        ).toEqual([])
        expect(
            validateCommand('advancement', { action: 'grant', target: '@s', mode: 'only' }).length,
        ).toBeTruthy()
    })

    it('uses historical locate syntax and introduces commands in their supported versions', () => {
        initializeRegistry('1.16')
        expect(findCommand('item')).toBe(undefined)
        expect(findCommand('jfr')).toBe(undefined)
        expect(buildCommand('locate', { id: 'minecraft:village' })).toBe(
            '/locate minecraft:village',
        )
        expect(buildCommand('locatebiome', { id: 'minecraft:plains' })).toBe(
            '/locatebiome minecraft:plains',
        )
        expect(
            !findCommand('execute')!.chainCategories!.some((c) =>
                c.subcommands.some((s) => s.id === 'if-biome'),
            ),
        ).toBeTruthy()
        initializeRegistry('1.17')
        expect(findCommand('item')).toBeTruthy()
        expect(findCommand('jfr')).toBe(undefined)
        initializeRegistry('1.18')
        expect(findCommand('jfr')).toBeTruthy()
        initializeRegistry('1.19')
        expect(findCommand('locatebiome')).toBe(undefined)
        expect(buildCommand('locate', { type: 'structure', id: 'minecraft:village_plains' })).toBe(
            '/locate structure minecraft:village_plains',
        )
        expect(
            findCommand('execute')!.chainCategories!.some((c) =>
                c.subcommands.some((s) => s.id === 'if-biome'),
            ),
        ).toBeTruthy()
    })
})

describe('buildCommand', () => {
    it('generates a basic give command', () => {
        expect(
            buildCommand('give', {
                target: '@s',
                item: 'minecraft:diamond',
                count: 64,
            }),
        ).toBe('/give @s minecraft:diamond 64')
    })

    it('generates a summon command with NBT', () => {
        expect(
            buildCommand('summon', {
                entity: 'minecraft:axolotl',
                pos: '~ ~ ~',
                nbt: '{Variant:0}',
            }),
        ).toBe('/summon minecraft:axolotl ~ ~ ~ {Variant:0}')
    })

    it('generates a command with no arguments', () => {
        expect(buildCommand('seed', {})).toBe('/seed')
        expect(buildCommand('reload', {})).toBe('/reload')
    })

    it('appends boolean arguments only when true', () => {
        expect(buildCommand('list', { uuids: true })).toBe('/list uuids')
        expect(buildCommand('list', { uuids: false })).toBe('/list')
    })

    it('automatically quotes message arguments', () => {
        expect(buildCommand('say', { message: 'Hello World' })).toBe('/say "Hello World"')
    })

    it('skips empty optional arguments', () => {
        expect(buildCommand('gamemode', { mode: 'creative', target: '' })).toBe(
            '/gamemode creative',
        )
    })

    it('throws an error when a required argument is missing', () => {
        expect(() => buildCommand('gamemode', {})).toThrow()
    })

    it('throws an error for an unknown command', () => {
        expect(() => buildCommand('nonexistent', {})).toThrow(/Unknown command/)
    })
})

describe('validateCommand', () => {
    it('returns an empty error array for valid arguments', () => {
        expect(validateCommand('gamemode', { mode: 'creative' }).length).toBe(0)
    })

    it('returns errors when required fields are missing', () => {
        expect(validateCommand('give', {}).length > 0).toBeTruthy()
    })

    // Note: `gamemode` uses a custom "gamemode" type, so the enum validation in `generator.ts` does not apply.
    // If enum validation is needed for custom types, the `validateCommand` logic should be updated.

    it('returns errors for non-integer values passed to integer arguments', () => {
        expect(
            validateCommand('give', {
                target: '@s',
                item: 'minecraft:stone',
                count: 1.5,
            }).length > 0,
        ).toBeTruthy()
    })
})

describe('getDefaultValues', () => {
    it('returns preset default values', () => {
        expect(getDefaultValues('give').count).toBe(1)
    })

    it('returns an empty object for an unknown command', () => {
        expect(getDefaultValues('nonexistent')).toEqual({})
    })
})

describe('registry completeness', () => {
    it('all commands have unique ids', () => {
        const commands = getAllCommands()
        const ids = commands.map((c) => c.id)
        expect(new Set(ids).size).toBe(ids.length)
    })

    it('all commands have descriptions and categories', () => {
        for (const cmd of getAllCommands()) {
            expect(cmd.descriptionKey).toBeTruthy()
            expect(cmd.category).toBeTruthy()
        }
    })
})
