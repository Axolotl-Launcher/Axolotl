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
        layout: 'card',
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

/** 一个家族子命令：`positioned` 把坐标、目标、高度图三种写法收在一处。 */
const positionedSub: ExecuteChainSubcommand = {
    id: 'positioned',
    labelKey: 'positioned.label',
    descriptionKey: 'positioned.desc',
    template: '',
    arguments: [],
    variants: {
        argument: 'positioning',
        labelKey: 'variant.field.positioning',
        layout: 'compact',
        options: [
            {
                id: 'pos',
                legacy: [{ id: 'positioned' }],
                labelKey: 'form.positioned-pos.label',
                descriptionKey: 'positioned.desc',
                template: 'positioned {pos}',
                arguments: [
                    {
                        name: 'pos',
                        displayNameKey: 'pos.display',
                        type: 'coordinates',
                        required: true,
                        descriptionKey: 'pos.desc',
                    },
                ],
            },
            {
                id: 'as',
                legacy: [{ id: 'positioned-as' }],
                labelKey: 'form.positioned-as.label',
                descriptionKey: 'positioned-as.desc',
                template: 'positioned as {target}',
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
        ],
    },
}

/** 一个带子命令级参数的家族：`store` 的 `mode` 与写法无关。 */
const storeSub: ExecuteChainSubcommand = {
    id: 'store',
    labelKey: 'store.label',
    descriptionKey: 'store.desc',
    template: '',
    arguments: [
        {
            name: 'mode',
            displayNameKey: 'mode.display',
            type: 'enum',
            required: true,
            descriptionKey: 'mode.desc',
            enumValues: ['result', 'success'],
            defaultValue: 'result',
        },
    ],
    variants: {
        argument: 'destination',
        labelKey: 'variant.field.destination',
        layout: 'card',
        options: [
            {
                id: 'score',
                legacy: [
                    { id: 'store-result-score', values: { mode: 'result' } },
                    { id: 'store-success-score', values: { mode: 'success' } },
                ],
                labelKey: 'form.store-score.label',
                descriptionKey: 'store-score.desc',
                template: 'store {mode} score {target} {objective}',
                arguments: [
                    {
                        name: 'target',
                        displayNameKey: 'target.display',
                        type: 'entity-selector',
                        required: true,
                        descriptionKey: 'target.desc',
                    },
                    {
                        name: 'objective',
                        displayNameKey: 'objective.display',
                        type: 'objective',
                        required: true,
                        descriptionKey: 'objective.desc',
                    },
                ],
            },
            {
                id: 'block',
                legacy: [
                    { id: 'store-result-block', values: { mode: 'result' } },
                    { id: 'store-success-block', values: { mode: 'success' } },
                ],
                labelKey: 'form.store-block.label',
                descriptionKey: 'store-block.desc',
                template: 'store {mode} block {pos} {path} {type} {scale}',
                arguments: [
                    {
                        name: 'pos',
                        displayNameKey: 'pos.display',
                        type: 'coordinates',
                        required: true,
                        descriptionKey: 'pos.desc',
                    },
                ],
            },
        ],
    },
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

    it('put the subcommand-level arguments before the form ones', () => {
        expect(stepArguments(storeSub, { destination: 'score' }).map((arg) => arg.name)).toEqual([
            'mode',
            'target',
            'objective',
        ])
        expect(stepTemplate(storeSub, { destination: 'score' })).toBe(
            'store {mode} score {target} {objective}',
        )
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
            switchVariant(
                ifSub,
                { condition: 'entity', target: '@e', biome: 'minecraft:plains' },
                'block',
            ),
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

    it('keeps a subcommand-level argument such as store mode', () => {
        expect(
            switchVariant(
                storeSub,
                { destination: 'score', mode: 'success', target: '@s' },
                'block',
            ),
        ).toEqual({ mode: 'success', destination: 'block' })
    })
})

describe('migrateLegacyStep', () => {
    const lookup = new Map<string, ExecuteChainSubcommand>([
        ['if', ifSub],
        ['positioned', positionedSub],
        ['store', storeSub],
    ])

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

    it('reads the legacy ids a family declares, including the bare command', () => {
        expect(
            migrateLegacyStep({ subcommandId: 'positioned', values: { pos: '~ ~ ~' } }, lookup),
        ).toEqual({ subcommandId: 'positioned', values: { positioning: 'pos', pos: '~ ~ ~' } })
        expect(
            migrateLegacyStep({ subcommandId: 'positioned-as', values: { target: '@s' } }, lookup),
        ).toEqual({ subcommandId: 'positioned', values: { positioning: 'as', target: '@s' } })
    })

    it('leaves an already migrated family step alone', () => {
        expect(
            migrateLegacyStep(
                { subcommandId: 'positioned', values: { positioning: 'as', target: '@s' } },
                lookup,
            ),
        ).toBeNull()
    })

    it('restores the fields an old id had folded into itself', () => {
        expect(
            migrateLegacyStep(
                { subcommandId: 'store-success-block', values: { pos: '~ ~ ~' } },
                lookup,
            ),
        ).toEqual({
            subcommandId: 'store',
            values: { pos: '~ ~ ~', mode: 'success', destination: 'block' },
        })
    })
})
