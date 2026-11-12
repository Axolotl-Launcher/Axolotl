import { expect, it } from 'vitest'

import { collectSchematicStateValues, schematicStateProperties } from './block-state.ts'

const stairsDefinition = {
    variants: {
        'facing=east,half=bottom,shape=inner_left': { model: 'minecraft:block/oak_stairs_inner' },
        'facing=east,half=bottom,shape=straight': { model: 'minecraft:block/oak_stairs' },
        'facing=east,half=top,shape=outer_right': { model: 'minecraft:block/oak_stairs_outer' },
        'facing=north,half=bottom,shape=straight': { model: 'minecraft:block/oak_stairs' },
        'facing=south,half=bottom,shape=straight': { model: 'minecraft:block/oak_stairs' },
        'facing=west,half=bottom,shape=straight': { model: 'minecraft:block/oak_stairs' },
    },
}

it('derives ordered alternatives from the variants of the block', () => {
    const properties = schematicStateProperties(
        {
            name: 'minecraft:oak_stairs',
            properties: {
                facing: 'north',
                half: 'bottom',
                shape: 'straight',
                waterlogged: 'false',
            },
        },
        stairsDefinition,
    )

    expect(properties.map((property) => property.key)).toEqual([
        'facing',
        'half',
        'shape',
        'waterlogged',
    ])
    expect(properties[0]).toEqual({
        key: 'facing',
        value: 'north',
        boolean: false,
        values: ['north', 'east', 'south', 'west'],
    })
    expect(properties[1].values).toEqual(['bottom', 'top'])
    expect(properties[2].values).toEqual(['straight', 'inner_left', 'outer_right'])
    expect(properties[3]).toEqual({
        key: 'waterlogged',
        value: 'false',
        boolean: true,
        values: ['false', 'true'],
    })
})

it('reads multipart conditions including alternatives and nested logic', () => {
    const definition = {
        multipart: [
            {
                apply: { model: 'minecraft:block/redstone_dust_dot' },
                when: { OR: [{ east: 'none' }, { east: 'side|up', north: 'side|up' }] },
            },
            { apply: { model: 'minecraft:block/redstone_dust_side0' }, when: { north: 'side|up' } },
        ],
    }

    const properties = schematicStateProperties(
        {
            name: 'minecraft:redstone_wire',
            properties: { east: 'none', north: 'side', power: '0' },
        },
        definition,
    )

    expect(properties.find((property) => property.key === 'east')?.values).toEqual([
        'none',
        'side',
        'up',
    ])
    expect(properties.find((property) => property.key === 'north')?.values).toEqual(['side', 'up'])
    expect(properties.find((property) => property.key === 'power')?.values).toEqual(['0'])
})

it('accepts the string form of a multipart condition', () => {
    const values = collectSchematicStateValues({
        multipart: [{ apply: {}, when: 'north=true,waterlogged=false' }],
    })

    expect([...(values.get('north') ?? [])]).toEqual(['true'])
    expect([...(values.get('waterlogged') ?? [])]).toEqual(['false'])
})

it('orders numeric alternatives numerically instead of lexically', () => {
    const properties = schematicStateProperties(
        { name: 'minecraft:oak_sign', properties: { rotation: '0' } },
        {
            variants: {
                'rotation=0': {},
                'rotation=1': {},
                'rotation=10': {},
                'rotation=11': {},
                'rotation=2': {},
            },
        },
    )

    expect(properties[0].values).toEqual(['0', '1', '2', '10', '11'])
})

it('keeps a property read-only when the block definition cannot describe it', () => {
    expect(
        schematicStateProperties(
            { name: 'minecraft:chest', properties: { facing: 'north', type: 'single' } },
            undefined,
        ),
    ).toEqual([
        { key: 'facing', value: 'north', boolean: false, values: ['north'] },
        { key: 'type', value: 'single', boolean: false, values: ['single'] },
    ])
})

it('keeps the current value when a definition omits it', () => {
    const properties = schematicStateProperties(
        { name: 'minecraft:oak_stairs', properties: { facing: 'up', half: 'top' } },
        stairsDefinition,
    )

    expect(properties[0].values).toEqual(['north', 'east', 'south', 'west', 'up'])
})
