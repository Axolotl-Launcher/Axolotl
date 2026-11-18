import { describe, expect, it } from 'vitest'

import type { ExecuteChainSubcommand } from './commands/types.ts'
import {
    migrateLegacyStep,
    resolveVariant,
    stepArguments,
    stepTemplate,
    switchVariant,
} from './variants.ts'

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
                id: 'entity',
                labelKey: 'kind.entity',
                descriptionKey: 'if-entity.desc',
                template: 'if entity {target}',
                arguments: [
                    {
                        name: 'target',
                        displayNameKey: 'target.display',
                        type: 'entity-selector',
                        required: true,
                        descriptionKey: 'target.desc',
                    },
                ],
            },
            {
                id: 'block',
                labelKey: 'kind.block',
                descriptionKey: 'if-block.desc',
                template: 'if block {pos} {block}',
                arguments: [
                    {
                        name: 'pos',
                        displayNameKey: 'pos.display',
                        type: 'coordinates',
                        required: true,
                        descriptionKey: 'pos.desc',
                    },
                    {
                        name: 'block',
                        displayNameKey: 'block.display',
                        type: 'block',
                        required: true,
                        descriptionKey: 'block.desc',
                    },
                ],
            },
        ],
    },
}

const plainSub: ExecuteChainSubcommand = {
    id: 'as',
    labelKey: 'as.label',
    descriptionKey: 'as.desc',
    template: 'as {target}',
    arguments: [
        {
            name: 'target',
            displayNameKey: 'target.display',
            type: 'entity-selector',
            required: true,
            descriptionKey: 'target.desc',
        },
    ],
}

describe('resolveVariant', () => {
    it('finds the option stored in the step', () => {
        expect(resolveVariant(ifSub, { condition: 'block' })?.id).toBe('block')
    })

    it('returns nothing while no condition is picked', () => {
        expect(resolveVariant(ifSub, {})).toBeUndefined()
        expect(resolveVariant(ifSub, { condition: '' })).toBeUndefined()
    })

    it('returns nothing for a condition the data no longer has', () => {
        expect(resolveVariant(ifSub, { condition: 'items' })).toBeUndefined()
    })
})

describe('stepArguments and stepTemplate', () => {
    it('follow the picked condition', () => {
        expect(stepArguments(ifSub, { condition: 'block' }).map((arg) => arg.name)).toEqual([
            'pos',
            'block',
        ])
        expect(stepTemplate(ifSub, { condition: 'block' })).toBe('if block {pos} {block}')
    })

    it('stay empty until a condition is picked', () => {
        expect(stepArguments(ifSub, {})).toEqual([])
        expect(stepTemplate(ifSub, {})).toBe('')
    })

    it('use the subcommand itself when it carries no condition', () => {
        expect(stepArguments(plainSub, {}).map((arg) => arg.name)).toEqual(['target'])
        expect(stepTemplate(plainSub, {})).toBe('as {target}')
    })
})

describe('switchVariant', () => {
    it('keeps values whose name still exists', () => {
        expect(
            switchVariant(ifSub, { condition: 'block', block: 'stone', pos: '~ ~ ~' }, 'entity'),
        ).toEqual({ condition: 'entity' })
    })

    it('drops values the new condition has no field for', () => {
        expect(
            switchVariant(ifSub, { condition: 'entity', target: '@e', block: 'stone' }, 'block'),
        ).toEqual({ condition: 'block' })
    })

    it('carries a shared field across', () => {
        const shared: ExecuteChainSubcommand = {
            ...ifSub,
            variants: {
                ...ifSub.variants!,
                options: [
                    ifSub.variants!.options[0],
                    {
                        ...ifSub.variants!.options[0],
                        id: 'function',
                        template: 'if function {target}',
                    },
                ],
            },
        }
        expect(switchVariant(shared, { condition: 'entity', target: '@e' }, 'function')).toEqual({
            condition: 'function',
            target: '@e',
        })
    })
})

describe('migrateLegacyStep', () => {
    const lookup = new Map<string, ExecuteChainSubcommand>([['if', ifSub]])

    it('turns an old per-condition id into a condition value', () => {
        expect(
            migrateLegacyStep({ subcommandId: 'if-block', values: { block: 'stone' } }, lookup),
        ).toEqual({ subcommandId: 'if', values: { condition: 'block', block: 'stone' } })
    })

    it('leaves ordinary subcommands alone', () => {
        expect(migrateLegacyStep({ subcommandId: 'as', values: {} }, lookup)).toBeNull()
    })

    it('leaves a condition the data no longer carries alone', () => {
        expect(migrateLegacyStep({ subcommandId: 'if-items', values: {} }, lookup)).toBeNull()
    })
})
