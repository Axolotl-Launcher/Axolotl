import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { gunzipSync } from 'node:zlib'

import { expect, it } from 'vitest'

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

it('bundled original 26.3 templates include every engine End city piece and ship', () => {
    expect(pack.schemaVersion).toBe(2)
    expect(pack.version).toBe('26.3')
    expect(pack.dataVersion).toBe(5023)
    expect(Object.keys(pack.templates).length).toBe(139)
    for (const name of END_CITY_TEMPLATE_NAMES) {
        const template = pack.templates[`end_city/${name}`]
        expect(template).toBeTruthy()
        expect(template.blocks.length).toBeTruthy()
        for (const [x, y, z, state] of template.blocks) {
            expect(
                [x, y, z].every((value, axis) => value >= 0 && value < template.size[axis]),
            ).toBeTruthy()
            expect(template.palette[state].name.startsWith('minecraft:')).toBeTruthy()
        }
    }
    expect(pack.templates['end_city/ship'].size).toEqual([13, 24, 29])
    expect(supportsStructureBlocks('26.3', 'end-city')).toBe(true)
    expect(supportsStructureBlocks('26.3', 'ancient-city')).toBe(true)
    expect(supportsStructureBlocks('26.2', 'end-city')).toBe(true)
    expect(supportsStructureBlocks('26.3', 'fortress')).toBe(false)
    expect(pack.textureColors['minecraft:block/purpur_block']).not.toEqual(
        pack.textureColors['minecraft:block/end_stone_bricks'],
    )
    expect(
        pack.textureColors['minecraft:block/purpur_block'][0] >
            pack.textureColors['minecraft:block/purpur_block'][1],
    ).toBeTruthy()
    expect(
        pack.textureColors['minecraft:block/end_stone_bricks'][1] >
            pack.textureColors['minecraft:block/end_stone_bricks'][2],
    ).toBeTruthy()
})

it('historical template resources resolve engine aliases and preserve version capabilities', () => {
    for (const version of ['1.9.4', '1.10.2', '1.12.2', '1.13.2', '1.17.1', '1.19.2', '26.2']) {
        expect(supportsStructureBlocks(version, 'end-city'), version).toBe(true)
    }
    expect(seedMapStructureResourceVersion('1.20')).toBe('1.20.6')
    expect(supportsStructureBlocks('1.20', 'ancient-city')).toBe(true)
    expect(supportsStructureBlocks('1.18.2', 'ancient-city')).toBe(false)
    expect(supportsStructureBlocks('b1.7', 'end-city')).toBe(false)
})

it('every bundled version has intact assets and versioned ancient city layout, blocks and loot', () => {
    const catalog = JSON.parse(
        readFileSync(new URL('./assets/catalog.json', import.meta.url), 'utf8'),
    )
    for (const version of Object.keys(catalog.versions)) {
        const source = seedMapStructureResourceVersion(version)
        expect(
            catalog.versions[source] && !catalog.versions[source].resourceVersion,
            version,
        ).toBeTruthy()
        if (source !== version) continue
        const read = (name: string) =>
            readFileSync(new URL(`./assets/versions/${source}/${name}`, import.meta.url))
        const bytes = read('structures.json.gz')
        const provenance = JSON.parse(read('sources.json').toString())
        expect(createHash('sha256').update(bytes).digest('hex'), source).toBe(
            provenance.assetSha256,
        )
        const resource = JSON.parse(gunzipSync(bytes).toString()) as StructureTemplatePack
        expect(resource.version).toBe(source)
        expect(resource.dataVersion).toBe(catalog.versions[source].dataVersion)
        expect(Object.keys(resource.templates)).toEqual(
            Object.keys(catalog.versions[source].templates),
        )
        for (const [id, template] of Object.entries(resource.templates)) {
            for (const [x, y, z, state] of template.blocks) {
                expect(
                    [x, y, z].every((value, axis) => value >= 0 && value < template.size[axis]),
                    `${source}: ${id}`,
                ).toBeTruthy()
                expect(
                    template.palette[state]?.name.startsWith('minecraft:'),
                    `${source}: ${id}: ${state}`,
                ).toBeTruthy()
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
        expect(layout.missingTemplates, source).toEqual(missing)
        const nativeRoot = new URL(
            `../../../../app/src/seed_map/assets/versions/${source}/`,
            import.meta.url,
        )
        const blocksBytes = readFileSync(new URL('ancient-city-blocks.json.gz', nativeRoot))
        expect(createHash('sha256').update(blocksBytes).digest('hex'), source).toBe(
            provenance.ancientCityBlocksSha256,
        )
        expect(readFileSync(new URL('ancient-city-layout.json.gz', nativeRoot))).toEqual(
            read('ancient-city-layout.json.gz'),
        )
        const native = JSON.parse(gunzipSync(blocksBytes).toString())
        expect(native.version).toBe(source)
        for (const table of ['chests/ancient_city', 'chests/ancient_city_ice_box']) {
            expect(native.lootTables[table]?.pools.length > 0, `${source}: ${table}`).toBeTruthy()
        }
    }
})

it('all four rotations preserve block count, negative/far coordinates and template origin', () => {
    const original = assembleStructureBlocks([piece('ship')], pack)
    expect(original.size > 500).toBeTruthy()
    for (const rotation of [0, 90, 180, 270]) {
        const rotated = assembleStructureBlocks([piece('ship', rotation)], pack)
        expect(rotated.size).toBe(original.size)
        for (const block of original.values()) {
            const offset = rotateStructurePosition(
                [block.position[0] + 30_000_000, block.position[1], block.position[2] - 29_999_900],
                rotation,
            )
            expect(
                rotated.has([-30_000_000 + offset[0], offset[1], 29_999_900 + offset[2]].join(',')),
            ).toBeTruthy()
            expect(block.state.name).not.toBe('minecraft:structure_block')
        }
    }
    expect(rotateStructurePosition([5, 4, 7], 90)).toEqual([-7, 4, 5])
})

it('piece order, air overwrite and ignored structure markers retain Minecraft placement semantics', () => {
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
    expect(assembleStructureBlocks(pieces, data).size).toBe(3)
    data.templates['end_city/second_floor_1'].overwriteAir = true
    const blocks = assembleStructureBlocks(pieces, data)
    expect(blocks.size).toBe(2)
    expect([...blocks.values()].every((block) => block.pieceId === 0)).toBeTruthy()
    expect(assembleStructureBlocks(pieces.toReversed(), data).size).toBe(3)
})

it('rejects unsupported pieces, versions, rotations, unknown positions and oversized requests', () => {
    expect(() => assembleStructureBlocks([piece('unknown')], pack)).toThrow()
    expect(() => assembleStructureBlocks([piece('ship', 45)], pack)).toThrow()
    expect(() =>
        assembleStructureBlocks([{ ...piece('ship'), positionKnown: false }], pack),
    ).toThrow()
    expect(() => assembleStructureBlocks([piece('ship')], { ...pack, version: '26.2' })).toThrow()
    expect(() =>
        assembleStructureBlocks(
            Array.from({ length: 2049 }, () => piece('ship')),
            pack,
        ),
    ).toThrow()
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

it('all bundled template sizes and palette indices are valid, including eight shipwreck palettes', () => {
    for (const template of Object.values(pack.templates)) {
        for (const palette of template.alternativePalettes ?? [template.palette]) {
            for (const [x, y, z, index] of template.blocks) {
                expect(
                    [x, y, z].every((value, axis) => value >= 0 && value < template.size[axis]),
                ).toBeTruthy()
                expect(palette[index].name.startsWith('minecraft:')).toBeTruthy()
            }
        }
    }
    expect(pack.templates['shipwreck/rightsideup_full'].alternativePalettes!.length).toBe(8)
})

it('shipwreck template origins and rotations are preserved and all 20 variants assemble', () => {
    const names = Object.keys(pack.templates).filter((name) => name.startsWith('shipwreck/'))
    expect(names.length).toBe(20)
    for (const name of names)
        for (const rotation of [0, 90, 180, 270]) {
            const source = piece(name, rotation)
            const placements = seedMapTemplatePieces(details([source]), query, 'shipwreck')
            expect(placements.length).toBe(1)
            expect(placements[0].position).toEqual(source.position)
            const blocks = assembleStructureBlocks(placements, pack)
            expect(blocks.size > 20).toBeTruthy()
            for (const block of blocks.values())
                for (const axis of ['x', 'y', 'z'] as const) {
                    const i = ['x', 'y', 'z'].indexOf(axis)
                    expect(
                        block.position[i] >= placements[0].min![axis] &&
                            block.position[i] <= placements[0].max![axis],
                    ).toBeTruthy()
                }
        }
})

it('igloo pivots keep the shaft connected for every encoded rotation and basement depth', () => {
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
                expect(placements.length).toBe(size + 1)
                expect(assembleStructureBlocks(placements, pack).size > 200).toBeTruthy()
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
                expect(opening(bottomPlacement, [3, 0, 7])).toEqual(topOpening)
                for (const middle of placements.filter((p) => p.templateId === 'igloo/middle'))
                    expect(opening(middle, [1, 0, 1])).toEqual(topOpening)
                expect(bottomPlacement.position.y).toBe(90 - 3 - size * 3)
            }
    expect(seedMapTemplatePieces(details([top]), query, 'igloo').length).toBe(1)
})

it('ruined portal mirrors and all fossil variants produce bounded template blocks at negative chunk positions', () => {
    for (const rotation of [0, 90, 180, 270])
        for (const mirror of [0, 1]) {
            const placements = seedMapTemplatePieces(
                details([], { ...variant, rotation, mirror }),
                query,
                'ruined-portal',
            )
            expect(placements.length).toBe(1)
            const blocks = assembleStructureBlocks(placements, pack)
            for (const block of blocks.values()) {
                expect(
                    block.position[0] >= placements[0].min!.x &&
                        block.position[0] <= placements[0].max!.x,
                ).toBeTruthy()
                expect(
                    block.position[2] >= placements[0].min!.z &&
                        block.position[2] <= placements[0].max!.z,
                ).toBeTruthy()
            }
        }
    for (let start = 0; start < 14; start++) {
        const placements = seedMapTemplatePieces(
            details([], { ...variant, start }),
            query,
            'nether-fossil',
        )
        expect(placements.length).toBe(1)
        expect(placements[0].position).toEqual({ x: -29, y: 62, z: 37 })
        expect(assembleStructureBlocks(placements, pack).size > 0).toBeTruthy()
    }
})

it('unknown position structures and procedural structures are not advertised as template predictions', () => {
    for (const kind of ['bastion', 'abandoned-camp', 'stronghold', 'fortress', 'desert-pyramid'])
        expect(seedMapTemplatePieces(details([piece('unknown')]), query, kind)).toEqual([])
    expect(
        seedMapTemplatePieces(
            details([piece('shipwreck/rightsideup_full')]),
            { ...query, version: '1.12.2' },
            'shipwreck',
        ),
    ).toEqual([])
    const tower = seedMapTemplatePieces(
        details([{ ...piece('pillager_outpost/watchtower'), rotation: 0 }], {
            ...variant,
            rotation: 270,
        }),
        query,
        'outpost',
    )
    expect(tower.length).toBe(1)
    expect(tower[0].rotation).toBe(270)
})
