import * as THREE from 'three'

import { soundFile, textureFile } from './assets'
import {
    createSharedUniforms,
    FOG_GLSL,
    pixelTexture,
    type SharedUniforms,
    WARDEN_TEXTURES,
    WardenModel,
} from './model'
import { CaptureWorld } from './scene'

/**
 * The sculk shriekers of the ancient city capture, simulated at 20 ticks a
 * second from Minecraft's own rules (`SculkShriekerBlockEntity`,
 * `WardenSpawnTracker`, `Warden`, `WardenAi`, the Darkness effect, the
 * lightmap and the fog), inside the world the capture was taken in.
 *
 * Opened in the viewer, the still capture gives way to that world seen from
 * the same eyes: the same frame, now moving. What the shriekers do happens in
 * that world and only inside that frame, until the warden comes: then the
 * frame grows over the whole page and the view drops from where the player
 * floated to where they stand, in front of it.
 */

/** Where the warden's punch sends the viewer. */
const PUNCH_DESTINATION = 'https://github.com/khiqwq'
/** How long a delete waits for a shrieker to hear it before the capture is known to stay. */
const DELETE_ANSWER_TICKS = 60

const TICK_MS = 50
const NEAR = 0.05
const EYE_HEIGHT = 1.62
/**
 * The instance's render distance (23) and FOV (74). The brightness is the
 * "Bright" setting with the player under Night Vision, which matches the
 * capture as closely as the instance's own 3200 % (taken under Night Vision
 * too) and, unlike it, lets Darkness dim the world.
 */
const RENDER_DISTANCE = 23 * 16
const GAMMA = 1
const FOV = 74
/** Darkness Pulsing at vanilla's 100 % (the instance has it off; the egg shows it). */
const DARKNESS_PULSING = 1
/** The instance's "Decreased" particles: `calculateParticleLevel` drops one in three. */
const DROPPED_PARTICLES = 1 / 3

const SHRIEKING_TICKS = 90
const DARKNESS_DURATION = 260
const DARKNESS_DISPLAY_LIMIT = 200
const DARKNESS_BLEND_TICKS = 22
const DARKNESS_INTERVAL = 120
const SHRIEKER_DARKNESS_RADIUS = 40
const WARDEN_DARKNESS_RADIUS = 20
const EMERGE_DURATION = 134
const ROAR_DURATION = 84
const ROAR_SOUND_DELAY = 25
const VIBRATION_COOLDOWN = 40
const MELEE_ATTACK_COOLDOWN = 18
const WARDEN_HALF_WIDTH = 0.45
const WARDEN_EYE_HEIGHT = 2.9 * 0.85
const PLAYER_HALF_WIDTH = 0.3
const ATTACK_REACH = Math.sqrt(2.04) - 0.6
const FIGHT_SPEED = 0.3 * 1.2
const ANGRY = 80
const AGITATED = 40

/**
 * Where `SpawnUtil.trySpawnMob` lands the warden (within five blocks of the
 * shrieker, on top of the sculk floor) and where the player stands, south of
 * it, once the view has dropped into the world. Scene coordinates.
 */
const WARDEN_SPAWN = new THREE.Vector3(12.5, -2, -1.5)
const STAND_POINT = new THREE.Vector3(12.5, -2, 4.5)
/** Ticks from the summon until the view stands at the player's eyes and fills the page. */
const DESCENT_TICKS = 50
/**
 * The world is drawn at the capture's own resolution, so the still and the
 * live frame match; machines that cannot keep up get half of it.
 */
const CAPTURE_SCALE_FALLBACK = 0.5
/** Frames slower than this (25 a second) stop the drawing between ticks. */
const SLOW_FRAME_MS = 40

/** `SculkSensorBlock`: active for 30 ticks, then 10 of cooldown before it listens again. */
const SENSOR_ACTIVE_TICKS = 30
const SENSOR_COOLDOWN_TICKS = 10
/** How far from its sensor a delete's vibration starts: inside the 8-block listener range. */
const DELETE_VIBRATION_REACH = 7.5
/** `particle/vibration.png`: seven frames, one a tick. */
const VIBRATION_FRAMES = 7

const REPLY_SOUNDS: Record<number, string> = {
    1: 'entity.warden.nearby_close',
    2: 'entity.warden.nearby_closer',
}

function variants(prefix: string, count: number, separator = '_') {
    return Array.from({ length: count }, (_, index) => `${prefix}${separator}${index + 1}`)
}

/** `sounds.json` entries of the events the egg plays, by file under `sounds/`. */
const SOUNDS: Record<string, { files: string[]; volume?: number }> = {
    'block.sculk_shrieker.shriek': {
        files: variants('block.sculk_shrieker.shriek', 5, ''),
        volume: 0.85,
    },
    'entity.warden.nearby_close': { files: variants('mob.warden.nearby_close', 4) },
    'entity.warden.nearby_closer': { files: variants('mob.warden.nearby_closer', 3) },
    'entity.warden.agitated': { files: variants('mob.warden.agitated', 6) },
    'entity.warden.angry': { files: variants('mob.warden.angry', 6) },
    'entity.warden.emerge': { files: ['mob.warden.emerge'] },
    'entity.warden.heartbeat': { files: variants('mob.warden.heartbeat', 4) },
    'entity.warden.listening': { files: variants('mob.warden.listening', 5) },
    'entity.warden.listening_angry': { files: variants('mob.warden.listening_angry', 5) },
    'entity.warden.tendril_clicks': { files: variants('mob.warden.tendril_clicks', 6) },
    'entity.warden.roar': { files: variants('mob.warden.roar', 5) },
    'entity.warden.step': { files: variants('mob.warden.step', 4), volume: 0.3 },
    'entity.warden.attack_impact': { files: variants('mob.warden.attack_impact', 2) },
    'entity.player.hurt': { files: variants('damage.hit', 3, '') },
    'block.sculk_sensor.clicking': {
        files: variants('block.sculk_sensor.sculk_clicking', 6, ''),
        volume: 0.73,
    },
    'block.sculk_sensor.clicking_stop': {
        files: variants('block.sculk_sensor.sculk_clicking_stop', 5, ''),
        volume: 0.62,
    },
}

const random = Math.random
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))
const lerp = (t: number, from: number, to: number) => from + (to - from) * t
const randomBetween = (min: number, max: number) => min + random() * (max - min)
const randomBetweenInclusive = (min: number, max: number) =>
    min + Math.floor(random() * (max - min + 1))
/** `LivingEntity.getVoicePitch` of an adult. */
const voicePitch = () => (random() - random()) * 0.2 + 1
/** `LightTexture.getBrightness` in the overworld (no ambient light). */
const lightBrightness = (level: number) => {
    const f = level / 15
    return f / (4 - 3 * f)
}

function wrapDegrees(degrees: number) {
    let wrapped = degrees % 360
    if (wrapped >= 180) wrapped -= 360
    if (wrapped < -180) wrapped += 360
    return wrapped
}

const rotLerp = (t: number, from: number, to: number) => from + t * wrapDegrees(to - from)
const rotateTowards = (from: number, to: number, max: number) =>
    from + clamp(wrapDegrees(to - from), -max, max)
const rotateIfNecessary = (from: number, to: number, max: number) =>
    to - clamp(wrapDegrees(to - from), -max, max)
const yawTowards = (dx: number, dz: number) => (Math.atan2(dz, dx) * 180) / Math.PI - 90
const pitchTowards = (dy: number, horizontal: number) =>
    -((Math.atan2(dy, horizontal) * 180) / Math.PI)

function loadImage(name: string) {
    return new Promise<HTMLImageElement>((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error(`Could not load ${name}`))
        image.src = textureFile(name)
    })
}

/** `SoundEngine.play` through Web Audio: OpenAL's linear distance model and stereo panning. */
class Sounds {
    private readonly buffers = new Map<string, Promise<AudioBuffer | null>>()

    constructor(private readonly context: AudioContext) {}

    preload() {
        for (const event of Object.values(SOUNDS))
            for (const file of event.files) void this.buffer(file)
    }

    private buffer(file: string) {
        let pending = this.buffers.get(file)
        if (!pending) {
            pending = fetch(soundFile(file))
                .then((response) => {
                    if (!response.ok) throw new Error(`Could not load ${file}`)
                    return response.arrayBuffer()
                })
                .then((data) => this.context.decodeAudioData(data))
                .catch(() => null)
            this.buffers.set(file, pending)
        }
        return pending
    }

    listen(position: THREE.Vector3, forward: THREE.Vector3, up: THREE.Vector3) {
        const listener = this.context.listener
        if (listener.positionX) {
            listener.positionX.value = position.x
            listener.positionY.value = position.y
            listener.positionZ.value = position.z
            listener.forwardX.value = forward.x
            listener.forwardY.value = forward.y
            listener.forwardZ.value = forward.z
            listener.upX.value = up.x
            listener.upY.value = up.y
            listener.upZ.value = up.z
        } else {
            listener.setPosition(position.x, position.y, position.z)
            listener.setOrientation(forward.x, forward.y, forward.z, up.x, up.y, up.z)
        }
    }

    /**
     * The instance's volume times the definition's, clamped to 1; the pitch
     * clamped to [0.5, 2]; heard up to `max(volume, 1) × 16` blocks away.
     */
    play(event: string, position: THREE.Vector3, volume: number, pitch: number) {
        const definition = SOUNDS[event]
        if (!definition) return
        const file = definition.files[Math.floor(random() * definition.files.length)]
        if (!file) return
        const requestedAt = performance.now()
        const at = position.clone()
        void this.buffer(file).then((buffer) => {
            if (!buffer || this.context.state === 'closed') return
            if (performance.now() - requestedAt > 1000) return
            const source = new AudioBufferSourceNode(this.context, {
                buffer,
                playbackRate: clamp(pitch, 0.5, 2),
            })
            const gain = new GainNode(this.context, {
                gain: clamp(volume * (definition.volume ?? 1), 0, 1),
            })
            const panner = new PannerNode(this.context, {
                panningModel: 'equalpower',
                distanceModel: 'linear',
                refDistance: 0,
                maxDistance: Math.max(volume, 1) * 16,
                rolloffFactor: 1,
                positionX: at.x,
                positionY: at.y,
                positionZ: at.z,
            })
            source.connect(gain).connect(panner).connect(this.context.destination)
            source.start()
        })
    }
}

/** A `MobEffectInstance` of Darkness on the player, with its `BlendState`. */
interface DarknessEffect {
    duration: number
    factor: number
    factorPrevious: number
}

const PARTICLE_VERTEX = /* glsl */ `
attribute vec4 tint;
varying vec2 texCoord0;
varying vec4 vertexColor;
varying vec3 relativePosition;

void main() {
    vec4 world = modelMatrix * vec4(position, 1.0);
    relativePosition = world.xyz - cameraPosition;
    texCoord0 = uv;
    // the tint carries the colour times the lightmap texel of the particle's light
    vertexColor = tint;
    gl_Position = projectionMatrix * viewMatrix * world;
}
`

/**
 * `core/particle.fsh`. The sprite is a strip of `frames` animation frames;
 * `frame` and `frameNext` are blended like an interpolated atlas animation.
 */
const PARTICLE_FRAGMENT = /* glsl */ `
uniform sampler2D map;
uniform float frames;
uniform float frame;
uniform float frameNext;
uniform float frameMix;
varying vec2 texCoord0;
varying vec4 vertexColor;
varying vec3 relativePosition;
${FOG_GLSL}

void main() {
    vec4 current = texture2D(map, vec2(texCoord0.x, (frame + texCoord0.y) / frames));
    vec4 next = texture2D(map, vec2(texCoord0.x, (frameNext + texCoord0.y) / frames));
    vec4 color = mix(current, next, frameMix) * vertexColor;
    if (color.a < 0.1) {
        discard;
    }
    gl_FragColor = apply_fog(color, relativePosition);
}
`

/** `rendertype_entity_shadow`: the shadow texture tinted by its alpha, then fog. */
const SHADOW_FRAGMENT = /* glsl */ `
uniform sampler2D map;
varying vec2 texCoord0;
varying vec4 vertexColor;
varying vec3 relativePosition;
${FOG_GLSL}

void main() {
    vec4 color = texture2D(map, texCoord0) * vertexColor;
    gl_FragColor = apply_fog(color, relativePosition);
}
`

/** Quads rebuilt every frame, laid out as `SingleQuadParticle` emits its four corners. */
class QuadBatch {
    readonly mesh: THREE.Mesh
    private readonly positions: Float32Array
    private readonly uvs: Float32Array
    private readonly tints: Float32Array
    private count = 0
    private readonly corner = new THREE.Vector3()

    constructor(
        private readonly capacity: number,
        material: THREE.Material,
    ) {
        const geometry = new THREE.BufferGeometry()
        this.positions = new Float32Array(capacity * 12)
        this.uvs = new Float32Array(capacity * 8)
        this.tints = new Float32Array(capacity * 16)
        const indices = new Uint32Array(capacity * 6)
        for (let quad = 0; quad < capacity; quad++) {
            indices.set(
                [0, 1, 2, 0, 2, 3].map((offset) => quad * 4 + offset),
                quad * 6,
            )
        }
        geometry.setAttribute('position', new THREE.BufferAttribute(this.positions, 3))
        geometry.setAttribute('uv', new THREE.BufferAttribute(this.uvs, 2))
        geometry.setAttribute('tint', new THREE.BufferAttribute(this.tints, 4))
        geometry.setIndex(new THREE.BufferAttribute(indices, 1))
        this.mesh = new THREE.Mesh(geometry, material)
        this.mesh.frustumCulled = false
    }

    begin() {
        this.count = 0
    }

    add(
        center: THREE.Vector3,
        rotation: THREE.Quaternion,
        size: number,
        uv: [number, number, number, number],
        tint: [number, number, number, number],
    ) {
        if (this.count >= this.capacity) return
        const [u0, u1, v0, v1] = uv
        const corners: [number, number, number, number][] = [
            [1, -1, u1, v1],
            [1, 1, u1, v0],
            [-1, 1, u0, v0],
            [-1, -1, u0, v1],
        ]
        corners.forEach(([x, y, u, v], index) => {
            const vertex = this.count * 4 + index
            this.corner.set(x, y, 0).applyQuaternion(rotation).multiplyScalar(size).add(center)
            this.positions.set([this.corner.x, this.corner.y, this.corner.z], vertex * 3)
            this.uvs.set([u, v], vertex * 2)
            this.tints.set(tint, vertex * 4)
        })
        this.count++
    }

    end() {
        const geometry = this.mesh.geometry
        geometry.setDrawRange(0, this.count * 6)
        for (const name of ['position', 'uv', 'tint'])
            geometry.getAttribute(name).needsUpdate = true
    }
}

interface ShriekParticle {
    x: number
    y: number
    yo: number
    z: number
    yd: number
    age: number
    delay: number
}

/** A sculk sensor's `SculkSensorPhase` and `VibrationSystem` state. */
interface Sensor {
    index: number
    center: THREE.Vector3
    radius: number
    visible: boolean
    /** Shrieker index and distance of each shrieker its clicking reaches. */
    reaches: [number, number][]
    phase: 'inactive' | 'active' | 'cooldown'
    phaseEnds: number
    /** A vibration is on its way to it (a listener takes one at a time). */
    listening: boolean
}

interface Shrieker {
    center: THREE.Vector3
    shriekingUntil: number
    listening: boolean
}

/**
 * A vibration on its way from where it happened to the listener that took it:
 * `VibrationInfo` travelling `floor(distance)` ticks, drawn as the
 * `VibrationSignalParticle` the server sends for it.
 */
interface VibrationSignal {
    x: number
    y: number
    z: number
    xo: number
    yo: number
    zo: number
    rot: number
    rotO: number
    pitch: number
    pitchO: number
    age: number
    lifetime: number
    target: THREE.Vector3
    arrive: () => void
}

/** `WardenSpawnTracker` of the player, kept by the page across viewings. */
export interface WarningTracker {
    /** `tryWarn`: the new warning level and whether it summons, or null while cooling down. */
    tryWarn(): { level: number; summon: boolean } | null
    /** The summoned warden has emerged: no shrieker warns again. */
    wardenEmerged(): void
}

/** `TerrainParticle` of the block the warden digs out of. */
interface TerrainParticle {
    x: number
    y: number
    z: number
    xo: number
    yo: number
    zo: number
    xd: number
    yd: number
    zd: number
    age: number
    lifetime: number
    size: number
    uo: number
    vo: number
    stopped: boolean
}

function terrainParticle(x: number, y: number, z: number): TerrainParticle {
    // `Particle(level, x, y, z, 0, 0, 0)`: a random direction and speed, nudged up.
    let xd = (random() * 2 - 1) * 0.4
    let yd = (random() * 2 - 1) * 0.4
    let zd = (random() * 2 - 1) * 0.4
    const speed = (random() + random() + 1) * 0.15
    const length = Math.hypot(xd, yd, zd) || 1
    xd = (xd / length) * speed * 0.4
    yd = (yd / length) * speed * 0.4 + 0.1
    zd = (zd / length) * speed * 0.4
    return {
        x,
        y,
        z,
        xo: x,
        yo: y,
        zo: z,
        xd,
        yd,
        zd,
        age: 0,
        lifetime: Math.trunc(4 / (random() * 0.9 + 0.1)),
        size: (0.1 * (random() * 0.5 + 0.5) * 2) / 2,
        uo: random() * 3,
        vo: random() * 3,
        stopped: false,
    }
}

type WardenPose = 'emerging' | 'standing' | 'roaring'

/** The summoned warden: the parts of `Warden`, `WardenAi` and its controls the encounter reaches. */
class Warden {
    tickCount = 0
    x: number
    y: number
    z: number
    xo: number
    yo: number
    zo: number
    xd = 0
    zd = 0
    bodyRot: number
    bodyRotO: number
    headRot: number
    headRotO: number
    xRot = 0
    xRotO = 0
    pose: WardenPose = 'emerging'
    anger = 0
    clientAnger = 0
    tendril = 0
    tendrilO = 0
    heart = 0
    heartO = 0
    walkPosition = 0
    walkSpeed = 0
    walkSpeedO = 0
    emergeStart: number | null = 0
    roarStart: number | null = null
    attackStart: number | null = null
    nextVibration = 0
    roarEndsAt = 0
    roarSoundAt = 0
    fighting = false
    attackCooldown = 0
    ambientSoundTime = 0
    moveDist = 0
    nextStep = 1
    lookingAtPlayer = false
    headStableTime = 0
    lastStableHeadRot: number
    hits = 0

    constructor(at: THREE.Vector3, yaw: number) {
        this.x = this.xo = at.x
        this.y = this.yo = at.y
        this.z = this.zo = at.z
        this.bodyRot = this.bodyRotO = this.headRot = this.headRotO = this.lastStableHeadRot = yaw
    }

    get angerLevel() {
        return this.anger >= ANGRY ? 'angry' : this.anger >= AGITATED ? 'agitated' : 'calm'
    }
}

/** The player, whose eyes are the camera once the view drops into the world. */
interface Player {
    x: number
    y: number
    z: number
    xo: number
    yo: number
    zo: number
    xd: number
    yd: number
    zd: number
    onGround: boolean
    hurtTime: number
    hurtDir: number
}

interface ScheduledEvent {
    at: number
    run: () => void
}

type Stage = 'idle' | 'loading' | 'capture' | 'finale'

export class WardenEngine {
    private sounds: Sounds | null = null
    private readonly shared: SharedUniforms = createSharedUniforms()
    private readonly camera = new THREE.PerspectiveCamera(70, 1, NEAR, 1024)
    private readonly capturePose = {
        position: new THREE.Vector3(),
        quaternion: new THREE.Quaternion(),
        fov: 70,
    }
    private renderer: THREE.WebGLRenderer | null = null
    private canvas: HTMLCanvasElement | null = null
    private readonly textures: Record<string, THREE.Texture>
    private readonly shriekBatch: QuadBatch
    private readonly signalBatch: QuadBatch
    private readonly signalMaterial: THREE.ShaderMaterial
    private readonly terrainBatch: QuadBatch
    private readonly shadowBatch: QuadBatch
    private readonly terrainMaterial: THREE.ShaderMaterial
    private model: WardenModel | null = null
    /** The capture's own camera, for placing the delete button's vibration. */
    private readonly captureView = new THREE.PerspectiveCamera(70, 1, NEAR, 1024)
    private sensors: Sensor[] = []
    private shriekers: Shrieker[] = []
    private signals: VibrationSignal[] = []
    private tracker: WarningTracker | null = null
    /** The shrieker whose shriek summoned the warden. */
    private readonly summoner = new THREE.Vector3()

    private stage: Stage = 'idle'
    private host: HTMLElement | null = null
    private mountToken = 0
    private readonly whenLive: (() => void)[] = []
    private readonly root = document.createElement('div')
    private readonly fadeLayer = document.createElement('div')
    /** Where the capture's frame was on screen when the warden came. */
    private frameRect = new DOMRect()
    private captureScale = 1
    private slowFrames = 0
    private slow = false
    private raf = 0
    private lastFrame = 0
    private lastRender = 0
    /** Milliseconds since the previous drawn frame, for the view's easing toward the warden. */
    private frameElapsed = 1000 / 60
    private accumulator = 0
    private tickCount = 0
    private darkness: DarknessEffect | null = null
    private readonly events: ScheduledEvent[] = []
    private shriekParticles: ShriekParticle[] = []
    private terrainParticles: TerrainParticle[] = []

    private warden: Warden | null = null
    private player: Player | null = null
    private summonedAt = 0
    private readonly lookPoint = new THREE.Vector3()
    /** Answers the delete being heard: true once the warden has the capture, false if it stays. */
    private answer: ((taken: boolean) => void) | null = null
    /** That delete summoned the warden, so it is answered once the warden's world covers the page. */
    private summoning = false
    private departed = false
    private readonly blockInput = (event: Event) => event.preventDefault()
    private readonly blockKeys = (event: KeyboardEvent) => {
        if ([' ', 'PageUp', 'PageDown', 'Home', 'End', 'ArrowUp', 'ArrowDown'].includes(event.key))
            event.preventDefault()
    }
    private readonly onPageShow = (event: PageTransitionEvent) => {
        if (event.persisted) this.reset()
    }

    constructor(
        private readonly world: CaptureWorld,
        images: Record<string, HTMLImageElement>,
    ) {
        this.textures = Object.fromEntries(
            Object.entries(images).map(([name, image]) => [name, pixelTexture(image)]),
        )
        this.shared.floorY.value = -1e6

        const particleMaterial = (map: THREE.Texture, frames: number, translucent: boolean) =>
            new THREE.ShaderMaterial({
                uniforms: {
                    ...this.shared,
                    map: { value: map },
                    frames: { value: frames },
                    frame: { value: 0 },
                    frameNext: { value: 0 },
                    frameMix: { value: 0 },
                },
                vertexShader: PARTICLE_VERTEX,
                fragmentShader: PARTICLE_FRAGMENT,
                transparent: translucent,
                side: THREE.FrontSide,
            })
        this.shriekBatch = new QuadBatch(20, particleMaterial(this.texture('shriek'), 1, true))
        this.signalMaterial = particleMaterial(this.texture('vibration'), VIBRATION_FRAMES, true)
        this.signalBatch = new QuadBatch(64, this.signalMaterial)
        this.terrainMaterial = particleMaterial(this.texture('sculk'), 4, false)
        this.terrainBatch = new QuadBatch(4096, this.terrainMaterial)
        this.shadowBatch = new QuadBatch(
            1,
            new THREE.ShaderMaterial({
                uniforms: { ...this.shared, map: { value: this.texture('shadow') } },
                vertexShader: PARTICLE_VERTEX,
                fragmentShader: SHADOW_FRAGMENT,
                transparent: true,
                depthWrite: false,
            }),
        )
        this.shriekBatch.mesh.renderOrder = 10
        this.signalBatch.mesh.renderOrder = 11
        this.terrainBatch.mesh.renderOrder = 9
        world.scene.add(
            this.shadowBatch.mesh,
            this.terrainBatch.mesh,
            this.shriekBatch.mesh,
            this.signalBatch.mesh,
        )

        world.captureCamera(this.camera)
        this.capturePose.position.copy(this.camera.position)
        this.capturePose.quaternion.copy(this.camera.quaternion)
        this.capturePose.fov = this.camera.fov
        world.captureCamera(this.captureView)
        this.captureView.aspect = world.meta.screenshot[0] / world.meta.screenshot[1]
        this.captureView.updateProjectionMatrix()
        this.resetBlocks()

        Object.assign(this.root.style, {
            position: 'fixed',
            inset: '0',
            zIndex: '2147483000',
            pointerEvents: 'auto',
        })
        Object.assign(this.fadeLayer.style, {
            position: 'fixed',
            inset: '0',
            background: '#000',
            opacity: '0',
        })
    }

    private texture(name: string) {
        const texture = this.textures[name]
        if (!texture) throw new Error(`Missing texture ${name}`)
        return texture
    }

    /** Every sensor inactive and every shrieker quiet, as at the capture's instant. */
    private resetBlocks() {
        const center = ([x, y, z]: [number, number, number]) =>
            new THREE.Vector3(x + 0.5, y + 0.5, z + 0.5)
        this.sensors = this.world.meta.sensors.map((sensor, index) => ({
            index,
            center: center(sensor.pos),
            radius: sensor.radius,
            visible: sensor.visible,
            reaches: sensor.reaches,
            phase: 'inactive',
            phaseEnds: 0,
            listening: false,
        }))
        this.shriekers = this.world.meta.shriekers.map((pos) => ({
            center: center(pos),
            shriekingUntil: 0,
            listening: false,
        }))
        this.signals = []
    }

    /** The Web Audio side, made on the click that asks for a delete. */
    useAudio(context: AudioContext) {
        if (this.sounds) return
        this.sounds = new Sounds(context)
        this.sounds.preload()
    }

    /**
     * Puts the live world over the capture in the viewer: the first frame is
     * drawn at the capture's instant, shown once it is on screen, and the world
     * starts ticking only then, so the still and the live frame meet exactly.
     */
    attach(host: HTMLElement) {
        if (this.stage === 'finale') return
        this.detachCanvas()
        this.host = host
        const token = ++this.mountToken
        this.stage = 'loading'
        let renderer: THREE.WebGLRenderer
        try {
            renderer = new THREE.WebGLRenderer({ antialias: false, alpha: false })
        } catch {
            this.stage = 'idle'
            this.whenLive.length = 0
            this.answerDelete(false)
            return
        }
        renderer.outputColorSpace = THREE.LinearSRGBColorSpace
        renderer.setClearColor(0x000000)
        renderer.setPixelRatio(1)
        const canvas: HTMLCanvasElement = renderer.domElement
        this.renderer = renderer
        this.canvas = canvas
        Object.assign(canvas.style, {
            position: 'absolute',
            inset: '0',
            width: '100%',
            height: '100%',
            // As the viewer fits the still (`object-contain`).
            objectFit: 'contain',
            opacity: '0',
            transition: 'opacity 160ms linear',
        })
        this.resizeCapture()
        host.append(canvas)

        this.world.restart()
        this.resetBlocks()
        this.tickCount = 0
        this.slowFrames = 0
        this.slow = false
        this.render(0)
        requestAnimationFrame(() =>
            requestAnimationFrame(() => {
                if (token !== this.mountToken || !this.canvas) return
                this.canvas.style.opacity = '1'
                window.setTimeout(() => {
                    if (token !== this.mountToken) return
                    this.stage = 'capture'
                    this.startLoop()
                    for (const run of this.whenLive.splice(0)) run()
                }, 180)
            }),
        )
    }

    /** The viewer moved on: the world stops, and nothing of it outlives the frame. */
    detach(host: HTMLElement) {
        if (this.host !== host || this.stage === 'finale') return
        this.mountToken++
        this.host = null
        this.stage = 'idle'
        this.whenLive.length = 0
        // A warden on its way stops coming with the world, so the capture stays.
        this.summoning = false
        this.answerDelete(false)
        this.clearWorld()
        this.stopLoop()
        this.detachCanvas()
    }

    private answerDelete(taken: boolean) {
        const answer = this.answer
        this.answer = null
        answer?.(taken)
    }

    private detachCanvas() {
        this.renderer?.dispose()
        this.renderer?.forceContextLoss()
        this.canvas?.remove()
        this.renderer = null
        this.canvas = null
    }

    private clearWorld() {
        this.darkness = null
        this.events.length = 0
        this.shriekParticles = []
        this.terrainParticles = []
        this.signals = []
    }

    private resizeCapture() {
        const [width, height] = this.world.meta.screenshot
        this.renderer?.setSize(
            Math.round(width * this.captureScale),
            Math.round(height * this.captureScale),
            false,
        )
        this.camera.aspect = width / height
    }

    /**
     * A delete of the capture, as Minecraft would hear it: a vibration from the
     * delete button's side reaches the sculk sensor nearest the button among
     * those the capture shows and that a shrieker can hear. Every sensor in
     * range takes it; an active sensor's clicking travels on to the shriekers
     * within 8 blocks, and the first to hear it tries to shriek
     * (`SculkShriekerBlockEntity.tryShriek`, through `tracker`).
     * `answer` learns whether the capture goes: true once a summoned warden's
     * world covers the page, false as soon as it is clear that it stays.
     */
    vibrate(
        button: () => Element | null,
        tracker: WarningTracker,
        answer: (taken: boolean) => void,
    ) {
        if (this.stage === 'finale' || this.summoning) {
            answer(false)
            return
        }
        this.answerDelete(false)
        this.tracker = tracker
        this.answer = answer
        // The world beyond the frame starts loading with the first delete.
        void this.world.loadFinale()
        const run = () => this.deleteVibration(button()?.getBoundingClientRect() ?? null)
        if (this.stage === 'capture') run()
        else this.whenLive.push(run)
    }

    /** Where on screen a point of the world is in the capture's frame. */
    private onScreen(point: THREE.Vector3, frame: DOMRect) {
        const ndc = point.clone().project(this.captureView)
        return {
            x: frame.left + ((ndc.x + 1) / 2) * frame.width,
            y: frame.top + ((1 - ndc.y) / 2) * frame.height,
        }
    }

    private deleteVibration(button: DOMRect | null) {
        const canvas = this.canvas
        if (!canvas) return
        // Nothing may hear it: every sensor in reach can be busy, or the shriekers cooling down.
        this.schedule(DELETE_ANSWER_TICKS, () => {
            if (!this.summoning) this.answerDelete(false)
        })
        const frame = canvas.getBoundingClientRect()
        const at = button
            ? { x: button.left + button.width / 2, y: button.top + button.height / 2 }
            : { x: frame.left + frame.width / 2, y: frame.bottom }
        let sensor: Sensor | null = null
        let nearest = Infinity
        for (const candidate of this.sensors) {
            if (!candidate.visible || candidate.reaches.length === 0) continue
            const point = this.onScreen(candidate.center, frame)
            const distance = Math.hypot(point.x - at.x, point.y - at.y)
            if (distance < nearest) {
                nearest = distance
                sensor = candidate
            }
        }
        if (!sensor) return
        // The vibration starts on the line from the eyes through the button, at
        // the sensor's depth, brought within its range.
        const view = this.captureView
        const ray = new THREE.Vector3(
            ((at.x - frame.left) / frame.width) * 2 - 1,
            1 - ((at.y - frame.top) / frame.height) * 2,
            0.5,
        )
            .unproject(view)
            .sub(view.position)
            .normalize()
        const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(view.quaternion)
        const depth = sensor.center.clone().sub(view.position).dot(forward)
        const offset = view.position
            .clone()
            .addScaledVector(ray, depth / Math.max(0.01, ray.dot(forward)))
            .sub(sensor.center)
        if (offset.length() > DELETE_VIBRATION_REACH) offset.setLength(DELETE_VIBRATION_REACH)
        const source = sensor.center.clone().add(offset)
        // `GameEventDispatcher.post`: every sensor whose range covers it.
        for (const listener of this.sensors) {
            if (listener.phase !== 'inactive' || listener.listening) continue
            const distance = source.distanceTo(listener.center)
            if (distance > listener.radius) continue
            listener.listening = true
            this.sendSignal(source, listener.center, distance, () => {
                listener.listening = false
                this.activateSensor(listener)
            })
        }
    }

    /** `VibrationSystem.Listener.scheduleVibration` and the particle the server sends for it. */
    private sendSignal(
        from: THREE.Vector3,
        to: THREE.Vector3,
        distance: number,
        arrive: () => void,
    ) {
        const dx = from.x - to.x
        const dy = from.y - to.y
        const dz = from.z - to.z
        const rot = Math.atan2(dx, dz)
        const pitch = Math.atan2(dy, Math.hypot(dx, dz))
        this.signals.push({
            x: from.x,
            y: from.y,
            z: from.z,
            xo: from.x,
            yo: from.y,
            zo: from.z,
            rot,
            rotO: rot,
            pitch,
            pitchO: pitch,
            age: 0,
            lifetime: Math.floor(distance),
            target: to.clone(),
            arrive,
        })
    }

    /** `VibrationSignalParticle.tick`, and the vibration's arrival when its travel time is up. */
    private tickSignals() {
        const arrived: VibrationSignal[] = []
        this.signals = this.signals.filter((signal) => {
            signal.xo = signal.x
            signal.yo = signal.y
            signal.zo = signal.z
            signal.rotO = signal.rot
            signal.pitchO = signal.pitch
            if (signal.age++ >= signal.lifetime) {
                arrived.push(signal)
                return false
            }
            const left = signal.lifetime - signal.age
            const t = left > 0 ? 1 / left : 1
            const target = signal.target
            signal.x = lerp(t, signal.x, target.x)
            signal.y = lerp(t, signal.y, target.y)
            signal.z = lerp(t, signal.z, target.z)
            const dx = signal.x - target.x
            const dy = signal.y - target.y
            const dz = signal.z - target.z
            if (dx !== 0 || dz !== 0) signal.rot = Math.atan2(dx, dz)
            if (dx !== 0 || dy !== 0 || dz !== 0) signal.pitch = Math.atan2(dy, Math.hypot(dx, dz))
            return true
        })
        for (const signal of arrived) signal.arrive()
    }

    /**
     * `SculkSensorBlock.activate`: the active model, the clicking, and the
     * `SCULK_SENSOR_TENDRILS_CLICKING` game event the shriekers listen for.
     */
    private activateSensor(sensor: Sensor) {
        if (sensor.phase !== 'inactive') return
        sensor.phase = 'active'
        sensor.phaseEnds = this.tickCount + SENSOR_ACTIVE_TICKS
        this.world.setSensorActive(sensor.index, true)
        this.sounds?.play('block.sculk_sensor.clicking', sensor.center, 1, random() * 0.2 + 0.8)
        for (const [index, distance] of sensor.reaches) {
            const shrieker = this.shriekers[index]
            if (!shrieker || shrieker.listening || this.tickCount < shrieker.shriekingUntil)
                continue
            shrieker.listening = true
            this.sendSignal(sensor.center, shrieker.center, distance, () => {
                shrieker.listening = false
                this.shriekerHears(shrieker)
            })
        }
    }

    /** `SculkSensorBlock.tick`: active → cooldown (with the stop of the clicking) → inactive. */
    private tickSensors() {
        for (const sensor of this.sensors) {
            if (sensor.phase === 'inactive' || this.tickCount < sensor.phaseEnds) continue
            if (sensor.phase === 'active') {
                sensor.phase = 'cooldown'
                sensor.phaseEnds = this.tickCount + SENSOR_COOLDOWN_TICKS
                this.sounds?.play(
                    'block.sculk_sensor.clicking_stop',
                    sensor.center,
                    1,
                    random() * 0.2 + 0.8,
                )
            } else {
                sensor.phase = 'inactive'
                this.world.setSensorActive(sensor.index, false)
            }
        }
    }

    /**
     * `SculkShriekerBlockEntity.tryShriek`: a shrieker that can summon shrieks
     * only if the player's spawn tracker takes the warning (not while it cools
     * down, nor with a warden around).
     */
    private shriekerHears(shrieker: Shrieker) {
        if (this.tickCount < shrieker.shriekingUntil || this.warden || this.summoning) return
        const warning = this.tracker?.tryWarn()
        if (warning?.summon) this.summoning = true
        else this.answerDelete(false)
        if (!warning) return
        this.shriek(shrieker, warning.level, warning.summon)
    }

    /**
     * Level event 3007 at the shrieker, then `tryRespond` 90 ticks later: the
     * reply of the warning level, or the warden, and Darkness either way.
     */
    private shriek(shrieker: Shrieker, warningLevel: number, summon: boolean) {
        shrieker.shriekingUntil = this.tickCount + SHRIEKING_TICKS
        const at = shrieker.center
        for (let index = 0; index < 10; index++) {
            if (random() < DROPPED_PARTICLES) continue
            this.shriekParticles.push({
                x: at.x,
                y: at.y,
                yo: at.y,
                z: at.z,
                yd: 0.1,
                age: 0,
                delay: index * 5,
            })
        }
        this.sounds?.play('block.sculk_shrieker.shriek', at, 2, 0.6 + random() * 0.4)
        this.schedule(SHRIEKING_TICKS, () => {
            if (summon) {
                this.summoner.copy(at)
                this.summon()
            } else {
                const event = REPLY_SOUNDS[warningLevel]
                if (event) {
                    this.sounds?.play(
                        event,
                        new THREE.Vector3(
                            Math.floor(at.x) + randomBetweenInclusive(-10, 10),
                            Math.floor(at.y) + randomBetweenInclusive(-10, 10),
                            Math.floor(at.z) + randomBetweenInclusive(-10, 10),
                        ),
                        5,
                        1,
                    )
                }
            }
            this.applyDarkness(at, SHRIEKER_DARKNESS_RADIUS)
        })
    }

    private schedule(delay: number, run: () => void) {
        this.events.push({ at: this.tickCount + delay, run })
    }

    private eyePosition() {
        const player = this.player
        if (!player) return this.capturePose.position.clone()
        return new THREE.Vector3(player.x, player.y + EYE_HEIGHT, player.z)
    }

    private playerPosition() {
        return this.eyePosition().setY(this.eyePosition().y - EYE_HEIGHT)
    }

    /** `Warden.applyDarknessAround`: 260 ticks to a player in range, unless more than 200 are left. */
    private applyDarkness(from: THREE.Vector3, radius: number) {
        if (this.departed) return
        if (this.playerPosition().distanceTo(from) >= radius) return
        const effect = this.darkness
        if (!effect) {
            this.darkness = { duration: DARKNESS_DURATION, factor: 0, factorPrevious: 0 }
        } else if (effect.duration <= DARKNESS_DISPLAY_LIMIT - 1) {
            effect.duration = Math.max(effect.duration, DARKNESS_DURATION)
        }
    }

    private startLoop() {
        if (this.raf) return
        this.lastFrame = performance.now()
        this.accumulator = 0
        this.raf = requestAnimationFrame(this.frame)
    }

    private stopLoop() {
        cancelAnimationFrame(this.raf)
        this.raf = 0
    }

    /** Anything that moves between ticks: then every frame is drawn, else one a tick. */
    private busy() {
        return (
            this.darkness !== null ||
            this.warden !== null ||
            this.shriekParticles.length > 0 ||
            this.terrainParticles.length > 0 ||
            this.signals.length > 0
        )
    }

    private readonly frame = (now: number) => {
        this.raf = requestAnimationFrame(this.frame)
        this.accumulator += Math.min(now - this.lastFrame, 1000)
        this.lastFrame = now
        let ticks = 0
        while (this.accumulator >= TICK_MS && ticks < 10) {
            this.tick()
            this.accumulator -= TICK_MS
            ticks++
        }
        if (ticks === 10) this.accumulator = 0
        if (!this.renderer) return
        const busy = this.busy()
        const sinceRender = now - this.lastRender
        // Every frame (up to 60 a second) while something moves, unless the
        // machine cannot keep up; one a tick otherwise.
        if (!(ticks > 0 || (busy && !this.slow && sinceRender >= 15))) return
        this.pace(sinceRender, busy)
        this.frameElapsed = Math.min(sinceRender, 250)
        this.lastRender = now
        if (this.stage === 'finale') this.layoutFinale()
        this.render(busy ? this.accumulator / TICK_MS : 0)
    }

    /**
     * Low-end machines: frames that take too long stop the drawing between
     * ticks, and if even one frame a tick is too much the capture's frame is
     * drawn at half its resolution.
     */
    private pace(sinceRender: number, busy: boolean) {
        if (sinceRender > 1000) return
        const limit = this.slow || !busy ? 2 * TICK_MS + 30 : SLOW_FRAME_MS
        this.slowFrames =
            sinceRender > limit ? this.slowFrames + 1 : Math.max(0, this.slowFrames - 1)
        if (this.slowFrames < 20) return
        this.slowFrames = 0
        if (!this.slow) {
            this.slow = true
        } else if (this.captureScale > CAPTURE_SCALE_FALLBACK) {
            this.captureScale = CAPTURE_SCALE_FALLBACK
            if (this.stage !== 'finale') this.resizeCapture()
        }
    }

    private tick() {
        this.tickCount++
        this.world.tick()
        const due = this.events.filter((event) => event.at <= this.tickCount)
        if (due.length) {
            this.events.splice(
                0,
                this.events.length,
                ...this.events.filter((e) => !due.includes(e)),
            )
            for (const event of due) event.run()
        }
        this.tickSignals()
        this.tickSensors()
        this.tickDarkness()
        this.tickShriekParticles()
        this.tickTerrainParticles()
        this.tickPlayer()
        this.tickWarden()
    }

    /** `MobEffectInstance.tick` and `BlendState.tick`: 22 ticks in, 22 ticks out before the end. */
    private tickDarkness() {
        const effect = this.darkness
        if (!effect) return
        effect.duration--
        effect.factorPrevious = effect.factor
        const target = effect.duration > DARKNESS_BLEND_TICKS ? 1 : 0
        const step = 1 / DARKNESS_BLEND_TICKS
        effect.factor += clamp(target - effect.factor, -step, step)
        if (effect.duration <= 0) this.darkness = null
    }

    private blendFactor(partial: number) {
        const effect = this.darkness
        return effect ? lerp(partial, effect.factorPrevious, effect.factor) : 0
    }

    private tickShriekParticles() {
        this.shriekParticles = this.shriekParticles.filter((particle) => {
            if (particle.delay > 0) {
                particle.delay--
                return true
            }
            particle.yo = particle.y
            if (particle.age++ >= 30) return false
            particle.y += particle.yd
            particle.yd *= 0.98
            return true
        })
    }

    private tickTerrainParticles() {
        this.terrainParticles = this.terrainParticles.filter((particle) => {
            particle.xo = particle.x
            particle.yo = particle.y
            particle.zo = particle.z
            if (particle.age++ >= particle.lifetime) return false
            particle.yd -= 0.04
            let onGround = false
            if (!particle.stopped) {
                const originalYa = particle.yd
                let ya = particle.yd
                const floor = this.world.floorAt(particle.x, particle.z)
                if (particle.y + ya < floor) ya = floor - particle.y
                particle.x += particle.xd
                particle.y += ya
                particle.z += particle.zd
                if (Math.abs(originalYa) >= 1.0e-5 && Math.abs(ya) < 1.0e-5) particle.stopped = true
                onGround = originalYa !== ya && originalYa < 0
            }
            particle.xd *= 0.98
            particle.yd *= 0.98
            particle.zd *= 0.98
            if (onGround) {
                particle.xd *= 0.7
                particle.zd *= 0.7
            }
            return true
        })
    }

    /** The player on the city's floor: hurt time and the punch's knockback. */
    private tickPlayer() {
        const player = this.player
        if (!player) return
        player.xo = player.x
        player.yo = player.y
        player.zo = player.z
        if (player.hurtTime > 0) player.hurtTime--
        if (!player.onGround || player.xd !== 0 || player.zd !== 0 || player.yd > 0) {
            player.x += player.xd
            player.y += player.yd
            player.z += player.zd
            const floor = this.world.floorAt(player.x, player.z)
            if (player.y <= floor) {
                player.y = floor
                player.yd = 0
                player.onGround = true
            } else {
                player.onGround = false
            }
            const friction = player.onGround ? 0.546 : 0.91
            player.xd *= friction
            player.zd *= friction
            player.yd = (player.yd - 0.08) * 0.98
            if (player.onGround) player.yd = Math.max(0, player.yd)
            if (Math.abs(player.xd) < 0.003) player.xd = 0
            if (Math.abs(player.zd) < 0.003) player.zd = 0
        }
    }

    /**
     * Spawns the warden as `SpawnUtil.trySpawnMob` with `TRIGGERED` does —
     * emerging, agitated — and lifts the capture's frame out of the viewer to
     * grow over the page while the view drops to the player's eyes.
     */
    private summon() {
        // Within five blocks of the shrieker, as `trySpawnMob` searches; the
        // player stands six blocks south of the warden.
        const block = (value: number) => Math.floor(value) + 0.5
        const spawn = new THREE.Vector3(
            clamp(WARDEN_SPAWN.x, block(this.summoner.x) - 5, block(this.summoner.x) + 5),
            0,
            clamp(WARDEN_SPAWN.z, block(this.summoner.z) - 5, block(this.summoner.z) + 5),
        )
        const stand = spawn.clone().add(STAND_POINT).sub(WARDEN_SPAWN)
        const yaw = yawTowards(stand.x - spawn.x, stand.z - spawn.z) + 25
        const warden = new Warden(spawn, yaw)
        warden.y = warden.yo = this.wardenFloor(warden.x, warden.z)
        this.warden = warden
        this.summonedAt = this.tickCount
        this.tracker?.wardenEmerged()
        const standY = this.world.floorAt(stand.x, stand.z)
        this.player = {
            x: stand.x,
            y: standY,
            z: stand.z,
            xo: stand.x,
            yo: standY,
            zo: stand.z,
            xd: 0,
            yd: 0,
            zd: 0,
            onGround: true,
            hurtTime: 0,
            hurtDir: 0,
        }
        this.lookPoint.set(warden.x, warden.y, warden.z)
        this.sounds?.play('entity.warden.agitated', this.wardenPosition(), 5, 1)
        this.schedule(1, () =>
            this.sounds?.play('entity.warden.emerge', this.wardenPosition(), 5, 1),
        )
        if (!this.model) {
            this.model = new WardenModel(this.textures, this.shared)
            this.world.scene.add(this.model.root)
        }
        this.enterFinale()
    }

    /** Takes the frame out of the viewer, where it was, into a layer over the whole page. */
    private enterFinale() {
        const canvas = this.canvas
        if (!canvas) return
        this.stage = 'finale'
        this.frameRect = canvas.getBoundingClientRect()
        Object.assign(canvas.style, {
            position: 'fixed',
            inset: 'auto',
            transition: 'none',
            borderRadius: '0.375rem',
        })
        this.root.append(canvas, this.fadeLayer)
        document.body.append(this.root)
        this.layoutFinale()
        window.addEventListener('wheel', this.blockInput, { passive: false })
        window.addEventListener('touchmove', this.blockInput, { passive: false })
        window.addEventListener('keydown', this.blockKeys)
        window.addEventListener('pageshow', this.onPageShow)
    }

    private descent(partial: number) {
        const progress = clamp((this.tickCount - this.summonedAt + partial) / DESCENT_TICKS, 0, 1)
        return progress * progress * (3 - 2 * progress)
    }

    /**
     * The frame grows from where it sat in the viewer to the whole viewport,
     * its resolution going from the capture's to the screen's.
     */
    private layoutFinale() {
        const canvas = this.canvas
        const renderer = this.renderer
        if (!canvas || !renderer) return
        const t = this.descent(this.accumulator / TICK_MS)
        const width = document.documentElement.clientWidth || window.innerWidth
        const height = document.documentElement.clientHeight || window.innerHeight
        const from = this.frameRect
        const left = lerp(t, from.left, 0)
        const top = lerp(t, from.top, 0)
        const w = Math.max(1, lerp(t, from.width, width))
        const h = Math.max(1, lerp(t, from.height, height))
        Object.assign(canvas.style, {
            left: `${left}px`,
            top: `${top}px`,
            width: `${w}px`,
            height: `${h}px`,
            borderRadius: `${(1 - t) * 0.375}rem`,
        })
        const captureDensity = (this.world.meta.screenshot[0] * this.captureScale) / from.width
        const screenDensity = Math.min(window.devicePixelRatio || 1, this.slow ? 1 : 1.5)
        const density = lerp(t, captureDensity, screenDensity)
        const bufferWidth = Math.max(1, Math.round(w * density))
        const bufferHeight = Math.max(1, Math.round(h * density))
        if (canvas.width !== bufferWidth || canvas.height !== bufferHeight)
            renderer.setSize(bufferWidth, bufferHeight, false)
        this.camera.aspect = w / h
        if (t >= 1 && this.summoning) {
            this.summoning = false
            this.answerDelete(true)
        }
    }

    private wardenPosition() {
        return new THREE.Vector3(this.warden?.x ?? 0, this.warden?.y ?? 0, this.warden?.z ?? 0)
    }

    /** The highest floor under the warden's 0.9-wide box. */
    private wardenFloor(x: number, z: number) {
        let top = -Infinity
        for (const dx of [-WARDEN_HALF_WIDTH, WARDEN_HALF_WIDTH])
            for (const dz of [-WARDEN_HALF_WIDTH, WARDEN_HALF_WIDTH])
                top = Math.max(top, this.world.floorAt(x + dx, z + dz))
        return top
    }

    private tickWarden() {
        const warden = this.warden
        const player = this.player
        if (!warden || !player) return
        const position = () => new THREE.Vector3(warden.x, warden.y, warden.z)
        warden.tickCount++
        warden.xo = warden.x
        warden.yo = warden.y
        warden.zo = warden.z
        warden.bodyRotO = warden.bodyRot
        warden.headRotO = warden.headRot
        warden.xRotO = warden.xRot

        // `Mob.baseTick`: an ambient sound now and then, none while roaring or emerging.
        if (random() * 1000 < warden.ambientSoundTime++) {
            warden.ambientSoundTime = -80
            if (warden.pose === 'standing' && warden.angerLevel !== 'calm') {
                this.sounds?.play(
                    warden.angerLevel === 'angry'
                        ? 'entity.warden.angry'
                        : 'entity.warden.agitated',
                    position(),
                    4,
                    voicePitch(),
                )
            }
        }

        // `Warden.tick` on the client: heartbeat, tendrils and digging particles.
        if (warden.tickCount % 20 === 0) warden.clientAnger = warden.anger
        const heartBeatDelay = 40 - Math.floor(clamp(warden.clientAnger / ANGRY, 0, 1) * 30)
        if (warden.tickCount % heartBeatDelay === 0) {
            warden.heart = 10
            this.sounds?.play('entity.warden.heartbeat', position(), 5, voicePitch())
        }
        warden.tendrilO = warden.tendril
        if (warden.tendril > 0) warden.tendril--
        warden.heartO = warden.heart
        if (warden.heart > 0) warden.heart--
        if (
            warden.pose === 'emerging' &&
            (warden.tickCount - (warden.emergeStart ?? 0)) * 50 < 4500
        ) {
            for (let index = 0; index < 30; index++) {
                if (random() < DROPPED_PARTICLES) continue
                this.terrainParticles.push(
                    terrainParticle(
                        warden.x + randomBetween(-0.7, 0.7),
                        warden.y,
                        warden.z + randomBetween(-0.7, 0.7),
                    ),
                )
            }
        }

        if ((warden.tickCount + 1) % DARKNESS_INTERVAL === 0)
            this.applyDarkness(position(), WARDEN_DARKNESS_RADIUS)

        // The brain: emerge, hear the player three times, roar, then fight.
        if (warden.pose === 'emerging' && warden.tickCount > EMERGE_DURATION) {
            warden.pose = 'standing'
            warden.nextVibration = warden.tickCount + 10
        }
        if (
            warden.pose === 'standing' &&
            !warden.fighting &&
            warden.tickCount >= warden.nextVibration &&
            warden.angerLevel !== 'angry'
        ) {
            this.hearVibration(warden)
        } else if (
            warden.pose === 'standing' &&
            !warden.fighting &&
            warden.angerLevel === 'angry'
        ) {
            warden.pose = 'roaring'
            warden.roarStart = warden.tickCount
            warden.roarSoundAt = warden.tickCount + ROAR_SOUND_DELAY
            warden.roarEndsAt = warden.tickCount + ROAR_DURATION
            warden.lookingAtPlayer = true
            warden.anger += 20
        }
        if (warden.pose === 'roaring') {
            if (warden.tickCount === warden.roarSoundAt) {
                this.sounds?.play('entity.warden.roar', position(), 3, 1)
            }
            if (warden.tickCount >= warden.roarEndsAt) {
                warden.pose = 'standing'
                warden.fighting = true
            }
        }

        let moving = false
        if (warden.fighting) moving = this.fight(warden, player)
        this.turn(warden, player, moving)
        warden.y = this.wardenFloor(warden.x, warden.z)

        // `LivingEntity.calculateEntityAnimation` from the distance walked.
        const walked = Math.hypot(warden.x - warden.xo, warden.z - warden.zo)
        warden.walkSpeedO = warden.walkSpeed
        warden.walkSpeed += (Math.min(walked * 4, 1) - warden.walkSpeed) * 0.4
        warden.walkPosition += warden.walkSpeed
        warden.moveDist += walked * 0.6
        if (warden.moveDist > warden.nextStep) {
            warden.nextStep = Math.trunc(warden.moveDist) + 1
            this.sounds?.play('entity.warden.step', position(), 10, 1)
        }
    }

    /** `VibrationUser.onReceiveVibration` for the player's steps on the city floor. */
    private hearVibration(warden: Warden) {
        warden.nextVibration = warden.tickCount + VIBRATION_COOLDOWN
        warden.tendril = 10
        const at = new THREE.Vector3(warden.x, warden.y, warden.z)
        this.sounds?.play('entity.warden.tendril_clicks', at, 5, voicePitch())
        warden.anger += 35
        warden.lookingAtPlayer = true
        this.sounds?.play(
            warden.angerLevel === 'calm'
                ? 'entity.warden.listening'
                : 'entity.warden.listening_angry',
            at,
            10,
            voicePitch(),
        )
    }

    /**
     * The fight activity: walk at 1.2× speed until the attack box reaches the
     * player, then `doHurtTarget` every 18 ticks. Returns whether it walked.
     */
    private fight(warden: Warden, player: Player) {
        if (warden.attackCooldown > 0) warden.attackCooldown--
        const reach = WARDEN_HALF_WIDTH + ATTACK_REACH + PLAYER_HALF_WIDTH
        const dx = player.x - warden.x
        const dz = player.z - warden.z
        if (Math.abs(dx) < reach && Math.abs(dz) < reach) {
            if (warden.attackCooldown <= 0 && !this.departed) this.attack(warden, player)
            warden.xd *= 0.546
            warden.zd *= 0.546
            warden.x += warden.xd
            warden.z += warden.zd
            return false
        }
        // `MoveControl` turns up to 90° a tick, then `travel` on the ground.
        const yaw = rotateTowards(warden.bodyRot, yawTowards(dx, dz), 90)
        warden.bodyRot = yaw
        const radians = (yaw * Math.PI) / 180
        const acceleration = FIGHT_SPEED * FIGHT_SPEED
        warden.xd += -Math.sin(radians) * acceleration
        warden.zd += Math.cos(radians) * acceleration
        warden.x += warden.xd
        warden.z += warden.zd
        warden.xd *= 0.546
        warden.zd *= 0.546
        return true
    }

    /**
     * `Warden.doHurtTarget`: the attack animation, the impact and the
     * knockback. The first punch throws the player back; the warden follows,
     * and as the second one lands the view goes dark and leaves the page.
     */
    private attack(warden: Warden, player: Player) {
        warden.roarStart = null
        warden.attackStart = warden.tickCount
        warden.attackCooldown = MELEE_ATTACK_COOLDOWN
        warden.hits++
        this.sounds?.play(
            'entity.warden.attack_impact',
            new THREE.Vector3(warden.x, warden.y, warden.z),
            10,
            voicePitch(),
        )
        this.sounds?.play('entity.player.hurt', this.eyePosition(), 1, voicePitch())
        player.hurtTime = 10
        const dx = warden.x - player.x
        const dz = warden.z - player.z
        player.hurtDir = (Math.atan2(dz, dx) * 180) / Math.PI - this.playerYaw()
        // `LivingEntity.knockback(1.5 × 0.5, sin(yaw), -cos(yaw))` from the warden's facing.
        const radians = (warden.bodyRot * Math.PI) / 180
        const power = 1.5 * 0.5
        player.xd = player.xd / 2 - Math.sin(radians) * power
        player.zd = player.zd / 2 + Math.cos(radians) * power
        if (player.onGround) player.yd = Math.min(0.4, player.yd / 2 + power)
        player.onGround = false
        warden.xd *= 0.6
        warden.zd *= 0.6
        if (warden.hits >= 2) this.depart()
    }

    /** `LookControl` and `BodyRotationControl` for a warden looking at the player. */
    private turn(warden: Warden, player: Player, moving: boolean) {
        const eye = new THREE.Vector3(player.x, player.y + EYE_HEIGHT, player.z)
        if (warden.lookingAtPlayer && warden.pose !== 'emerging') {
            const dx = eye.x - warden.x
            const dz = eye.z - warden.z
            warden.headRot = rotateTowards(warden.headRot, yawTowards(dx, dz), 10)
            warden.xRot = rotateTowards(
                warden.xRot,
                pitchTowards(eye.y - (warden.y + WARDEN_EYE_HEIGHT), Math.hypot(dx, dz)),
                40,
            )
        } else {
            warden.headRot = rotateTowards(warden.headRot, warden.bodyRot, 10)
            warden.xRot = 0
        }
        if (moving) {
            warden.headRot = rotateIfNecessary(warden.headRot, warden.bodyRot, 75)
            warden.lastStableHeadRot = warden.headRot
            warden.headStableTime = 0
        } else if (Math.abs(warden.headRot - warden.lastStableHeadRot) > 15) {
            warden.headStableTime = 0
            warden.lastStableHeadRot = warden.headRot
            warden.bodyRot = rotateIfNecessary(warden.bodyRot, warden.headRot, 75)
        } else {
            warden.headStableTime++
            if (warden.headStableTime > 10) {
                const progress = clamp((warden.headStableTime - 10) / 10, 0, 1)
                warden.bodyRot = rotateIfNecessary(
                    warden.bodyRot,
                    warden.headRot,
                    75 * (1 - progress),
                )
            }
        }
    }

    private playerYaw() {
        const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(this.camera.quaternion)
        return (Math.atan2(-forward.x, forward.z) * 180) / Math.PI
    }

    /**
     * Fades out, then lands on the page the punch sends the viewer to: the page
     * the demo is framed in goes there, not just the frame.
     */
    private depart() {
        if (this.departed) return
        this.departed = true
        const fadeTicks = 8
        for (let tick = 1; tick <= fadeTicks; tick++) {
            this.schedule(tick, () => {
                this.fadeLayer.style.opacity = String(tick / fadeTicks)
            })
        }
        this.schedule(fadeTicks + 2, () => {
            try {
                const page = window.top ?? window
                page.location.href = PUNCH_DESTINATION
            } catch {
                window.location.href = PUNCH_DESTINATION
            }
        })
    }

    /** Undoes the encounter, for a page restored from the back/forward cache. */
    private reset() {
        this.warden = null
        this.player = null
        this.departed = false
        this.summoning = false
        this.answer = null
        this.clearWorld()
        this.whenLive.length = 0
        this.fadeLayer.style.opacity = '0'
        if (this.model) this.model.root.visible = false
        window.removeEventListener('wheel', this.blockInput)
        window.removeEventListener('touchmove', this.blockInput)
        window.removeEventListener('keydown', this.blockKeys)
        window.removeEventListener('pageshow', this.onPageShow)
        this.stopLoop()
        this.detachCanvas()
        this.root.remove()
        this.stage = 'idle'
        this.host = null
        this.mountToken++
    }

    /**
     * The lightmap and fog of the frame (`LightTexture.updateLightTexture`,
     * `FogRenderer` with `DarknessFogEnvironment`): Night Vision lifts every
     * light level to full, then the Darkness pulse takes up to 45 % of it
     * back and its fog closes in; the fog stays black, as Night Vision does
     * not brighten it while Darkness is on.
     */
    private updateLight(partial: number) {
        const blend = this.blendFactor(partial)
        const darknessGamma = blend * DARKNESS_PULSING
        const light = this.world.light
        light.darknessScale = Math.max(
            0,
            Math.cos((this.tickCount - partial) * Math.PI * 0.025) * 0.45 * darknessGamma,
        )
        light.brightness = Math.max(0, GAMMA - darknessGamma)
        this.world.updateLightmap()

        const span = clamp(RENDER_DISTANCE / 10, 4, 64)
        let start = 1e6
        let end = 1e6
        if (this.darkness) {
            const distance = lerp(blend, RENDER_DISTANCE, 15)
            start = distance * 0.75
            end = distance
        }
        this.world.setFog(start, end, RENDER_DISTANCE - span, RENDER_DISTANCE)
        const shared = this.shared
        shared.fogEnvironmentalStart.value = start
        shared.fogEnvironmentalEnd.value = end
        shared.fogRenderDistanceStart.value = RENDER_DISTANCE - span
        shared.fogRenderDistanceEnd.value = RENDER_DISTANCE
        // The fog colour is darkened by the Darkness blend, black by the time its fog reaches anything.
        shared.fogColor.value.set(0, 0, 0, 1)
    }

    /** The capture's eyes, then down to the player's, tilted by `bobHurt`. */
    private updateCamera(partial: number) {
        const camera = this.camera
        const player = this.player
        const warden = this.warden
        if (!player || !warden || !this.model) {
            camera.position.copy(this.capturePose.position)
            camera.quaternion.copy(this.capturePose.quaternion)
            camera.fov = this.capturePose.fov
        } else {
            const eye = new THREE.Vector3(
                lerp(partial, player.xo, player.x),
                lerp(partial, player.yo, player.y) + EYE_HEIGHT,
                lerp(partial, player.zo, player.z),
            )
            const head = new THREE.Vector3()
            this.model.headPosition(head)
            const target = new THREE.Vector3(
                head.x,
                Math.max(warden.y + 0.6, head.y + 0.35),
                head.z,
            )
            // eases 18 % of the way per frame at 60 frames a second, whatever the rate
            this.lookPoint.lerp(target, 1 - 0.82 ** (this.frameElapsed / (1000 / 60)))
            const eyeRotation = new THREE.Quaternion().setFromRotationMatrix(
                new THREE.Matrix4().lookAt(eye, this.lookPoint, new THREE.Vector3(0, 1, 0)),
            )
            const eased = this.descent(partial)
            camera.position.lerpVectors(this.capturePose.position, eye, eased)
            camera.quaternion.slerpQuaternions(this.capturePose.quaternion, eyeRotation, eased)
            camera.fov = lerp(eased, this.capturePose.fov, FOV)

            if (player.hurtTime > 0) {
                const hurt = Math.sin((Math.max(0, player.hurtTime - partial) / 10) ** 4 * Math.PI)
                const tilt = (-hurt * 14 * Math.PI) / 180
                const direction = (player.hurtDir * Math.PI) / 180
                const roll = new THREE.Quaternion()
                    .setFromAxisAngle(new THREE.Vector3(0, 1, 0), -direction)
                    .multiply(
                        new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), -tilt),
                    )
                    .multiply(
                        new THREE.Quaternion().setFromAxisAngle(
                            new THREE.Vector3(0, 1, 0),
                            direction,
                        ),
                    )
                camera.quaternion.multiply(roll)
            }
        }
        camera.updateProjectionMatrix()
        camera.updateMatrixWorld()
        this.world.setCameraPosition(camera.position)
    }

    private render(partial: number) {
        const renderer = this.renderer
        if (!renderer) return
        this.updateCamera(partial)
        this.updateLight(partial)
        if (this.sounds) {
            const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(this.camera.quaternion)
            const up = new THREE.Vector3(0, 1, 0).applyQuaternion(this.camera.quaternion)
            this.sounds.listen(this.camera.position, forward, up)
        }
        this.buildParticles(partial)
        this.buildWarden(partial)
        renderer.render(this.world.scene, this.camera)
    }

    private lightTint(x: number, y: number, z: number, shade: number, alpha: number) {
        const [block, sky] = this.world.lightAt(x, y, z)
        const color = this.world.lightColor(block, sky)
        return [color.x * shade, color.y * shade, color.z * shade, alpha] as [
            number,
            number,
            number,
            number,
        ]
    }

    private buildParticles(partial: number) {
        // `ShriekParticle.render`: two quads tilted 60° from upright, back to
        // back, at full block light (`getLightColor` is 240).
        const tilted = new THREE.Quaternion().setFromEuler(new THREE.Euler(-1.0472, 0, 0))
        const reverse = new THREE.Quaternion().setFromEuler(
            new THREE.Euler(1.0472, -Math.PI, 0, 'YXZ'),
        )
        const full = this.world.lightColor(15, 0)
        const center = new THREE.Vector3()
        this.shriekBatch.begin()
        for (const particle of this.shriekParticles) {
            if (particle.delay > 0) continue
            const age = (particle.age + partial) / 30
            const alpha = 1 - clamp(age, 0, 1)
            const size = 0.85 * clamp(age * 0.75, 0, 1)
            center.set(particle.x, lerp(partial, particle.yo, particle.y), particle.z)
            const tint: [number, number, number, number] = [full.x, full.y, full.z, alpha]
            this.shriekBatch.add(center, tilted, size, [0, 1, 0, 1], tint)
            this.shriekBatch.add(center, reverse, size, [0, 1, 0, 1], tint)
        }
        this.shriekBatch.end()

        // `VibrationSignalParticle.render`: two quads turned toward the target,
        // spinning slowly, at full block light; its sprite animates a frame a tick.
        const up = new THREE.Vector3(0, 1, 0)
        const side = new THREE.Vector3(1, 0, 0)
        const turn = (yaw: number, pitch: number, spin: number) =>
            new THREE.Quaternion()
                .setFromAxisAngle(up, yaw)
                .multiply(new THREE.Quaternion().setFromAxisAngle(side, pitch))
                .multiply(new THREE.Quaternion().setFromAxisAngle(up, spin))
        this.signalMaterial.uniforms.frame!.value = this.tickCount % VIBRATION_FRAMES
        this.signalMaterial.uniforms.frameNext!.value = this.tickCount % VIBRATION_FRAMES
        this.signalBatch.begin()
        for (const signal of this.signals) {
            const spin = Math.sin((signal.age + partial - Math.PI * 2) * 0.05) * 2
            const yaw = lerp(partial, signal.rotO, signal.rot)
            const pitch = lerp(partial, signal.pitchO, signal.pitch) + Math.PI / 2
            center.set(
                lerp(partial, signal.xo, signal.x),
                lerp(partial, signal.yo, signal.y),
                lerp(partial, signal.zo, signal.z),
            )
            const tint: [number, number, number, number] = [full.x, full.y, full.z, 1]
            this.signalBatch.add(center, turn(yaw, -pitch, spin), 0.3, [0, 1, 0, 1], tint)
            this.signalBatch.add(center, turn(yaw - Math.PI, pitch, spin), 0.3, [0, 1, 0, 1], tint)
        }
        this.signalBatch.end()

        // Sculk's texture animates through four frames, 20 ticks each, interpolated.
        const time = this.tickCount + partial
        const frame = Math.floor(time / 20) % 4
        this.terrainMaterial.uniforms.frame!.value = frame
        this.terrainMaterial.uniforms.frameNext!.value = (frame + 1) % 4
        this.terrainMaterial.uniforms.frameMix!.value = (time % 20) / 20
        this.terrainBatch.begin()
        for (const particle of this.terrainParticles) {
            center.set(
                lerp(partial, particle.xo, particle.x),
                lerp(partial, particle.yo, particle.y),
                lerp(partial, particle.zo, particle.z),
            )
            this.terrainBatch.add(
                center,
                this.camera.quaternion,
                particle.size,
                [(particle.uo + 1) / 4, particle.uo / 4, particle.vo / 4, (particle.vo + 1) / 4],
                this.lightTint(center.x, center.y, center.z, 0.6, 1),
            )
        }
        this.terrainBatch.end()
    }

    private buildWarden(partial: number) {
        const warden = this.warden
        const model = this.model
        this.shadowBatch.begin()
        if (!warden || !model) {
            this.shadowBatch.end()
            if (model) model.root.visible = false
            return
        }
        model.root.visible = true
        const x = lerp(partial, warden.xo, warden.x)
        const y = lerp(partial, warden.yo, warden.y)
        const z = lerp(partial, warden.zo, warden.z)
        const bodyRot = rotLerp(partial, warden.bodyRotO, warden.bodyRot)
        const headRot = rotLerp(partial, warden.headRotO, warden.headRot)
        model.setupAnim({
            ageInTicks: warden.tickCount + partial,
            bodyRot,
            yRot: wrapDegrees(headRot - bodyRot),
            xRot: lerp(partial, warden.xRotO, warden.xRot),
            walkAnimationPos: warden.walkPosition - warden.walkSpeed * (1 - partial),
            walkAnimationSpeed: Math.min(lerp(partial, warden.walkSpeedO, warden.walkSpeed), 1),
            tendrilAnimation: lerp(partial, warden.tendrilO, warden.tendril) / 10,
            heartAnimation: lerp(partial, warden.heartO, warden.heart) / 10,
            attackStart: warden.attackStart,
            emergeStart: warden.emergeStart,
            roarStart: warden.roarStart,
            sniffStart: null,
        })
        model.place(x, y, z, bodyRot)
        // `EntityRenderer.getPackedLightCoords`: the light at the warden's eyes.
        const [block, sky] = this.world.lightAt(x, y + WARDEN_EYE_HEIGHT, z)
        this.world.lightColor(block, sky, this.shared.lightmapColor.value)

        // `EntityRenderer.extractShadow`: radius 0.9, fading with the camera's
        // distance and with the light on the floor it falls on.
        const distanceSq = this.camera.position.distanceToSquared(new THREE.Vector3(x, y, z))
        const power = 1 - distanceSq / 256
        if (power > 0) {
            const [floorBlock, floorSky] = this.world.lightAt(x, y + 0.5, z)
            const brightness = lightBrightness(Math.max(floorBlock, floorSky))
            const ground = new THREE.Quaternion().setFromEuler(new THREE.Euler(-Math.PI / 2, 0, 0))
            this.shadowBatch.add(
                new THREE.Vector3(x, y + 0.015625, z),
                ground,
                0.9,
                [0, 1, 0, 1],
                [1, 1, 1, clamp(power * 0.5 * brightness, 0, 1)],
            )
        }
        this.shadowBatch.end()
    }
}

export async function createWardenEngine() {
    const names = [...WARDEN_TEXTURES, 'shriek', 'sculk', 'shadow', 'vibration']
    const [world, images] = await Promise.all([
        CaptureWorld.load(),
        Promise.all(names.map((name) => loadImage(name))),
    ])
    return new WardenEngine(world, Object.fromEntries(names.map((name, i) => [name, images[i]!])))
}
