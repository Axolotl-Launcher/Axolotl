import * as THREE from 'three'

export interface SolidSkinLayerDefinition {
    width: number
    height: number
    depth: number
    u: number
    v: number
    pixelScale: readonly [number, number, number]
    centerOffset: readonly [number, number, number]
}

type Axis = 'x' | 'y' | 'z'
type Face = 'down' | 'up' | 'north' | 'south' | 'west' | 'east'
type VoxelPosition = { x: number; y: number; z: number }
type UvPosition = { u: number; v: number }
type Point = [number, number, number]

const TEXTURE_SIZE = 64
const MODEL_PIXEL_SIZE = 1 / 16
const UV_PIXEL_INSET = 1 / TEXTURE_SIZE
const FACES: readonly Face[] = ['down', 'up', 'north', 'south', 'west', 'east']
const FACE_AXIS: Record<Face, Axis> = {
    down: 'y',
    up: 'y',
    north: 'z',
    south: 'z',
    west: 'x',
    east: 'x',
}
const FACE_STEP: Record<Face, VoxelPosition> = {
    down: { x: 0, y: -1, z: 0 },
    up: { x: 0, y: 1, z: 0 },
    north: { x: 0, y: 0, z: -1 },
    south: { x: 0, y: 0, z: 1 },
    west: { x: -1, y: 0, z: 0 },
    east: { x: 1, y: 0, z: 0 },
}
const OPPOSITE_FACE: Record<Face, Face> = {
    down: 'up',
    up: 'down',
    north: 'south',
    south: 'north',
    west: 'east',
    east: 'west',
}
const GEOMETRY_FACE: Record<Face, Face> = {
    down: 'up',
    up: 'down',
    north: 'north',
    south: 'south',
    west: 'east',
    east: 'west',
}

function alpha(pixels: Uint8ClampedArray, uv: UvPosition): number {
    if (uv.u < 0 || uv.v < 0 || uv.u >= TEXTURE_SIZE || uv.v >= TEXTURE_SIZE) return 0
    return pixels[(uv.v * TEXTURE_SIZE + uv.u) * 4 + 3]
}

function isPresent(pixels: Uint8ClampedArray, uv: UvPosition): boolean {
    return alpha(pixels, uv) !== 0
}

function isSolid(pixels: Uint8ClampedArray, uv: UvPosition): boolean {
    return alpha(pixels, uv) === 255
}

function faceSize(d: SolidSkinLayerDefinition, face: Face): UvPosition {
    if (face === 'down' || face === 'up') return { u: d.width, v: d.depth }
    if (face === 'north' || face === 'south') return { u: d.width, v: d.height }
    return { u: d.depth, v: d.height }
}

function textureUv(d: SolidSkinLayerDefinition, face: Face, uv: UvPosition): UvPosition {
    if (face === 'down') return { u: d.u + d.depth + uv.u, v: d.v + uv.v }
    if (face === 'up') return { u: d.u + d.depth + d.width + uv.u, v: d.v + uv.v }
    if (face === 'north') return { u: d.u + d.depth + uv.u, v: d.v + d.depth + uv.v }
    if (face === 'south') {
        return { u: d.u + d.depth + d.width + d.depth + uv.u, v: d.v + d.depth + uv.v }
    }
    if (face === 'west') return { u: d.u + uv.u, v: d.v + d.depth + uv.v }
    return { u: d.u + d.depth + d.width + uv.u, v: d.v + d.depth + uv.v }
}

function uvToVoxel(d: SolidSkinLayerDefinition, face: Face, uv: UvPosition): VoxelPosition {
    if (face === 'down') return { x: uv.u, y: 0, z: d.depth - 1 - uv.v }
    if (face === 'up') return { x: uv.u, y: d.height - 1, z: d.depth - 1 - uv.v }
    if (face === 'north') return { x: uv.u, y: uv.v, z: 0 }
    if (face === 'south') return { x: d.width - 1 - uv.u, y: uv.v, z: d.depth - 1 }
    if (face === 'west') return { x: 0, y: uv.v, z: d.depth - 1 - uv.u }
    return { x: d.width - 1, y: uv.v, z: uv.u }
}

function voxelToUv(d: SolidSkinLayerDefinition, face: Face, voxel: VoxelPosition): UvPosition {
    if (face === 'down' || face === 'up') return { u: voxel.x, v: d.depth - 1 - voxel.z }
    if (face === 'north') return { u: voxel.x, v: voxel.y }
    if (face === 'south') return { u: d.width - 1 - voxel.x, v: voxel.y }
    if (face === 'west') return { u: d.depth - 1 - voxel.z, v: voxel.y }
    return { u: voxel.z, v: voxel.y }
}

function move(voxel: VoxelPosition, face: Face, amount = 1): VoxelPosition {
    const step = FACE_STEP[face]
    return {
        x: voxel.x + step.x * amount,
        y: voxel.y + step.y * amount,
        z: voxel.z + step.z * amount,
    }
}

function isOnFace(uv: UvPosition, size: UvPosition): boolean {
    return uv.u >= 0 && uv.u < size.u && uv.v >= 0 && uv.v < size.v
}

function pixelUv(uv: UvPosition): [number, number] {
    return [(uv.u + 0.5) / TEXTURE_SIZE, (uv.v + 0.5) / TEXTURE_SIZE]
}

function edgeUv(value: number, end: boolean): number {
    return (value + (end ? -UV_PIXEL_INSET : UV_PIXEL_INSET)) / TEXTURE_SIZE
}

function addTriangles(
    positions: number[],
    normals: number[],
    uvs: number[],
    corners: Point[],
    normal: Point,
    uvCorners: Array<[number, number]>,
): void {
    const indices = corners.length === 3 ? [0, 1, 2] : [0, 1, 2, 0, 2, 3]
    for (const index of indices) {
        positions.push(...corners[index])
        normals.push(...normal)
        uvs.push(...uvCorners[index])
    }
}

function directionValue(point: Point, face: Face): number {
    const step = FACE_STEP[face]
    return point[0] * step.x + point[1] * step.y + point[2] * step.z
}

function trimCorner(corners: Point[], directions: readonly Face[]): Point[] {
    if (!directions.length) return corners
    let removed = corners[0]
    for (let index = 1; index < corners.length; index++) {
        const candidate = corners[index]
        for (const direction of directions) {
            const difference =
                directionValue(removed, direction) - directionValue(candidate, direction)
            if (difference > 0) break
            if (difference < 0) {
                removed = candidate
                break
            }
        }
    }
    return corners.filter((corner) => corner !== removed)
}

function cornerForFace(corners: readonly Face[][], face: Face): Face[] | undefined {
    return corners.find((corner) =>
        corner.every((direction) => FACE_AXIS[direction] !== FACE_AXIS[face]),
    )
}

function addPixelCube(
    positions: number[],
    normals: number[],
    uvs: number[],
    min: THREE.Vector3,
    max: THREE.Vector3,
    uv: UvPosition,
    hidden: ReadonlySet<Face>,
    hiddenCorners: readonly Face[][],
): void {
    const hiddenGeometryFaces = new Set([...hidden].map((face) => GEOMETRY_FACE[face]))
    const geometryCorners = hiddenCorners.map((corner) => corner.map((face) => GEOMETRY_FACE[face]))
    const sampledUv = pixelUv(uv)
    const faces: Array<{ face: Face; normal: Point; corners: Point[] }> = [
        {
            face: 'north',
            normal: [0, 0, -1],
            corners: [
                [max.x, min.y, min.z],
                [min.x, min.y, min.z],
                [min.x, max.y, min.z],
                [max.x, max.y, min.z],
            ],
        },
        {
            face: 'south',
            normal: [0, 0, 1],
            corners: [
                [min.x, min.y, max.z],
                [max.x, min.y, max.z],
                [max.x, max.y, max.z],
                [min.x, max.y, max.z],
            ],
        },
        {
            face: 'down',
            normal: [0, -1, 0],
            corners: [
                [min.x, min.y, min.z],
                [max.x, min.y, min.z],
                [max.x, min.y, max.z],
                [min.x, min.y, max.z],
            ],
        },
        {
            face: 'up',
            normal: [0, 1, 0],
            corners: [
                [min.x, max.y, max.z],
                [max.x, max.y, max.z],
                [max.x, max.y, min.z],
                [min.x, max.y, min.z],
            ],
        },
        {
            face: 'west',
            normal: [-1, 0, 0],
            corners: [
                [min.x, min.y, min.z],
                [min.x, min.y, max.z],
                [min.x, max.y, max.z],
                [min.x, max.y, min.z],
            ],
        },
        {
            face: 'east',
            normal: [1, 0, 0],
            corners: [
                [max.x, min.y, max.z],
                [max.x, min.y, min.z],
                [max.x, max.y, min.z],
                [max.x, max.y, max.z],
            ],
        },
    ]

    for (const face of faces) {
        if (hiddenGeometryFaces.has(face.face)) continue
        const trimmedCorners = trimCorner(
            face.corners,
            cornerForFace(geometryCorners, face.face) ?? [],
        )
        addTriangles(
            positions,
            normals,
            uvs,
            trimmedCorners,
            face.normal,
            trimmedCorners.map(() => sampledUv),
        )
    }
}

function faceRect(d: SolidSkinLayerDefinition, face: Face): [number, number, number, number] {
    const start = textureUv(d, face, { u: 0, v: 0 })
    const size = faceSize(d, face)
    return [start.u, start.v, start.u + size.u, start.v + size.v]
}

function addSurfaceBox(
    positions: number[],
    normals: number[],
    uvs: number[],
    min: THREE.Vector3,
    max: THREE.Vector3,
    d: SolidSkinLayerDefinition,
): void {
    const rect = (face: Face) => {
        const [u0, v0, u1, v1] = faceRect(d, face)
        return [edgeUv(u0, false), edgeUv(v0, false), edgeUv(u1, true), edgeUv(v1, true)] as const
    }
    const addFace = (normal: Point, corners: Point[], uvCorners: Array<[number, number]>) =>
        addTriangles(positions, normals, uvs, corners, normal, uvCorners)

    {
        const [u0, v0, u1, v1] = rect('north')
        addFace(
            [0, 0, -1],
            [
                [max.x, min.y, min.z],
                [min.x, min.y, min.z],
                [min.x, max.y, min.z],
                [max.x, max.y, min.z],
            ],
            [
                [u0, v1],
                [u1, v1],
                [u1, v0],
                [u0, v0],
            ],
        )
    }
    {
        const [u0, v0, u1, v1] = rect('south')
        addFace(
            [0, 0, 1],
            [
                [min.x, min.y, max.z],
                [max.x, min.y, max.z],
                [max.x, max.y, max.z],
                [min.x, max.y, max.z],
            ],
            [
                [u0, v1],
                [u1, v1],
                [u1, v0],
                [u0, v0],
            ],
        )
    }
    {
        const [u0, v0, u1, v1] = rect('up')
        addFace(
            [0, -1, 0],
            [
                [min.x, min.y, min.z],
                [max.x, min.y, min.z],
                [max.x, min.y, max.z],
                [min.x, min.y, max.z],
            ],
            [
                [u1, v1],
                [u0, v1],
                [u0, v0],
                [u1, v0],
            ],
        )
    }
    {
        const [u0, v0, u1, v1] = rect('down')
        addFace(
            [0, 1, 0],
            [
                [min.x, max.y, max.z],
                [max.x, max.y, max.z],
                [max.x, max.y, min.z],
                [min.x, max.y, min.z],
            ],
            [
                [u1, v0],
                [u0, v0],
                [u0, v1],
                [u1, v1],
            ],
        )
    }
    for (const [face, x, normal] of [
        ['east', min.x, [-1, 0, 0]],
        ['west', max.x, [1, 0, 0]],
    ] satisfies Array<[Face, number, Point]>) {
        const [u0, v0, u1, v1] = rect(face)
        const corners: Point[] =
            normal[0] < 0
                ? [
                      [x, min.y, min.z],
                      [x, min.y, max.z],
                      [x, max.y, max.z],
                      [x, max.y, min.z],
                  ]
                : [
                      [x, min.y, max.z],
                      [x, min.y, min.z],
                      [x, max.y, min.z],
                      [x, max.y, max.z],
                  ]
        addFace(normal, corners, [
            [u0, v1],
            [u1, v1],
            [u1, v0],
            [u0, v0],
        ])
    }
}

function addExtrudedPixel(
    positions: number[],
    normals: number[],
    uvs: number[],
    pixels: Uint8ClampedArray,
    bounds: THREE.Box3,
    voxelSize: THREE.Vector3,
    d: SolidSkinLayerDefinition,
    face: Face,
    onFaceUv: UvPosition,
): void {
    const onTextureUv = textureUv(d, face, onFaceUv)
    if (!isPresent(pixels, onTextureUv)) return

    const voxel = uvToVoxel(d, face, onFaceUv)
    const solidPixel = isSolid(pixels, onTextureUv)
    const hidden = new Set<Face>()
    const hiddenCorners: Face[][] = []
    let onBorder = false
    let backsideOverlaps = false
    const size = faceSize(d, face)

    for (const neighbourFace of FACES) {
        if (FACE_AXIS[neighbourFace] === FACE_AXIS[face]) continue
        const neighbour = move(voxel, neighbourFace)
        let neighbourUv = voxelToUv(d, face, neighbour)
        if (isOnFace(neighbourUv, size)) {
            const neighbourTextureUv = textureUv(d, face, neighbourUv)
            if (isPresent(pixels, neighbourTextureUv)) {
                if (!(solidPixel && !isSolid(pixels, neighbourTextureUv))) hidden.add(neighbourFace)
            } else {
                const farNeighbour = move(neighbour, neighbourFace)
                let farNeighbourUv = voxelToUv(d, face, farNeighbour)
                if (!isOnFace(farNeighbourUv, size)) {
                    farNeighbourUv = voxelToUv(d, neighbourFace, farNeighbour)
                    const farTextureUv = textureUv(d, neighbourFace, farNeighbourUv)
                    if (isPresent(pixels, farTextureUv)) {
                        if (!(solidPixel && !isSolid(pixels, farTextureUv)))
                            hidden.add(neighbourFace)
                    }
                }
            }
        } else {
            onBorder = true
            neighbourUv = voxelToUv(d, neighbourFace, voxel)
            if (isPresent(pixels, textureUv(d, neighbourFace, neighbourUv))) {
                backsideOverlaps = true
                hidden.add(neighbourFace)
                hiddenCorners.push([OPPOSITE_FACE[face], neighbourFace])
            } else {
                const inward = move(voxel, face, -1)
                const inwardUv = voxelToUv(d, neighbourFace, inward)
                if (isPresent(pixels, textureUv(d, neighbourFace, inwardUv)))
                    backsideOverlaps = true
            }
        }
    }

    if (!onBorder || backsideOverlaps) hidden.add(OPPOSITE_FACE[face])
    hidden.add(face)

    const threeVoxel = {
        x: d.width - 1 - voxel.x,
        y: d.height - 1 - voxel.y,
        z: voxel.z,
    }
    const min = new THREE.Vector3(
        bounds.min.x + threeVoxel.x * voxelSize.x,
        bounds.min.y + threeVoxel.y * voxelSize.y,
        bounds.min.z + threeVoxel.z * voxelSize.z,
    )
    const max = min.clone().add(voxelSize)
    addPixelCube(positions, normals, uvs, min, max, onTextureUv, hidden, hiddenCorners)
}

export function createSolidSkinLayerGeometry(
    mesh: THREE.Mesh,
    pixels: Uint8ClampedArray,
    d: SolidSkinLayerDefinition,
): THREE.BufferGeometry | null {
    const position = mesh.geometry.getAttribute('position') as THREE.BufferAttribute | undefined
    if (!position || pixels.length < TEXTURE_SIZE * TEXTURE_SIZE * 4) return null

    const sourceBounds = new THREE.Box3().setFromBufferAttribute(position)
    const center = sourceBounds
        .getCenter(new THREE.Vector3())
        .add(new THREE.Vector3(...d.centerOffset))
    const voxelSize = new THREE.Vector3(
        MODEL_PIXEL_SIZE * d.pixelScale[0],
        MODEL_PIXEL_SIZE * d.pixelScale[1],
        MODEL_PIXEL_SIZE * d.pixelScale[2],
    )
    const size = new THREE.Vector3(
        voxelSize.x * d.width,
        voxelSize.y * d.height,
        voxelSize.z * d.depth,
    )
    const halfSize = size.clone().multiplyScalar(0.5)
    const bounds = new THREE.Box3(center.clone().sub(halfSize), center.clone().add(halfSize))
    const positions: number[] = []
    const normals: number[] = []
    const uvs: number[] = []

    addSurfaceBox(positions, normals, uvs, bounds.min, bounds.max, d)
    for (const face of FACES) {
        const faceDimensions = faceSize(d, face)
        for (let u = 0; u < faceDimensions.u; u++) {
            for (let v = 0; v < faceDimensions.v; v++) {
                addExtrudedPixel(positions, normals, uvs, pixels, bounds, voxelSize, d, face, {
                    u,
                    v,
                })
            }
        }
    }

    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
    geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3))
    geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
    geometry.computeBoundingBox()
    geometry.computeBoundingSphere()
    return geometry
}
