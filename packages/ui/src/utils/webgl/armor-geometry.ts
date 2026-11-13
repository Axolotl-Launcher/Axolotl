import * as THREE from 'three'

export type ArmorGeometryLayer = 'outer' | 'leggings'
export type ArmorBodyPart = 'head' | 'body' | 'rightArm' | 'leftArm' | 'rightLeg' | 'leftLeg'

const MODEL_PIXEL_SIZE = 1 / 16
const OUTER_ARMOR_DILATION = MODEL_PIXEL_SIZE
const LEGGINGS_ARMOR_DILATION = MODEL_PIXEL_SIZE / 2
const LEG_DEFORMATION_REDUCTION = MODEL_PIXEL_SIZE / 10
const CLASSIC_ARM_WIDTH = 4 * MODEL_PIXEL_SIZE

interface UvBounds {
    minU: number
    maxU: number
    minV: number
    maxV: number
}

type FaceUvBounds = Pick<UvBounds, 'minU' | 'maxU'>

const ARMOR_ARM_UV_RECTS: Record<string, UvBounds> = {
    '0,0,-1': { minU: 44, maxU: 48, minV: 20, maxV: 32 },
    '0,0,1': { minU: 52, maxU: 56, minV: 20, maxV: 32 },
    '0,-1,0': { minU: 48, maxU: 52, minV: 16, maxV: 20 },
    '0,1,0': { minU: 44, maxU: 48, minV: 16, maxV: 20 },
    '-1,0,0': { minU: 48, maxU: 52, minV: 20, maxV: 32 },
    '1,0,0': { minU: 40, maxU: 44, minV: 20, maxV: 32 },
}

const ARMOR_LEG_UV_RECTS: Record<string, FaceUvBounds> = {
    '0,0,-1': { minU: 4, maxU: 8 },
    '0,0,1': { minU: 12, maxU: 16 },
    '0,-1,0': { minU: 8, maxU: 12 },
    '0,1,0': { minU: 4, maxU: 8 },
    '-1,0,0': { minU: 8, maxU: 12 },
    '1,0,0': { minU: 0, maxU: 4 },
}

function armorLegUvRect(normal: string, bodyPart: ArmorBodyPart): FaceUvBounds | undefined {
    if (bodyPart !== 'leftLeg') return ARMOR_LEG_UV_RECTS[normal]
    if (normal === '-1,0,0') return ARMOR_LEG_UV_RECTS['1,0,0']
    if (normal === '1,0,0') return ARMOR_LEG_UV_RECTS['-1,0,0']
    return ARMOR_LEG_UV_RECTS[normal]
}

function armorArmUvRect(normal: string, bodyPart: ArmorBodyPart): UvBounds | undefined {
    if (bodyPart !== 'leftArm') return ARMOR_ARM_UV_RECTS[normal]

    // Vanilla builds the left arm with CubeListBuilder.mirror(). In addition to
    // reversing every face below, mirroring swaps the two X-facing regions: the
    // physical outside of either arm uses U 40..44 and the inside uses U 48..52.
    if (normal === '-1,0,0') return ARMOR_ARM_UV_RECTS['1,0,0']
    if (normal === '1,0,0') return ARMOR_ARM_UV_RECTS['-1,0,0']
    return ARMOR_ARM_UV_RECTS[normal]
}

function isArm(bodyPart: ArmorBodyPart): boolean {
    return bodyPart === 'rightArm' || bodyPart === 'leftArm'
}

function isLeg(bodyPart: ArmorBodyPart): boolean {
    return bodyPart === 'rightLeg' || bodyPart === 'leftLeg'
}

function normalKey(normal: THREE.BufferAttribute, index: number): string {
    return `${Math.round(normal.getX(index))},${Math.round(normal.getY(index))},${Math.round(normal.getZ(index))}`
}

function remapArmUv(
    uv: THREE.BufferAttribute,
    normal: THREE.BufferAttribute,
    bodyPart: ArmorBodyPart,
): void {
    const sourceBounds = new Map<string, UvBounds>()
    for (let index = 0; index < uv.count; index++) {
        const key = normalKey(normal, index)
        if (!armorArmUvRect(key, bodyPart)) continue

        const u = uv.getX(index)
        const v = uv.getY(index)
        const bounds = sourceBounds.get(key)
        if (bounds) {
            bounds.minU = Math.min(bounds.minU, u)
            bounds.maxU = Math.max(bounds.maxU, u)
            bounds.minV = Math.min(bounds.minV, v)
            bounds.maxV = Math.max(bounds.maxV, v)
        } else {
            sourceBounds.set(key, { minU: u, maxU: u, minV: v, maxV: v })
        }
    }

    for (let index = 0; index < uv.count; index++) {
        const key = normalKey(normal, index)
        const source = sourceBounds.get(key)
        const target = armorArmUvRect(key, bodyPart)
        if (!source || !target) continue

        const uRange = source.maxU - source.minU
        const vRange = source.maxV - source.minV
        const sourceURatio = uRange > 0 ? (uv.getX(index) - source.minU) / uRange : 0
        const uRatio = bodyPart === 'leftArm' ? 1 - sourceURatio : sourceURatio
        const vRatio = vRange > 0 ? (uv.getY(index) - source.minV) / vRange : 0
        uv.setXY(
            index,
            THREE.MathUtils.lerp(target.minU, target.maxU, uRatio) / 64,
            THREE.MathUtils.lerp(target.minV, target.maxV, vRatio) / 32,
        )
    }
    uv.needsUpdate = true
}

function remapArmorUv(
    uv: THREE.BufferAttribute,
    normal: THREE.BufferAttribute | undefined,
    bodyPart: ArmorBodyPart,
): void {
    if (isArm(bodyPart) && normal) {
        // Armor arms always use the classic four-pixel layout, including on slim
        // players. Rebuild each face's UVs instead of stretching the slim atlas.
        remapArmUv(uv, normal, bodyPart)
        return
    }
    const leftLegSourceBounds = new Map<string, UvBounds>()
    if (bodyPart === 'leftLeg' && normal) {
        for (let index = 0; index < uv.count; index++) {
            const key = normalKey(normal, index)
            if (!ARMOR_LEG_UV_RECTS[key]) continue
            const u = uv.getX(index) - 16 / 64
            const v = uv.getY(index) - 32 / 64
            const bounds = leftLegSourceBounds.get(key)
            if (bounds) {
                bounds.minU = Math.min(bounds.minU, u)
                bounds.maxU = Math.max(bounds.maxU, u)
                bounds.minV = Math.min(bounds.minV, v)
                bounds.maxV = Math.max(bounds.maxV, v)
            } else {
                leftLegSourceBounds.set(key, { minU: u, maxU: u, minV: v, maxV: v })
            }
        }
    }

    for (let index = 0; index < uv.count; index++) {
        let u = uv.getX(index)
        let v = uv.getY(index)

        // Vanilla armor textures are 64x32. The left arm and leg reuse the right
        // limb regions, while player skins store their left-limb pixels in the
        // lower half of a 64x64 texture.
        if (bodyPart === 'leftLeg') {
            u -= 16 / 64
            v -= 32 / 64
            const normalKeyValue = normal ? normalKey(normal, index) : undefined
            const target = normalKeyValue && armorLegUvRect(normalKeyValue, bodyPart)
            const source = normalKeyValue && leftLegSourceBounds.get(normalKeyValue)
            if (target && source) {
                const ratio =
                    source.maxU > source.minU ? (u - source.minU) / (source.maxU - source.minU) : 0
                u = (target.minU + (1 - ratio) * (target.maxU - target.minU)) / 64
            }
        }

        uv.setXY(index, u, v * 2)
    }
    uv.needsUpdate = true
}

export function createArmorGeometry(
    source: THREE.BufferGeometry,
    layer: ArmorGeometryLayer,
    bodyPart: ArmorBodyPart,
): THREE.BufferGeometry {
    const geometry = source.clone()
    const position = geometry.getAttribute('position') as THREE.BufferAttribute | undefined
    if (!position) throw new Error('Armor source geometry has no position attribute')

    const bounds = new THREE.Box3().setFromBufferAttribute(position)
    const sourceCenter = bounds.getCenter(new THREE.Vector3())
    const targetCenter = sourceCenter.clone()
    const sourceSize = bounds.getSize(new THREE.Vector3())
    let dilation = layer === 'leggings' ? LEGGINGS_ARMOR_DILATION : OUTER_ARMOR_DILATION
    if (isLeg(bodyPart)) dilation -= LEG_DEFORMATION_REDUCTION
    const baseSize = sourceSize.clone()
    if (isArm(bodyPart) && sourceSize.x < CLASSIC_ARM_WIDTH) {
        const missingWidth = CLASSIC_ARM_WIDTH - sourceSize.x
        baseSize.x = CLASSIC_ARM_WIDTH
        // Slim arms share their inner edge with classic arms. The extra model
        // pixel therefore belongs on the outside, not equally on both sides.
        targetCenter.x += bodyPart === 'rightArm' ? missingWidth / 2 : -missingWidth / 2
    }

    const targetSize = baseSize.addScalar(dilation * 2)
    const scale = new THREE.Vector3(
        targetSize.x / sourceSize.x,
        targetSize.y / sourceSize.y,
        targetSize.z / sourceSize.z,
    )
    const vertex = new THREE.Vector3()
    for (let index = 0; index < position.count; index++) {
        vertex
            .fromBufferAttribute(position, index)
            .sub(sourceCenter)
            .multiply(scale)
            .add(targetCenter)
        position.setXYZ(index, vertex.x, vertex.y, vertex.z)
    }
    position.needsUpdate = true

    const uv = geometry.getAttribute('uv') as THREE.BufferAttribute | undefined
    const normal = geometry.getAttribute('normal') as THREE.BufferAttribute | undefined
    if (uv) remapArmorUv(uv, normal, bodyPart)

    geometry.computeBoundingBox()
    geometry.computeBoundingSphere()
    return geometry
}
