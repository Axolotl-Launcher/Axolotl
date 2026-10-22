import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { gunzipSync } from 'node:zlib'

import type { SeedMapStructureDetails, SeedMapStructurePiece } from './advanced.ts'
import {
    assembleStructureBlocks,
    END_CITY_TEMPLATE_NAMES,
    rotateStructurePosition,
    seedMapStructureResourceVersion,
    seedMapTemplatePieces,
    type StructureTemplatePack,
    supportsStructureBlocks,
} from './structure-blocks.ts'

const pack = JSON.parse(
    gunzipSync(
        readFileSync(new URL('./assets/versions/26.3/structures.json.gz', import.meta.url)),
    ).toString(),
) as StructureTemplatePack
const piece = (name: string, rotation = 0, id = 0): SeedMapStructurePiece => ({
    id,
    name,
    rotation,
    positionKnown: true,
    position: { x: -30_000_000, y: 0, z: 29_999_900 },
})

test('bundled original 26.3 templates include every engine End city piece and ship', () => {
    assert.equal(pack.schemaVersion, 2)
    assert.equal(pack.version, '26.3')
    assert.equal(pack.dataVersion, 5023)
    assert.equal(Object.keys(pack.templates).length, 139)
    for (const name of END_CITY_TEMPLATE_NAMES) {
        const template = pack.templates[`end_city/${name}`]
        assert.ok(template)
        assert.ok(template.blocks.length)
        for (const [x, y, z, state] of template.blocks) {
            assert.ok([x, y, z].every((value, axis) => value >= 0 && value < template.size[axis]))
            assert.ok(template.palette[state].name.startsWith('minecraft:'))
        }
    }
    assert.deepEqual(pack.templates['end_city/ship'].size, [13, 24, 29])
    assert.equal(supportsStructureBlocks('26.3', 'end-city'), true)
    assert.equal(supportsStructureBlocks('26.3', 'ancient-city'), true)
    assert.equal(supportsStructureBlocks('26.2', 'end-city'), true)
    assert.equal(supportsStructureBlocks('26.3', 'fortress'), false)
    assert.notDeepEqual(
        pack.textureColors['minecraft:block/purpur_block'],
        pack.textureColors['minecraft:block/end_stone_bricks'],
    )
    assert.ok(
        pack.textureColors['minecraft:block/purpur_block'][0] >
            pack.textureColors['minecraft:block/purpur_block'][1],
    )
    assert.ok(
        pack.textureColors['minecraft:block/end_stone_bricks'][1] >
            pack.textureColors['minecraft:block/end_stone_bricks'][2],
    )
})

test('historical template resources resolve engine aliases and preserve version capabilities', () => {
    for (const version of ['1.9.4', '1.10.2', '1.12.2', '1.13.2', '1.17.1', '1.19.2', '26.2']) {
        assert.equal(supportsStructureBlocks(version, 'end-city'), true, version)
    }
    assert.equal(seedMapStructureResourceVersion('1.20'), '1.20.6')
    assert.equal(supportsStructureBlocks('1.20', 'ancient-city'), true)
    assert.equal(supportsStructureBlocks('1.18.2', 'ancient-city'), false)
    assert.equal(supportsStructureBlocks('b1.7', 'end-city'), false)
})

test('every bundled version has intact assets and versioned ancient city layout, blocks and loot', () => {
    const catalog = JSON.parse(
        readFileSync(new URL('./assets/catalog.json', import.meta.url), 'utf8'),
    )
    for (const version of Object.keys(catalog.versions)) {
        const source = seedMapStructureResourceVersion(version)
        assert.ok(catalog.versions[source] && !catalog.versions[source].resourceVersion, version)
        if (source !== version) continue
        const read = (name: string) =>
            readFileSync(new URL(`./assets/versions/${source}/${name}`, import.meta.url))
        const bytes = read('structures.json.gz')
        const provenance = JSON.parse(read('sources.json').toString())
        assert.equal(
            createHash('sha256').update(bytes).digest('hex'),
            provenance.assetSha256,
            source,
        )
        const resource = JSON.parse(gunzipSync(bytes).toString()) as StructureTemplatePack
        assert.equal(resource.version, source)
        assert.equal(resource.dataVersion, catalog.versions[source].dataVersion)
        assert.deepEqual(
            Object.keys(resource.templates),
            Object.keys(catalog.versions[source].templates),
        )
        for (const [id, template] of Object.entries(resource.templates)) {
            for (const [x, y, z, state] of template.blocks) {
                assert.ok(
                    [x, y, z].every((value, axis) => value >= 0 && value < template.size[axis]),
                    `${source}: ${id}`,
                )
                assert.ok(
                    template.palette[state]?.name.startsWith('minecraft:'),
                    `${source}: ${id}: ${state}`,
                )
            }
        }
        if (!supportsStructureBlocks(source, 'ancient-city')) continue
        const layout = JSON.parse(gunzipSync(read('ancient-city-layout.json.gz')).toString()) as {
            pools: Record<
                string,
                { elements: { element: { element_type: string; location: string } }[] }
            >
            templates: Record<string, unknown>
            missingTemplates: string[]
        }
        const missing = [
            ...new Set(
                Object.values(layout.pools).flatMap((pool) =>
                    pool.elements.flatMap(({ element }) =>
                        element.element_type === 'minecraft:single_pool_element' &&
                        !layout.templates[element.location]
                            ? [element.location]
                            : [],
                    ),
                ),
            ),
        ].sort()
        assert.deepEqual(layout.missingTemplates, missing, source)
        const nativeRoot = new URL(
            `../../../../app/src/seed_map/assets/versions/${source}/`,
            import.meta.url,
        )
        const blocksBytes = readFileSync(new URL('ancient-city-blocks.json.gz', nativeRoot))
        assert.equal(
            createHash('sha256').update(blocksBytes).digest('hex'),
            provenance.ancientCityBlocksSha256,
            source,
        )
        assert.deepEqual(
            readFileSync(new URL('ancient-city-layout.json.gz', nativeRoot)),
            read('ancient-city-layout.json.gz'),
        )
        const native = JSON.parse(gunzipSync(blocksBytes).toString())
        assert.equal(native.version, source)
        for (const table of ['chests/ancient_city', 'chests/ancient_city_ice_box']) {
            assert.ok(native.lootTables[table]?.pools.length > 0, `${source}: ${table}`)
        }
    }
})

test('all four rotations preserve block count, negative/far coordinates and template origin', () => {
    const original = assembleStructureBlocks([piece('ship')], pack)
    assert.ok(original.size > 500)
    for (const rotation of [0, 90, 180, 270]) {
        const rotated = assembleStructureBlocks([piece('ship', rotation)], pack)
        assert.equal(rotated.size, original.size)
        for (const block of original.values()) {
            const offset = rotateStructurePosition(
                [block.position[0] + 30_000_000, block.position[1], block.position[2] - 29_999_900],
                rotation,
            )
            assert.ok(
                rotated.has([-30_000_000 + offset[0], offset[1], 29_999_900 + offset[2]].join(',')),
            )
            assert.notEqual(block.state.name, 'minecraft:structure_block')
        }
    }
    assert.deepEqual(rotateStructurePosition([5, 4, 7], 90), [-7, 4, 5])
})

test('piece order, air overwrite and ignored structure markers retain Minecraft placement semantics', () => {
    const data: StructureTemplatePack = {
        schemaVersion: 2,
        resources: pack.resources,
        textureColors: pack.textureColors,
        version: '26.3',
        dataVersion: 5023,
        templates: {
            'end_city/base_floor': {
                size: [3, 1, 1],
                overwriteAir: true,
                palette: [{ name: 'minecraft:purpur_block', properties: {} }],
                blocks: [
                    [0, 0, 0, 0],
                    [1, 0, 0, 0],
                    [2, 0, 0, 0],
                ],
            },
            'end_city/second_floor_1': {
                size: [3, 1, 1],
                overwriteAir: false,
                palette: [
                    { name: 'minecraft:air', properties: {} },
                    { name: 'minecraft:structure_block', properties: {} },
                    { name: 'minecraft:structure_void', properties: {} },
                ],
                blocks: [
                    [0, 0, 0, 0],
                    [1, 0, 0, 1],
                    [2, 0, 0, 2],
                ],
            },
        },
    }
    const pieces = [piece('base_floor'), piece('second_floor_1', 0, 1)]
    assert.equal(assembleStructureBlocks(pieces, data).size, 3)
    data.templates['end_city/second_floor_1'].overwriteAir = true
    const blocks = assembleStructureBlocks(pieces, data)
    assert.equal(blocks.size, 2)
    assert.ok([...blocks.values()].every((block) => block.pieceId === 0))
    assert.equal(assembleStructureBlocks(pieces.toReversed(), data).size, 3)
})

test('rejects unsupported pieces, versions, rotations, unknown positions and oversized requests', () => {
    assert.throws(() => assembleStructureBlocks([piece('unknown')], pack))
    assert.throws(() => assembleStructureBlocks([piece('ship', 45)], pack))
    assert.throws(() => assembleStructureBlocks([{ ...piece('ship'), positionKnown: false }], pack))
    assert.throws(() => assembleStructureBlocks([piece('ship')], { ...pack, version: '26.2' }))
    assert.throws(() =>
        assembleStructureBlocks(
            Array.from({ length: 2049 }, () => piece('ship')),
            pack,
        ),
    )
})

const variant: NonNullable<SeedMapStructureDetails['variant']> = {
    rotation: 0,
    mirror: 0,
    start: 1,
    biome: 1,
    abandoned: false,
    giant: false,
    underground: false,
    airPocket: false,
    basement: false,
    cracked: false,
    size: 4,
    offset: { x: 3, y: 62, z: 5 },
    size3d: { x: 7, y: 5, z: 8 },
}
const details = (
    pieces: SeedMapStructurePiece[] = [],
    value = variant,
): SeedMapStructureDetails => ({
    supported: true,
    truncated: false,
    containers: [],
    limits: [],
    pieces,
    variant: value,
})
const query = { version: '26.3', x: -17, z: 33 }

test('all bundled template sizes and palette indices are valid, including eight shipwreck palettes', () => {
    for (const template of Object.values(pack.templates)) {
        for (const palette of template.alternativePalettes ?? [template.palette]) {
            for (const [x, y, z, index] of template.blocks) {
                assert.ok(
                    [x, y, z].every((value, axis) => value >= 0 && value < template.size[axis]),
                )
                assert.ok(palette[index].name.startsWith('minecraft:'))
            }
        }
    }
    assert.equal(pack.templates['shipwreck/rightsideup_full'].alternativePalettes!.length, 8)
})

test('shipwreck template origins and rotations are preserved and all 20 variants assemble', () => {
    const names = Object.keys(pack.templates).filter((name) => name.startsWith('shipwreck/'))
    assert.equal(names.length, 20)
    for (const name of names)
        for (const rotation of [0, 90, 180, 270]) {
            const source = piece(name, rotation)
            const placements = seedMapTemplatePieces(details([source]), query, 'shipwreck')
            assert.equal(placements.length, 1)
            assert.deepEqual(placements[0].position, source.position)
            const blocks = assembleStructureBlocks(placements, pack)
            assert.ok(blocks.size > 20)
            for (const block of blocks.values())
                for (const axis of ['x', 'y', 'z'] as const) {
                    const i = ['x', 'y', 'z'].indexOf(axis)
                    assert.ok(
                        block.position[i] >= placements[0].min![axis] &&
                            block.position[i] <= placements[0].max![axis],
                    )
                }
        }
})

test('igloo pivots keep the shaft connected for every encoded rotation and basement depth', () => {
    const top = { ...piece('igloo/top'), position: { x: -32, y: 90, z: 32 } }
    for (const rotation of [0, 90])
        for (const mirror of [0, 1])
            for (const size of [4, 11]) {
                const placements = seedMapTemplatePieces(
                    details([top, { ...top, id: size + 1, name: 'igloo/bottom' }], {
                        ...variant,
                        rotation,
                        mirror,
                        basement: true,
                        size,
                    }),
                    query,
                    'igloo',
                )
                assert.equal(placements.length, size + 1)
                assert.ok(assembleStructureBlocks(placements, pack).size > 200)
                const actualRotation = rotation + mirror * 180
                const opening = (
                    placement: SeedMapStructurePiece,
                    local: [number, number, number],
                ) => {
                    const p = rotateStructurePosition(local, actualRotation)
                    return [p[0] + placement.position.x, p[2] + placement.position.z]
                }
                const topPlacement = placements.find((p) => p.templateId === 'igloo/top')!
                const bottomPlacement = placements.find((p) => p.templateId === 'igloo/bottom')!
                const topOpening = opening(topPlacement, [3, 0, 5])
                assert.deepEqual(opening(bottomPlacement, [3, 0, 7]), topOpening)
                for (const middle of placements.filter((p) => p.templateId === 'igloo/middle'))
                    assert.deepEqual(opening(middle, [1, 0, 1]), topOpening)
                assert.equal(bottomPlacement.position.y, 90 - 3 - size * 3)
            }
    assert.equal(seedMapTemplatePieces(details([top]), query, 'igloo').length, 1)
})

test('ruined portal mirrors and all fossil variants produce bounded template blocks at negative chunk positions', () => {
    for (const rotation of [0, 90, 180, 270])
        for (const mirror of [0, 1]) {
            const placements = seedMapTemplatePieces(
                details([], { ...variant, rotation, mirror }),
                query,
                'ruined-portal',
            )
            assert.equal(placements.length, 1)
            const blocks = assembleStructureBlocks(placements, pack)
            for (const block of blocks.values()) {
                assert.ok(
                    block.position[0] >= placements[0].min!.x &&
                        block.position[0] <= placements[0].max!.x,
                )
                assert.ok(
                    block.position[2] >= placements[0].min!.z &&
                        block.position[2] <= placements[0].max!.z,
                )
            }
        }
    for (let start = 0; start < 14; start++) {
        const placements = seedMapTemplatePieces(
            details([], { ...variant, start }),
            query,
            'nether-fossil',
        )
        assert.equal(placements.length, 1)
        assert.deepEqual(placements[0].position, { x: -29, y: 62, z: 37 })
        assert.ok(assembleStructureBlocks(placements, pack).size > 0)
    }
})

test('unknown position structures and procedural structures are not advertised as template predictions', () => {
    for (const kind of ['bastion', 'abandoned-camp', 'stronghold', 'fortress', 'desert-pyramid'])
        assert.deepEqual(seedMapTemplatePieces(details([piece('unknown')]), query, kind), [])
    assert.deepEqual(
        seedMapTemplatePieces(
            details([piece('shipwreck/rightsideup_full')]),
            { ...query, version: '1.12.2' },
            'shipwreck',
        ),
        [],
    )
    const tower = seedMapTemplatePieces(
        details([{ ...piece('pillager_outpost/watchtower'), rotation: 0 }], {
            ...variant,
            rotation: 270,
        }),
        query,
        'outpost',
    )
    assert.equal(tower.length, 1)
    assert.equal(tower[0].rotation, 270)
})
