import assert from 'node:assert/strict'
import test from 'node:test'

import { Quaternion, Vector3 } from 'three'

import type { SeedMapStructureDetails } from './advanced.ts'
import { seedMapPreviewAxes } from './preview-overlays.ts'
import {
    assembleStructureBlocks,
    structureChestBlocks,
    type StructureTemplatePack,
} from './structure-blocks.ts'
import { seedMapKnownLayoutChests } from './structure-layout.ts'

test('world axis indicators use camera orientation and positive Minecraft directions', () => {
    const axes = seedMapPreviewAxes(new Quaternion())
    assert.equal(axes.find((axis) => axis.axis === 'X')!.x, 28)
    assert.equal(axes.find((axis) => axis.axis === 'Y')!.y, -28)
    assert.equal(axes.find((axis) => axis.axis === 'Z')!.depth, 1)
    const rotated = seedMapPreviewAxes(
        new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), Math.PI / 2),
    )
    assert.ok(Math.abs(rotated.find((axis) => axis.axis === 'Z')!.x + 28) < 0.0001)
    assert.ok(Math.abs(rotated.find((axis) => axis.axis === 'X')!.depth - 1) < 0.0001)
})

test('Nether fortress chest heights follow the validated turn-room rule and exclude unknown heights', () => {
    const details: SeedMapStructureDetails = {
        supported: true,
        truncated: false,
        limits: ['relative_height'],
        pieces: [
            {
                id: 1,
                name: 'NeSCLT',
                rotation: 0,
                positionKnown: true,
                position: { x: -10, y: 64, z: -20 },
                min: { x: -11, y: 64, z: -24 },
                max: { x: -6, y: 70, z: -20 },
            },
            {
                id: 2,
                name: 'NeSCRT',
                rotation: 90,
                positionKnown: true,
                position: { x: -30, y: 72, z: -40 },
                min: { x: -30, y: 72, z: -41 },
                max: { x: -26, y: 78, z: -37 },
            },
            {
                id: 3,
                name: 'NeBS',
                rotation: 0,
                positionKnown: true,
                position: { x: 0, y: 0, z: 0 },
            },
        ],
        containers: [
            { id: 4, piece: 1, x: -8, z: -18, table: 'chests/nether_bridge' },
            { id: 8, piece: 2, x: -33, z: -39, table: 'minecraft:chests/nether_bridge' },
            { id: 12, piece: 3, x: 0, z: 0, table: 'chests/nether_bridge' },
        ],
    }
    assert.deepEqual(
        seedMapKnownLayoutChests(details, 'fortress').map((chest) => chest.position),
        [
            [-8, 66, -18],
            [-33, 74, -39],
        ],
    )
    assert.deepEqual(seedMapKnownLayoutChests(details, 'stronghold'), [])
    assert.deepEqual(seedMapKnownLayoutChests(null, 'fortress'), [])
})

test('chest markers use final template blocks after air processing, mirror and rotation', () => {
    const pack: StructureTemplatePack = {
        schemaVersion: 2,
        version: '26.3',
        dataVersion: 5023,
        resources: { blockDefinitions: {}, blockModels: {}, defaultBlockProperties: {} },
        textureColors: {},
        templates: {
            'end_city/base_floor': {
                size: [3, 1, 1],
                overwriteAir: true,
                palette: [
                    { name: 'minecraft:chest', properties: {} },
                    { name: 'minecraft:trapped_chest', properties: {} },
                    { name: 'minecraft:ender_chest', properties: {} },
                ],
                blocks: [
                    [0, 0, 0, 0],
                    [1, 0, 0, 1],
                    [2, 0, 0, 2],
                ],
            },
            'end_city/base_roof': {
                size: [1, 1, 1],
                overwriteAir: true,
                palette: [{ name: 'minecraft:air', properties: {} }],
                blocks: [[0, 0, 0, 0]],
            },
        },
    }
    const pieces = [
        {
            id: 0,
            name: 'base_floor',
            rotation: 90,
            mirror: true,
            positionKnown: true,
            position: { x: -100, y: 70, z: 20 },
        },
        {
            id: 1,
            name: 'base_roof',
            rotation: 0,
            positionKnown: true,
            position: { x: -100, y: 70, z: 20 },
        },
    ]
    const chests = structureChestBlocks(assembleStructureBlocks(pieces, pack))
    assert.deepEqual(
        chests.map((chest) => chest.position),
        [[-100, 70, 19]],
    )
    assert.ok(chests.every((chest) => chest.pieceId === 0))
    assert.deepEqual(
        chests.map((chest) => chest.name),
        ['minecraft:trapped_chest'],
    )
})
