import { describe, expect, it } from 'vitest'

import type { ExecuteChainSubcommand } from './commands/types.ts'
import { loadCommandsForVersion } from './loader.ts'
import {
    LEGACY_SAMPLE_STATE,
    migratePresetState,
    presetNeedsMigration,
    type UserPreset,
} from './presets.ts'

const ifSub: ExecuteChainSubcommand = {
    id: 'if',
    labelKey: 'if.label',
    descriptionKey: 'if.desc',
    template: '',
    arguments: [],
    variants: {
        argument: 'condition',
        labelKey: 'variant.field.condition',
        options: [
            {
                id: 'biome',
                labelKey: 'biome.label',
                descriptionKey: 'biome.desc',
                template: 'if biome {pos} {biome}',
                arguments: [],
            },
        ],
    },
}

const lookup = new Map<string, ExecuteChainSubcommand>([['if', ifSub]])

function preset(executeState: UserPreset['executeState'], chainString?: string): UserPreset {
    return {
        id: 'preset-1',
        name: 'Chain',
        commandId: 'execute',
        versionId: '1.21',
        values: {},
        executeState,
        chainString,
        createdAt: 0,
    }
}

describe('presetNeedsMigration', () => {
    it('flags steps stored under the old one-id-per-form names', () => {
        expect(
            presetNeedsMigration(
                preset({
                    chain: [{ subcommandId: 'if-biome', values: { pos: '~ ~ ~' } }],
                    finalCommand: 'say hi',
                }),
                lookup,
            ),
        ).toBe(true)
    })

    it('leaves a preset that already stores the form inside the step alone', () => {
        expect(
            presetNeedsMigration(
                preset({
                    chain: [{ subcommandId: 'if', values: { condition: 'biome' } }],
                    finalCommand: 'say hi',
                }),
                lookup,
            ),
        ).toBe(false)
    })

    it('flags a preset that only kept the command string', () => {
        expect(presetNeedsMigration(preset(undefined, 'as @a run say hi'), lookup)).toBe(true)
    })
})

describe('migratePresetState', () => {
    it('rewrites the old step into a subcommand plus its form', () => {
        expect(
            migratePresetState(
                {
                    chain: [
                        { subcommandId: 'if-biome', values: { pos: '~ ~ ~', biome: 'plains' } },
                        { subcommandId: 'as', values: { target: '@a' } },
                    ],
                    finalCommand: 'say hi',
                },
                lookup,
            ),
        ).toEqual({
            chain: [
                {
                    subcommandId: 'if',
                    values: { pos: '~ ~ ~', biome: 'plains', condition: 'biome' },
                },
                { subcommandId: 'as', values: { target: '@a' } },
            ],
            finalCommand: 'say hi',
        })
    })

    it('reports nothing to do when every step is already current', () => {
        expect(
            migratePresetState(
                {
                    chain: [{ subcommandId: 'if', values: { condition: 'biome' } }],
                    finalCommand: '',
                },
                lookup,
            ),
        ).toBeNull()
    })

    it('upgrades the development sample against the real 1.21 data', () => {
        const real = new Map<string, ExecuteChainSubcommand>()
        for (const command of loadCommandsForVersion('1.21')) {
            for (const category of command.chainCategories ?? []) {
                for (const sub of category.subcommands) real.set(sub.id, sub)
            }
        }
        expect(presetNeedsMigration(preset(LEGACY_SAMPLE_STATE), real)).toBe(true)
        expect(migratePresetState(LEGACY_SAMPLE_STATE, real)).toEqual({
            chain: [
                {
                    subcommandId: 'if',
                    values: { pos: '~ ~ ~', biome: 'minecraft:plains', condition: 'biome' },
                },
                { subcommandId: 'positioned', values: { target: '@p', positioning: 'as' } },
                {
                    subcommandId: 'store',
                    values: {
                        target: '@s',
                        path: 'Health',
                        type: 'int',
                        scale: 1,
                        destination: 'entity',
                        mode: 'success',
                    },
                },
            ],
            finalCommand: 'say migrated',
        })
    })
})
