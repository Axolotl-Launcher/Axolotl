import * as THREE from 'three'

import { type AnimationDefinition, ATTACK, EMERGE, ROAR, SNIFF } from './animations'

type Vec3 = [number, number, number]

interface CubeDefinition {
    texOffs: [number, number]
    origin: Vec3
    size: Vec3
    mirror?: boolean
}

interface PartDefinition {
    name: string
    parent: string | null
    pose: Vec3
    cubes: CubeDefinition[]
}

/** `WardenModel.createBodyLayer()`: pivots and boxes on the 128×128 texture, in model pixels. */
const PARTS: PartDefinition[] = [
    { name: 'root', parent: null, pose: [0, 0, 0], cubes: [] },
    { name: 'bone', parent: 'root', pose: [0, 24, 0], cubes: [] },
    {
        name: 'body',
        parent: 'bone',
        pose: [0, -21, 0],
        cubes: [{ texOffs: [0, 0], origin: [-9, -13, -4], size: [18, 21, 11] }],
    },
    {
        name: 'right_ribcage',
        parent: 'body',
        pose: [-7, -2, -4],
        cubes: [{ texOffs: [90, 11], origin: [-2, -11, -0.1], size: [9, 21, 0] }],
    },
    {
        name: 'left_ribcage',
        parent: 'body',
        pose: [7, -2, -4],
        cubes: [{ texOffs: [90, 11], origin: [-7, -11, -0.1], size: [9, 21, 0], mirror: true }],
    },
    {
        name: 'head',
        parent: 'body',
        pose: [0, -13, 0],
        cubes: [{ texOffs: [0, 32], origin: [-8, -16, -5], size: [16, 16, 10] }],
    },
    {
        name: 'right_tendril',
        parent: 'head',
        pose: [-8, -12, 0],
        cubes: [{ texOffs: [52, 32], origin: [-16, -13, 0], size: [16, 16, 0] }],
    },
    {
        name: 'left_tendril',
        parent: 'head',
        pose: [8, -12, 0],
        cubes: [{ texOffs: [58, 0], origin: [0, -13, 0], size: [16, 16, 0] }],
    },
    {
        name: 'right_arm',
        parent: 'body',
        pose: [-13, -13, 1],
        cubes: [{ texOffs: [44, 50], origin: [-4, 0, -4], size: [8, 28, 8] }],
    },
    {
        name: 'left_arm',
        parent: 'body',
        pose: [13, -13, 1],
        cubes: [{ texOffs: [0, 58], origin: [-4, 0, -4], size: [8, 28, 8] }],
    },
    {
        name: 'right_leg',
        parent: 'bone',
        pose: [-5.9, -13, 0],
        cubes: [{ texOffs: [76, 48], origin: [-3.1, 0, -3], size: [6, 13, 6] }],
    },
    {
        name: 'left_leg',
        parent: 'bone',
        pose: [5.9, -13, 0],
        cubes: [{ texOffs: [76, 76], origin: [-2.9, 0, -3], size: [6, 13, 6] }],
    },
]

const TEXTURE_SIZE = 128

/**
 * `WardenRenderer`'s emissive layers in draw order: the texture, the parts the
 * layer model keeps (`retainExactParts`) and the alpha function of each.
 */
const EMISSIVE_LAYERS: {
    texture: string
    parts: string[]
    alpha: (state: WardenRenderState) => number
}[] = [
    {
        texture: 'warden_bioluminescent_layer',
        parts: ['head', 'left_arm', 'right_arm', 'left_leg', 'right_leg'],
        alpha: () => 1,
    },
    {
        texture: 'warden_pulsating_spots_1',
        parts: ['body', 'head', 'left_arm', 'right_arm', 'left_leg', 'right_leg'],
        alpha: (state) => Math.max(0, Math.cos(state.ageInTicks * 0.045) * 0.25),
    },
    {
        texture: 'warden_pulsating_spots_2',
        parts: ['body', 'head', 'left_arm', 'right_arm', 'left_leg', 'right_leg'],
        alpha: (state) => Math.max(0, Math.cos(state.ageInTicks * 0.045 + Math.PI) * 0.25),
    },
    {
        texture: 'warden',
        parts: ['left_tendril', 'right_tendril'],
        alpha: (state) => state.tendrilAnimation,
    },
    { texture: 'warden_heart', parts: ['body'], alpha: (state) => state.heartAnimation },
]

export const WARDEN_TEXTURES = [
    'warden',
    'warden_bioluminescent_layer',
    'warden_pulsating_spots_1',
    'warden_pulsating_spots_2',
    'warden_heart',
]

/** What `WardenRenderer.extractRenderState` hands the model for one frame. */
export interface WardenRenderState {
    ageInTicks: number
    bodyRot: number
    /** Head yaw relative to the body, in degrees. */
    yRot: number
    /** Head pitch, in degrees; positive looks down. */
    xRot: number
    walkAnimationPos: number
    walkAnimationSpeed: number
    tendrilAnimation: number
    heartAnimation: number
    /** Ticks each `AnimationState` was started at, or null while stopped. */
    attackStart: number | null
    emergeStart: number | null
    roarStart: number | null
    sniffStart: number | null
}

interface Part {
    object: THREE.Object3D
    initial: Vec3
    x: number
    y: number
    z: number
    xRot: number
    yRot: number
    zRot: number
    xScale: number
    yScale: number
    zScale: number
}

/** `ModelPart.Cube`: the box's six faces with the vanilla UV layout and face normals. */
function cubeGeometry(cube: CubeDefinition) {
    const [u, v] = cube.texOffs
    const [width, height, depth] = cube.size
    let minX = cube.origin[0]
    let maxX = minX + width
    const minY = cube.origin[1]
    const maxY = minY + height
    const minZ = cube.origin[2]
    const maxZ = minZ + depth
    if (cube.mirror) [minX, maxX] = [maxX, minX]

    const t0: Vec3 = [minX, minY, minZ]
    const t1: Vec3 = [maxX, minY, minZ]
    const t2: Vec3 = [maxX, maxY, minZ]
    const t3: Vec3 = [minX, maxY, minZ]
    const l0: Vec3 = [minX, minY, maxZ]
    const l1: Vec3 = [maxX, minY, maxZ]
    const l2: Vec3 = [maxX, maxY, maxZ]
    const l3: Vec3 = [minX, maxY, maxZ]
    const u0 = u
    const u1 = u + depth
    const u2 = u + depth + width
    const u22 = u + depth + width + width
    const u3 = u + depth + width + depth
    const u4 = u + depth + width + depth + width
    const v0 = v
    const v1 = v + depth
    const v2 = v + depth + height

    const faces: [Vec3[], number, number, number, number, Vec3][] = [
        [[l1, l0, t0, t1], u1, v0, u2, v1, [0, -1, 0]],
        [[t2, t3, l3, l2], u2, v1, u22, v0, [0, 1, 0]],
        [[t0, l0, l3, t3], u0, v1, u1, v2, [-1, 0, 0]],
        [[t1, t0, t3, t2], u1, v1, u2, v2, [0, 0, -1]],
        [[l1, t1, t2, l2], u2, v1, u3, v2, [1, 0, 0]],
        [[l0, l1, l2, l3], u3, v1, u4, v2, [0, 0, 1]],
    ]

    const positions: number[] = []
    const uvs: number[] = []
    const normals: number[] = []
    for (const [corners, fu0, fv0, fu1, fv1, facing] of faces) {
        const faceUvs: [number, number][] = [
            [fu1, fv0],
            [fu0, fv0],
            [fu0, fv1],
            [fu1, fv1],
        ]
        let quad = corners.map((corner, index) => ({ corner, uv: faceUvs[index]! }))
        const normal: Vec3 = cube.mirror && facing[0] !== 0 ? [-facing[0], 0, 0] : facing
        if (cube.mirror) quad = quad.reverse()
        for (const index of [0, 1, 2, 0, 2, 3]) {
            const { corner, uv } = quad[index]!
            positions.push(corner[0] / 16, corner[1] / 16, corner[2] / 16)
            uvs.push(uv[0] / TEXTURE_SIZE, uv[1] / TEXTURE_SIZE)
            normals.push(...normal)
        }
    }

    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
    geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
    geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3))
    return geometry
}

/** Uniforms every Minecraft-shaded material reads: the lightmap colour and the fog of the frame. */
export function createSharedUniforms() {
    return {
        lightmapColor: { value: new THREE.Vector3(1, 1, 1) },
        fogColor: { value: new THREE.Vector4(0, 0, 0, 1) },
        fogEnvironmentalStart: { value: 1024 },
        fogEnvironmentalEnd: { value: 1024 },
        fogRenderDistanceStart: { value: 1024 },
        fogRenderDistanceEnd: { value: 1024 },
        /** The page the warden emerges from: nothing of it is drawn below. */
        floorY: { value: 0 },
    }
}

export type SharedUniforms = ReturnType<typeof createSharedUniforms>

/** `include/fog.glsl`, applied to the camera-relative position. */
export const FOG_GLSL = /* glsl */ `
uniform vec4 fogColor;
uniform float fogEnvironmentalStart;
uniform float fogEnvironmentalEnd;
uniform float fogRenderDistanceStart;
uniform float fogRenderDistanceEnd;

float linear_fog_value(float vertexDistance, float fogStart, float fogEnd) {
    if (vertexDistance <= fogStart) {
        return 0.0;
    } else if (vertexDistance >= fogEnd) {
        return 1.0;
    }
    return (vertexDistance - fogStart) / (fogEnd - fogStart);
}

vec4 apply_fog(vec4 inColor, vec3 relative) {
    float spherical = length(relative);
    float cylindrical = max(length(relative.xz), abs(relative.y));
    float fogValue = max(
        linear_fog_value(spherical, fogEnvironmentalStart, fogEnvironmentalEnd),
        linear_fog_value(cylindrical, fogRenderDistanceStart, fogRenderDistanceEnd)
    );
    return vec4(mix(inColor.rgb, fogColor.rgb, fogValue * fogColor.a), inColor.a);
}
`

const ENTITY_VERTEX = /* glsl */ `
varying vec2 texCoord0;
varying vec3 relativePosition;
varying vec3 worldNormal;
varying float worldY;

void main() {
    vec4 world = modelMatrix * vec4(position, 1.0);
    relativePosition = world.xyz - cameraPosition;
    worldY = world.y;
    worldNormal = normalize(mat3(modelMatrix) * normal);
    texCoord0 = uv;
    gl_Position = projectionMatrix * viewMatrix * world;
}
`

/**
 * `core/entity.fsh` with the defines of `entity_cutout` and
 * `entity_translucent_emissive`: a 0.1 alpha cutout, per-face lighting from the
 * level's two diffuse lights, the lightmap unless emissive, then fog.
 */
const ENTITY_FRAGMENT = /* glsl */ `
uniform sampler2D map;
uniform float alpha;
uniform vec3 lightmapColor;
uniform float floorY;
varying vec2 texCoord0;
varying vec3 relativePosition;
varying vec3 worldNormal;
varying float worldY;
${FOG_GLSL}

const vec3 LIGHT0 = normalize(vec3(0.2, 1.0, -0.7));
const vec3 LIGHT1 = normalize(vec3(-0.2, 1.0, 0.7));

void main() {
    if (worldY < floorY) {
        discard;
    }
    vec4 color = texture2D(map, texCoord0);
    if (color.a < 0.1) {
        discard;
    }
    vec3 normal = gl_FrontFacing ? worldNormal : -worldNormal;
    vec2 light = max(vec2(0.0), vec2(dot(LIGHT0, normal), dot(LIGHT1, normal)));
    float lightAccum = min(1.0, (light.x + light.y) * 0.6 + 0.4);
    color *= vec4(vec3(lightAccum), alpha);
#ifndef EMISSIVE
    color.rgb *= lightmapColor;
#endif
    gl_FragColor = apply_fog(color, relativePosition);
}
`

export function pixelTexture(image: HTMLImageElement | HTMLCanvasElement) {
    const texture = new THREE.Texture(image)
    texture.magFilter = THREE.NearestFilter
    texture.minFilter = THREE.NearestFilter
    texture.generateMipmaps = false
    texture.flipY = false
    texture.colorSpace = THREE.NoColorSpace
    texture.needsUpdate = true
    return texture
}

function entityMaterial(map: THREE.Texture, shared: SharedUniforms, emissive: boolean) {
    return new THREE.ShaderMaterial({
        uniforms: { ...shared, map: { value: map }, alpha: { value: 1 } },
        vertexShader: ENTITY_VERTEX,
        fragmentShader: ENTITY_FRAGMENT,
        defines: emissive ? { EMISSIVE: '' } : {},
        side: THREE.DoubleSide,
        transparent: emissive,
        depthWrite: !emissive,
    })
}

type BakedKeyframe = [number, number, number, number, 0 | 1]

function binarySearch(keyframes: BakedKeyframe[], seconds: number) {
    let low = 0
    let high = keyframes.length
    while (low < high) {
        const mid = (low + high) >> 1
        if (seconds <= keyframes[mid]![0]) high = mid
        else low = mid + 1
    }
    return low
}

function clampIndex(index: number, length: number) {
    return Math.min(length - 1, Math.max(0, index))
}

function catmullrom(t: number, p0: number, p1: number, p2: number, p3: number) {
    return (
        0.5 *
        (2 * p1 +
            (p2 - p0) * t +
            (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t +
            (3 * p1 - p0 - 3 * p2 + p3) * t * t * t)
    )
}

/** `KeyframeAnimation`, baked once: keyframes converted as `degreeVec`, `posVec` and `scaleVec` do. */
function bake(definition: AnimationDefinition) {
    return {
        definition,
        channels: definition.channels.map(([part, target, keyframes]) => ({
            part,
            target,
            keyframes: keyframes.map(([time, x, y, z, interpolation]): BakedKeyframe => {
                if (target === 'rotation') {
                    const r = Math.PI / 180
                    return [time, x * r, y * r, z * r, interpolation]
                }
                if (target === 'position') return [time, x, -y, z, interpolation]
                return [time, x - 1, y - 1, z - 1, interpolation]
            }),
        })),
    }
}

export class WardenModel {
    readonly root = new THREE.Group()
    private readonly parts = new Map<string, Part>()
    private readonly emissiveMaterials: {
        material: THREE.ShaderMaterial
        alpha: (state: WardenRenderState) => number
    }[] = []
    private readonly animations = {
        attack: bake(ATTACK),
        emerge: bake(EMERGE),
        roar: bake(ROAR),
        sniff: bake(SNIFF),
    }

    constructor(textures: Record<string, THREE.Texture>, shared: SharedUniforms) {
        this.root.matrixAutoUpdate = false
        const geometries = new Map<string, THREE.BufferGeometry>()
        for (const definition of PARTS) {
            const object = new THREE.Object3D()
            object.matrixAutoUpdate = false
            const parent = definition.parent ? this.parts.get(definition.parent)!.object : this.root
            parent.add(object)
            this.parts.set(definition.name, {
                object,
                initial: definition.pose,
                x: 0,
                y: 0,
                z: 0,
                xRot: 0,
                yRot: 0,
                zRot: 0,
                xScale: 1,
                yScale: 1,
                zScale: 1,
            })
            if (definition.cubes.length > 0) {
                geometries.set(definition.name, cubeGeometry(definition.cubes[0]!))
            }
        }

        const texture = (name: string) => {
            const found = textures[name]
            if (!found) throw new Error(`Missing texture ${name}`)
            return found
        }
        const base = entityMaterial(texture('warden'), shared, false)
        for (const [name, geometry] of geometries) {
            const mesh = new THREE.Mesh(geometry, base)
            mesh.renderOrder = 0
            this.parts.get(name)!.object.add(mesh)
        }
        EMISSIVE_LAYERS.forEach((layer, index) => {
            const material = entityMaterial(texture(layer.texture), shared, true)
            this.emissiveMaterials.push({ material, alpha: layer.alpha })
            for (const name of layer.parts) {
                const mesh = new THREE.Mesh(geometries.get(name)!, material)
                mesh.renderOrder = index + 1
                this.parts.get(name)!.object.add(mesh)
            }
        })
    }

    private part(name: string) {
        return this.parts.get(name)!
    }

    /** `WardenModel.setupAnim` followed by the layer alphas and the part matrices. */
    setupAnim(state: WardenRenderState) {
        for (const part of this.parts.values()) {
            ;[part.x, part.y, part.z] = part.initial
            part.xRot = part.yRot = part.zRot = 0
            part.xScale = part.yScale = part.zScale = 1
        }
        const head = this.part('head')
        const body = this.part('body')

        head.xRot = state.xRot * (Math.PI / 180)
        head.yRot = state.yRot * (Math.PI / 180)

        const speedModifier = Math.min(0.5, 3 * state.walkAnimationSpeed)
        const adjustedPos = state.walkAnimationPos * 0.8662
        const adjustedPosCosine = Math.cos(adjustedPos)
        const adjustedPosSine = Math.sin(adjustedPos)
        const speedModifierWithMin = Math.min(0.35, speedModifier)
        head.zRot += 0.3 * adjustedPosSine * speedModifier
        head.xRot += 1.2 * Math.cos(adjustedPos + Math.PI / 2) * speedModifierWithMin
        body.zRot = 0.1 * adjustedPosSine * speedModifier
        body.xRot = adjustedPosCosine * speedModifierWithMin
        this.part('left_leg').xRot = adjustedPosCosine * speedModifier
        this.part('right_leg').xRot = Math.cos(adjustedPos + Math.PI) * speedModifier
        const leftArm = this.part('left_arm')
        const rightArm = this.part('right_arm')
        leftArm.xRot = -(0.8 * adjustedPosCosine * speedModifier)
        leftArm.zRot = 0
        rightArm.xRot = -(0.8 * adjustedPosSine * speedModifier)
        rightArm.zRot = 0
        leftArm.yRot = 0
        leftArm.z = 1
        leftArm.x = 13
        leftArm.y = -13
        rightArm.yRot = 0
        rightArm.z = 1
        rightArm.x = -13
        rightArm.y = -13

        const scaledAge = state.ageInTicks * 0.1
        head.zRot += 0.06 * Math.cos(scaledAge)
        head.xRot += 0.06 * Math.sin(scaledAge)
        body.zRot += 0.025 * Math.sin(scaledAge)
        body.xRot += 0.025 * Math.cos(scaledAge)

        const tendrilXRot =
            state.tendrilAnimation * (Math.cos(state.ageInTicks * 2.25) * Math.PI * 0.1)
        this.part('left_tendril').xRot = tendrilXRot
        this.part('right_tendril').xRot = -tendrilXRot

        this.apply(this.animations.attack, state.attackStart, state.ageInTicks)
        this.apply(this.animations.emerge, state.emergeStart, state.ageInTicks)
        this.apply(this.animations.roar, state.roarStart, state.ageInTicks)
        this.apply(this.animations.sniff, state.sniffStart, state.ageInTicks)

        for (const { material, alpha } of this.emissiveMaterials) {
            const value = alpha(state)
            material.uniforms.alpha!.value = value
            material.visible = value > 1.0e-5
        }

        const rotation = new THREE.Euler()
        const quaternion = new THREE.Quaternion()
        for (const part of this.parts.values()) {
            rotation.set(part.xRot, part.yRot, part.zRot, 'ZYX')
            quaternion.setFromEuler(rotation)
            part.object.matrix.compose(
                new THREE.Vector3(part.x / 16, part.y / 16, part.z / 16),
                quaternion,
                new THREE.Vector3(part.xScale, part.yScale, part.zScale),
            )
        }
    }

    private apply(
        animation: ReturnType<typeof bake>,
        startTick: number | null,
        ageInTicks: number,
    ) {
        if (startTick === null) return
        const millis = Math.trunc((ageInTicks - startTick) * 50)
        let seconds = millis / 1000
        if (animation.definition.looping) seconds %= animation.definition.length
        for (const channel of animation.channels) {
            const keyframes = channel.keyframes
            const at = (index: number) => keyframes[clampIndex(index, keyframes.length)]!
            const prev = Math.max(0, binarySearch(keyframes, seconds) - 1)
            const next = Math.min(keyframes.length - 1, prev + 1)
            const from = at(prev)
            const to = at(next)
            const alpha =
                next !== prev
                    ? Math.min(1, Math.max(0, (seconds - from[0]) / (to[0] - from[0])))
                    : 0
            const value: Vec3 = [0, 0, 0]
            for (const axis of [1, 2, 3] as const) {
                value[axis - 1] =
                    to[4] === 1
                        ? catmullrom(
                              alpha,
                              at(prev - 1)[axis],
                              from[axis],
                              to[axis],
                              at(next + 1)[axis],
                          )
                        : from[axis] + (to[axis] - from[axis]) * alpha
            }
            const part = this.part(channel.part)
            if (channel.target === 'position') {
                part.x += value[0]
                part.y += value[1]
                part.z += value[2]
            } else if (channel.target === 'rotation') {
                part.xRot += value[0]
                part.yRot += value[1]
                part.zRot += value[2]
            } else {
                part.xScale += value[0]
                part.yScale += value[1]
                part.zScale += value[2]
            }
        }
    }

    /** The centre of the head's box in the world, as of the last rendered frame. */
    headPosition(target: THREE.Vector3) {
        return this.part('head').object.localToWorld(target.set(0, -0.5, 0))
    }

    /**
     * Places the model as `LivingEntityRenderer` does: at the entity's feet,
     * turned by `180 - bodyRot`, flipped with `scale(-1, -1, 1)` and lowered by 1.501.
     */
    place(x: number, y: number, z: number, bodyRot: number) {
        this.root.matrix
            .makeTranslation(x, y, z)
            .multiply(new THREE.Matrix4().makeRotationY(((180 - bodyRot) * Math.PI) / 180))
            .multiply(new THREE.Matrix4().makeScale(-1, -1, 1))
            .multiply(new THREE.Matrix4().makeTranslation(0, -1.501, 0))
        this.root.matrixWorldNeedsUpdate = true
    }

    dispose() {
        this.root.traverse((object) => {
            if (object instanceof THREE.Mesh) {
                object.geometry.dispose()
                ;(object.material as THREE.Material).dispose()
            }
        })
    }
}
