import type { SchematicMeshData } from '../schematic-preview/mesh-worker'
import type { UndergroundLayer, UndergroundMesh, UndergroundMeshRequest } from './underground'

const oreTextures = [
    'diamond_ore',
    'iron_ore',
    'iron_ore',
    'copper_ore',
    'copper_ore',
    'gold_ore',
    'redstone_ore',
    'lapis_ore',
    'coal_ore',
    'ancient_debris_side',
    'emerald_ore',
    'nether_quartz_ore',
    'nether_gold_ore',
]
const baseTextures = [
    '',
    'stone',
    'deepslate',
    'netherrack',
    'water_still',
    'lava_still',
    'bedrock',
]

export function undergroundTexture(id: number, axis: number) {
    if (id === 2 && axis === 1) return 'minecraft:block/deepslate_top'
    if (id < 16) return `minecraft:block/${baseTextures[id] ?? 'stone'}`
    const deep = id >= 80
    const kind = id - 16 - (deep ? 64 : 0)
    const texture = kind === 9 && axis === 1 ? 'ancient_debris_top' : oreTextures[kind]
    return `minecraft:block/${deep ? 'deepslate_' : ''}${texture ?? 'stone'}`
}

type Buffers = { [K in keyof SchematicMeshData]: number[] }
const buffers = (): Buffers => ({
    positions: [],
    normals: [],
    colors: [],
    uvs: [],
    blockPositions: [],
})
const normalOffsets = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
]
const corners = [
    [
        [0, 0, 0],
        [0, 0, 1],
        [0, 1, 1],
        [0, 1, 0],
    ],
    [
        [1, 0, 1],
        [1, 0, 0],
        [1, 1, 0],
        [1, 1, 1],
    ],
    [
        [0, 0, 0],
        [1, 0, 0],
        [1, 0, 1],
        [0, 0, 1],
    ],
    [
        [0, 1, 1],
        [1, 1, 1],
        [1, 1, 0],
        [0, 1, 0],
    ],
    [
        [1, 0, 0],
        [0, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
    ],
    [
        [0, 0, 1],
        [1, 0, 1],
        [1, 1, 1],
        [0, 1, 1],
    ],
]

/** Mesh only cavity-facing terrain, fluid boundaries and mineral candidates; omit region cut faces. */
export function buildUndergroundMesh(request: UndergroundMeshRequest): UndergroundMesh {
    const { blocks, width, height, textureUvs } = request
    if (blocks.length !== width * width * height || width > 64 || height > 384)
        throw new Error('Invalid underground mesh bounds')
    const layers: Record<UndergroundLayer, Buffers> = {
        terrain: buffers(),
        surface: buffers(),
        bedrock: buffers(),
        water: buffers(),
        lava: buffers(),
        ores: buffers(),
    }
    const at = (x: number, y: number, z: number) => blocks[(x * width + z) * height + y]
    let faces = 0
    const focusY = Math.max(1, Math.min(height - 3, request.focusY ?? height / 2))
    let spawn: [number, number, number] = [width / 2, focusY, width / 2]
    let spawnDistance = Infinity
    const border = request.surface ? 1 : 0
    const appendFace = (
        target: Buffers,
        id: number,
        x: number,
        y: number,
        z: number,
        side: number,
    ) => {
        const normal = normalOffsets[side]
        const axis = normal[0] ? 0 : normal[1] ? 1 : 2
        const texture = undergroundTexture(id, axis)
        const uv = textureUvs[texture]
        if (!uv) throw new Error(`Missing underground texture: ${texture}`)
        faces++
        if (faces > 150_000) throw new Error('Underground mesh exceeds the face limit')
        const [u0, v0, u1, v1] = uv
        const faceUvs = [
            [u0, v1],
            [u1, v1],
            [u1, v0],
            [u0, v0],
        ]
        for (const vertex of [0, 1, 2, 0, 2, 3]) {
            const corner = corners[side][vertex]
            target.positions.push(x + corner[0], y + corner[1], z + corner[2])
            target.normals.push(...normal)
            target.uvs.push(...faceUvs[vertex])
            target.colors.push(...(id === 4 ? [0.08, 0.25, 0.7] : [1, 1, 1]))
            target.blockPositions.push(x, y, z)
        }
    }
    const surfaceAt = (x: number, z: number) => {
        const y = request.surface?.[x * width + z]
        const local = y === undefined || y === -32768 ? undefined : y - (request.surfaceOffset ?? 0)
        return local !== undefined && local >= 0 && local < height ? local : undefined
    }
    for (let x = border; x < width - border; x++)
        for (let z = border; z < width - border; z++) {
            const top = surfaceAt(x, z)
            if (top !== undefined && at(x, top, z) !== 6)
                for (let side = 0; side < 6; side++) {
                    const normal = normalOffsets[side]
                    if (side !== 2 && side !== 3 && surfaceAt(x + normal[0], z + normal[2]) === top)
                        continue
                    appendFace(layers.surface, request.nether ? 3 : 1, x, top, z, side)
                }
            for (let y = 0; y < height; y++) {
                const id = at(x, y, z)
                if (id === 0) {
                    if (y > 0 && y + 2 < height && at(x, y + 1, z) === 0 && at(x, y + 2, z) === 0) {
                        const distance =
                            (x - width / 2) ** 2 + (y - focusY) ** 2 + (z - width / 2) ** 2
                        if (distance < spawnDistance) {
                            spawnDistance = distance
                            spawn = [x + 0.5, y + 1.6, z + 0.5]
                        }
                    }
                    continue
                }
                const layer: UndergroundLayer =
                    id === 4
                        ? 'water'
                        : id === 5
                          ? 'lava'
                          : id === 6
                            ? 'bedrock'
                            : id >= 16
                              ? 'ores'
                              : 'terrain'
                const target = layers[layer]
                for (let side = 0; side < 6; side++) {
                    const normal = normalOffsets[side]
                    const [nx, ny, nz] = [x + normal[0], y + normal[1], z + normal[2]]
                    if (ny < 0 || ny >= height) {
                        if (layer === 'bedrock') appendFace(target, id, x, y, z, side)
                        continue
                    }
                    if (nx < 0 || nx >= width || nz < 0 || nz >= width) continue
                    const neighbor = at(nx, ny, nz)
                    if (y === top && layer === 'terrain') continue
                    if (layer === 'terrain' && neighbor !== 0 && neighbor !== 4 && neighbor !== 5)
                        continue
                    if (layer === 'bedrock' && neighbor === 6) continue
                    if (
                        (layer === 'water' || layer === 'lava') &&
                        neighbor !== 0 &&
                        neighbor !== (id === 4 ? 5 : 4)
                    )
                        continue
                    if (layer === 'ores' && neighbor >= 16) continue
                    appendFace(target, id, x, y, z, side)
                }
            }
        }
    const result = {} as UndergroundMesh['layers']
    for (const layer of Object.keys(layers) as UndergroundLayer[]) {
        const source = layers[layer]
        result[layer] = {
            positions: new Float32Array(source.positions),
            normals: new Float32Array(source.normals),
            uvs: new Float32Array(source.uvs),
            colors: new Float32Array(source.colors),
            blockPositions: new Float32Array(source.blockPositions),
        }
    }
    return { layers: result, spawn, faces, noAir: !Number.isFinite(spawnDistance) }
}
