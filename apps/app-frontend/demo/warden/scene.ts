import * as THREE from 'three'

import { sceneFile } from './assets'

/**
 * The world the ancient city capture was taken in: every block face the
 * screenshot shows (and, loaded later, the ones the warden's encounter needs),
 * baked from the save with vanilla textures, smooth lighting and per-vertex
 * light levels. It is drawn as Sodium 0.7 draws terrain on
 * Minecraft 1.21.8: vertex colour = ambient occlusion × face shade × the
 * lightmap, the atlas sampled with its own mip chain (none for cutout blocks),
 * a 0.1 alpha cutoff and linear fog. The lightmap is vanilla's
 * `core/lightmap.fsh`, evaluated for the 16 × 16 light levels on the CPU, and
 * animated sprites tick as `TextureAtlasSprite.Ticker` does.
 *
 * Positions are relative to the scene's origin, the block 89286 −49 −40560.
 */

interface SpriteAnimation {
    texture: string
    /** Atlas x, y and size of the sprite. */
    atlas: [number, number, number]
    /** First row of the sprite's frames in the animation strip. */
    stripY: number
    size: number
    frameCount: number
    /** Frame index and time of each step, as the `.mcmeta` lists them. */
    sequence: [number, number][]
    interpolate: boolean
    /** Ticks into its sequence the sprite showed when the capture was taken. */
    phase: number
}

export interface SceneMeta {
    origin: [number, number, number]
    viewQuads: number
    finaleQuads: number
    atlasSize: number
    mips: number
    /** Top of the floor (full-collision blocks) per column, relative to the origin. */
    floor: { x0: number; z0: number; width: number; depth: number; heights: number[][] }
    /** Sculk shriekers that can summon, relative to the origin. */
    shriekers: [number, number, number][]
    /** Block << 4 | sky light of a box of layers, base64. */
    light: {
        x0: number
        y0: number
        z0: number
        width: number
        height: number
        depth: number
        data: string
    }
    /** The camera the capture was taken from (spectator, flying). */
    camera: { eye: [number, number, number]; yaw: number; pitch: number; fov: number }
    screenshot: [number, number]
    animations: SpriteAnimation[]
    stripSize: [number, number]
    /** Sculk sensors around those the capture shows. */
    sensors: SensorMeta[]
    /** The animation an active sensor's tendrils show. */
    tendrilActive: number
}

export interface SensorMeta {
    pos: [number, number, number]
    /** Listener radius: 8, or 16 for a calibrated sensor. */
    radius: number
    /** Whether the capture shows the sensor. */
    visible: boolean
    /** Its tendril faces in the view and the finale meshes, by quad. */
    view: number[]
    finale: number[]
    /** The shriekers its clicking reaches: index into `shriekers` and distance. */
    reaches: [number, number][]
}

const VERTEX = /* glsl */ `
precision highp float;
precision highp int;
uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;
uniform sampler2D lightmap;
uniform vec3 cameraPos;
in vec3 position;
in vec2 uv;
in float bright;
in vec2 light;
in float material;
in float sensor;
uniform float sensorActive[SENSORS];
uniform int tendrilActive;
out vec4 vColor;
out vec2 vUv;
out vec2 vDist;
flat out int vMat;
void main() {
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    // CompactChunkVertex.encodeLight clamps to [8, 248]; the shader samples at light / 256
    vColor = vec4(vec3(bright), 1.0) * texture(lightmap, clamp(light, 8.0, 248.0) / 256.0);
    vUv = uv;
    int mat = int(material + 0.5);
    int index = int(sensor + 0.5);
    // an active (or cooling down) sculk sensor's tendrils show the active texture
    if (index > 0 && sensorActive[index - 1] > 0.5) mat = (mat & 3) | ((tendrilActive + 1) << 2);
    vMat = mat;
    vec3 rel = position - cameraPos;
    vDist = vec2(max(length(rel.xz), abs(rel.y)), length(rel));
}`

/** Material bits: 1 alpha cutoff, 2 no mipmaps, bits 2..5 the animated sprite + 1. */
const FRAGMENT = /* glsl */ `
precision highp float;
precision highp int;
uniform sampler2D atlas;
uniform sampler2D strip;
uniform vec2 atlasSize;
uniform vec2 stripSize;
uniform vec4 animRect[9];
uniform vec3 animFrame[9];
uniform vec4 fogColor;
uniform vec2 envFog;
uniform vec2 renderFog;
in vec4 vColor;
in vec2 vUv;
in vec2 vDist;
flat in int vMat;
out vec4 fragColor;
float linearFog(float d, vec2 range) {
    if (d <= range.x) return 0.0;
    if (d >= range.y) return 1.0;
    return (d - range.x) / (range.y - range.x);
}
void main() {
    bool cutout = (vMat & 1) != 0;
    bool noMip = (vMat & 2) != 0;
    int anim = vMat >> 2;
    // Cutout blocks are not mipmapped (Sodium's CUTOUT): zero gradients sample the full-size level.
    float mipmapped = noMip ? 0.0 : 1.0;
    vec2 dx = dFdx(vUv) * mipmapped;
    vec2 dy = dFdy(vUv) * mipmapped;
    vec4 c;
    if (anim > 0) {
        // the sprite's current frames from the strip, blended as an interpolated animation
        vec4 r = animRect[anim - 1];
        vec3 f = animFrame[anim - 1];
        vec2 local = clamp((vUv * atlasSize - r.xy) / r.z, 0.0, 0.99999);
        vec2 scale = atlasSize / stripSize;
        vec2 ua = vec2(local.x * r.z, r.w + (f.x + local.y) * r.z) / stripSize;
        vec2 ub = vec2(local.x * r.z, r.w + (f.y + local.y) * r.z) / stripSize;
        vec4 a = textureGrad(strip, ua, dx * scale, dy * scale);
        vec4 b = textureGrad(strip, ub, dx * scale, dy * scale);
        c = vec4(mix(a.rgb, b.rgb, f.z), a.a);
    } else {
        c = textureGrad(atlas, vUv, dx, dy);
    }
    c *= vColor;
    if (cutout && c.a < 0.1) discard;
    float fog = max(linearFog(vDist.y, envFog), linearFog(vDist.x, renderFog));
    fragColor = vec4(mix(c.rgb, fogColor.rgb, fog * fogColor.a), c.a);
}`

/** The `lightmap.fsh` uniforms the scene varies. */
export interface LightmapParameters {
    ambient: number
    skyFactor: number
    blockFactor: number
    skyLightColor: [number, number, number]
    /** `NightVisionFactor`: 1 while the effect has more than 10 seconds left. */
    nightVision: number
    darknessScale: number
    brightness: number
}

/** `core/lightmap.fsh` for every block and sky level; texel (block, sky) at row sky. */
function computeLightmap(out: Uint8Array, p: LightmapParameters) {
    const brightness = (level: number) => {
        const curved = level / (4 - 3 * level)
        return curved + (1 - curved) * p.ambient
    }
    const notGamma = (x: number) => 1 - (1 - x) ** 4
    const clamp = (v: number) => Math.min(1, Math.max(0, v))
    const finish = (v: number) => {
        const c = clamp(v - p.darknessScale)
        return Math.round(clamp((c + (notGamma(c) - c) * p.brightness) * 0.96 + 0.03) * 255)
    }
    for (let sky = 0; sky < 16; sky++) {
        const sb = brightness(sky / 15) * p.skyFactor
        for (let block = 0; block < 16; block++) {
            const bb = brightness(block / 15) * p.blockFactor
            let r = (bb + p.skyLightColor[0] * sb) * 0.96 + 0.03
            let g = (bb * ((bb * 0.6 + 0.4) * 0.6 + 0.4) + p.skyLightColor[1] * sb) * 0.96 + 0.03
            let b = (bb * (bb * bb * 0.6 + 0.4) + p.skyLightColor[2] * sb) * 0.96 + 0.03
            // Night Vision scales the colour up until a channel reaches 1, before Darkness takes its share
            const max = Math.max(r, g, b)
            if (p.nightVision > 0 && max < 1) {
                r += (r / max - r) * p.nightVision
                g += (g / max - g) * p.nightVision
                b += (b / max - b) * p.nightVision
            }
            const i = (sky * 16 + block) * 4
            out[i] = finish(r)
            out[i + 1] = finish(g)
            out[i + 2] = finish(b)
            out[i + 3] = 255
        }
    }
}

/** `TextureAtlasSprite.Ticker`: the step of the sequence and the ticks spent in it. */
class SpriteTicker {
    private frame = 0
    private sub = 0

    constructor(private readonly animation: SpriteAnimation) {
        for (let tick = 0; tick < animation.phase; tick++) this.tick()
    }

    tick() {
        const sequence = this.animation.sequence
        this.sub++
        if (this.sub >= sequence[this.frame]![1]) {
            this.frame = (this.frame + 1) % sequence.length
            this.sub = 0
        }
    }

    /** The frame shown, the next frame and how far the blend has gone. */
    write(target: THREE.Vector3) {
        const sequence = this.animation.sequence
        const [frame, time] = sequence[this.frame]!
        if (!this.animation.interpolate) return target.set(frame, frame, 0)
        const next = sequence[(this.frame + 1) % sequence.length]![0]
        return target.set(frame, next, this.sub / time)
    }
}

async function bitmap(url: string) {
    const response = await fetch(url)
    if (!response.ok) throw new Error(`Could not load ${url}`)
    return createImageBitmap(await response.blob(), {
        premultiplyAlpha: 'none',
        colorSpaceConversion: 'none',
    })
}

/** A texture with the exported mip chain: `MipmapGenerator`'s levels, per sprite. */
async function mipTexture(name: string, levels: number) {
    const images = await Promise.all(
        Array.from({ length: levels + 1 }, (_, level) =>
            bitmap(sceneFile(`${level === 0 ? name : `${name}_mip${level}`}.png`)),
        ),
    )
    const texture = new THREE.Texture(images[0])
    texture.mipmaps = images
    texture.generateMipmaps = false
    texture.flipY = false
    texture.premultiplyAlpha = false
    texture.colorSpace = THREE.NoColorSpace
    texture.minFilter = THREE.NearestMipmapLinearFilter
    texture.magFilter = THREE.NearestFilter
    texture.needsUpdate = true
    return texture
}

/**
 * 20-byte vertices (xyz float, uv unorm16, brightness, block and sky light × 16,
 * material) and quad indices; `sensors` marks the quads of each sensor's tendrils.
 */
function quadGeometry(buffer: ArrayBuffer, quads: number, sensors: number[][]) {
    const vertices = quads * 4
    const sensor = new Uint8Array(vertices)
    sensors.forEach((list, index) => {
        for (const quad of list) sensor.fill(index + 1, quad * 4, quad * 4 + 4)
    })
    const floats = new THREE.InterleavedBuffer(new Float32Array(buffer, 0, vertices * 5), 5)
    const shorts = new THREE.InterleavedBuffer(new Uint16Array(buffer, 0, vertices * 10), 10)
    const bytes = new THREE.InterleavedBuffer(new Uint8Array(buffer, 0, vertices * 20), 20)
    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute('position', new THREE.InterleavedBufferAttribute(floats, 3, 0))
    geometry.setAttribute('uv', new THREE.InterleavedBufferAttribute(shorts, 2, 6, true))
    geometry.setAttribute('bright', new THREE.InterleavedBufferAttribute(bytes, 1, 16, true))
    geometry.setAttribute('light', new THREE.InterleavedBufferAttribute(bytes, 2, 17, false))
    geometry.setAttribute('material', new THREE.InterleavedBufferAttribute(bytes, 1, 19, false))
    geometry.setAttribute('sensor', new THREE.BufferAttribute(sensor, 1))
    geometry.setIndex(
        new THREE.BufferAttribute(new Uint16Array(buffer, vertices * 20, quads * 6), 1),
    )
    return geometry
}

/** The view direction of a Minecraft yaw and pitch, in degrees. */
export function viewVector(yaw: number, pitch: number, target = new THREE.Vector3()) {
    const y = (yaw * Math.PI) / 180
    const p = (pitch * Math.PI) / 180
    return target.set(-Math.sin(y) * Math.cos(p), -Math.sin(p), Math.cos(y) * Math.cos(p))
}

/** The camera rotation of a Minecraft yaw and pitch (no roll). */
export function viewRotation(yaw: number, pitch: number, target = new THREE.Quaternion()) {
    const forward = viewVector(yaw, pitch)
    const y = (yaw * Math.PI) / 180
    const right = new THREE.Vector3(-Math.cos(y), 0, -Math.sin(y))
    const up = new THREE.Vector3().crossVectors(right, forward)
    return target.setFromRotationMatrix(new THREE.Matrix4().makeBasis(right, up, forward.negate()))
}

export class CaptureWorld {
    readonly scene = new THREE.Scene()
    readonly material: THREE.RawShaderMaterial
    /**
     * The vanilla lightmap's inputs: the player under Night Vision, as when the
     * capture was taken, with the brightness option at "Bright"; no Darkness.
     */
    readonly light: LightmapParameters = {
        ambient: 0,
        skyFactor: 1,
        blockFactor: 1.5,
        skyLightColor: [1, 1, 1],
        nightVision: 1,
        darknessScale: 0,
        brightness: 1,
    }
    private readonly lightData = new Uint8Array(16 * 16 * 4)
    private readonly lightmap: THREE.DataTexture
    private readonly lightVolume: Uint8Array
    private tickers: SpriteTicker[] = []
    private readonly frames: THREE.Vector3[]
    private flicker = 0
    private finale: Promise<void> | null = null
    /** 1 for each sensor in its active or cooldown phase. */
    private readonly sensorActive: number[]

    private constructor(
        readonly meta: SceneMeta,
        view: ArrayBuffer,
        atlas: THREE.Texture,
        strip: THREE.Texture,
    ) {
        this.lightmap = new THREE.DataTexture(this.lightData, 16, 16, THREE.RGBAFormat)
        this.lightmap.magFilter = this.lightmap.minFilter = THREE.LinearFilter
        this.lightmap.colorSpace = THREE.NoColorSpace
        this.lightVolume = Uint8Array.from(atob(meta.light.data), (c) => c.charCodeAt(0))

        const animations = meta.animations
        this.frames = Array.from({ length: 9 }, () => new THREE.Vector3())
        const rects = Array.from({ length: 9 }, (_, index) => {
            const animation = animations[index]
            return animation
                ? new THREE.Vector4(
                      animation.atlas[0],
                      animation.atlas[1],
                      animation.size,
                      animation.stripY,
                  )
                : new THREE.Vector4()
        })

        this.sensorActive = new Array<number>(Math.max(1, meta.sensors.length)).fill(0)
        this.material = new THREE.RawShaderMaterial({
            glslVersion: THREE.GLSL3,
            vertexShader: VERTEX,
            fragmentShader: FRAGMENT,
            defines: { SENSORS: this.sensorActive.length },
            uniforms: {
                sensorActive: { value: this.sensorActive },
                tendrilActive: { value: meta.tendrilActive },
                lightmap: { value: this.lightmap },
                cameraPos: { value: new THREE.Vector3() },
                atlas: { value: atlas },
                strip: { value: strip },
                atlasSize: { value: new THREE.Vector2(meta.atlasSize, meta.atlasSize) },
                stripSize: { value: new THREE.Vector2(meta.stripSize[0], meta.stripSize[1]) },
                animRect: { value: rects },
                animFrame: { value: this.frames },
                fogColor: { value: new THREE.Vector4(0, 0, 0, 1) },
                envFog: { value: new THREE.Vector2(1e6, 1e6) },
                renderFog: { value: new THREE.Vector2(1e6, 1e6) },
            },
            side: THREE.FrontSide,
        })
        this.addMesh(
            quadGeometry(
                view,
                meta.viewQuads,
                meta.sensors.map((sensor) => sensor.view),
            ),
        )
        this.restart()
    }

    /** Back to the instant of the capture: each sprite at the frame it showed, the light steady. */
    restart() {
        this.tickers = this.meta.animations.map((animation) => new SpriteTicker(animation))
        this.tickers.forEach((ticker, index) => ticker.write(this.frames[index]!))
        this.flicker = 0
        this.sensorActive.fill(0)
        Object.assign(this.light, { blockFactor: 1.5, darknessScale: 0, brightness: 1 })
        this.setFog(1e6, 1e6, 1e6, 1e6)
        this.updateLightmap()
    }

    static async load() {
        const response = await fetch(sceneFile('scene.json'))
        if (!response.ok) throw new Error('Could not load the capture scene')
        const meta = (await response.json()) as SceneMeta
        const [view, atlas, strip] = await Promise.all([
            fetch(sceneFile('view.bin')).then((r) => {
                if (!r.ok) throw new Error('Could not load the capture scene')
                return r.arrayBuffer()
            }),
            mipTexture('atlas', meta.mips),
            mipTexture('anim', meta.mips),
        ])
        return new CaptureWorld(meta, view, atlas, strip)
    }

    private addMesh(geometry: THREE.BufferGeometry) {
        const mesh = new THREE.Mesh(geometry, this.material)
        mesh.frustumCulled = false
        mesh.matrixAutoUpdate = false
        this.scene.add(mesh)
    }

    /** The faces only the encounter sees, fetched once the warden is on its way. */
    loadFinale() {
        this.finale ??= fetch(sceneFile('finale.bin'))
            .then((response) => {
                if (!response.ok) throw new Error('Could not load the capture scene')
                return response.arrayBuffer()
            })
            .then((buffer) =>
                this.addMesh(
                    quadGeometry(
                        buffer,
                        this.meta.finaleQuads,
                        this.meta.sensors.map((sensor) => sensor.finale),
                    ),
                ),
            )
            .catch(() => {
                this.finale = null
            })
        return this.finale
    }

    /** A sensor's block state: the active model (active or cooldown phase) or the inactive one. */
    setSensorActive(index: number, active: boolean) {
        this.sensorActive[index] = active ? 1 : 0
    }

    /** The capture's camera: where the player floated, and the 1.1 × FOV of flying. */
    captureCamera(camera: THREE.PerspectiveCamera) {
        const view = this.meta.camera
        camera.fov = view.fov
        camera.position.fromArray(view.eye)
        viewRotation(view.yaw, view.pitch, camera.quaternion)
        camera.updateProjectionMatrix()
        camera.updateMatrixWorld()
    }

    /** A client tick: sprite animations and the block light's flicker (`LightTexture.tick`). */
    tick() {
        this.tickers.forEach((ticker, index) => {
            ticker.tick()
            ticker.write(this.frames[index]!)
        })
        this.flicker += (Math.random() - Math.random()) * Math.random() * Math.random() * 0.1
        this.flicker *= 0.9
        this.light.blockFactor = this.flicker + 1.5
    }

    updateLightmap() {
        computeLightmap(this.lightData, this.light)
        this.lightmap.needsUpdate = true
    }

    /** `texelFetch` of the lightmap at a block and sky level, as entities and particles read it. */
    lightColor(block: number, sky: number, target = new THREE.Vector3()) {
        const i = (sky * 16 + block) * 4
        return target.set(
            this.lightData[i]! / 255,
            this.lightData[i + 1]! / 255,
            this.lightData[i + 2]! / 255,
        )
    }

    /** Block and sky light of the block containing a point; dark outside the exported box. */
    lightAt(x: number, y: number, z: number): [number, number] {
        const box = this.meta.light
        const bx = Math.floor(x) - box.x0
        const by = Math.floor(y) - box.y0
        const bz = Math.floor(z) - box.z0
        if (bx < 0 || by < 0 || bz < 0 || bx >= box.width || by >= box.height || bz >= box.depth)
            return [0, 0]
        const packed = this.lightVolume[(by * box.depth + bz) * box.width + bx]!
        return [packed >> 4, packed & 15]
    }

    /** Top of the floor under a point. */
    floorAt(x: number, z: number) {
        const floor = this.meta.floor
        const row = floor.heights[Math.floor(z) - floor.z0]
        return row?.[Math.floor(x) - floor.x0] ?? -2
    }

    setFog(
        environmentalStart: number,
        environmentalEnd: number,
        renderStart: number,
        renderEnd: number,
    ) {
        const uniforms = this.material.uniforms
        ;(uniforms.envFog!.value as THREE.Vector2).set(environmentalStart, environmentalEnd)
        ;(uniforms.renderFog!.value as THREE.Vector2).set(renderStart, renderEnd)
    }

    setCameraPosition(position: THREE.Vector3) {
        ;(this.material.uniforms.cameraPos!.value as THREE.Vector3).copy(position)
    }
}
