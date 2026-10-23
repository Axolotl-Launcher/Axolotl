/**
 * The About hero scene of the desktop app (apps/app-frontend/src/components/ui/
 * AboutScene.vue) drawn with plain WebGL. The app builds it with three.js, which
 * the website does not ship, so this module reproduces what three.js does for
 * that scene: the same camera, lights, transforms, animation and draw order,
 * the app's own GLSL for the water and the rising rings (verbatim), and
 * MeshStandardMaterial's lighting for the axolotl model (metalness 0,
 * roughness 1, an ambient light and one directional light, sRGB texture in,
 * sRGB colour out). The model is the app's own Blockbench glTF export.
 */

type Vec3 = [number, number, number]
type Quat = [number, number, number, number]
type Mat4 = Float32Array

/* ---------------------------------------------------------------- GLSL (app) */

const WATER_VERTEX_SHADER = `#define WATER_VERT
varying vec2 vUv;
void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`

const WATER_FRAGMENT_SHADER = `#define WATER_FRAG
uniform float time;
uniform float seed;
uniform vec3 color;
varying vec2 vUv;

vec2 randomGradient(vec2 p) {
    float n = sin(dot(p, vec2(127.1, 311.7)));
    float angle = fract(n * 43758.5453123) * 6.28318530718 * seed;
    return vec2(cos(angle), sin(angle));
}

float perlinNoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);

    vec2 u = f * f * (3.0 - 2.0 * f);

    vec2 g1 = randomGradient(i);
    vec2 g2 = randomGradient(i + vec2(1.0, 0.0));
    vec2 g3 = randomGradient(i + vec2(0.0, 1.0));
    vec2 g4 = randomGradient(i + vec2(1.0, 1.0));

    vec2 d1 = f;
    vec2 d2 = f - vec2(1.0, 0.0);
    vec2 d3 = f - vec2(0.0, 1.0);
    vec2 d4 = f - vec2(1.0, 1.0);

    float v1 = dot(g1, d1);
    float v2 = dot(g2, d2);
    float v3 = dot(g3, d3);
    float v4 = dot(g4, d4);

    return mix(mix(v1, v2, u.x), mix(v3, v4, u.x), u.y);
}

void main() {
    float height = 0.0;
    height += perlinNoise(vec2(vUv.x * 10.0, time * 0.8)) * 0.3;
    height += perlinNoise(vec2(vUv.x * 5.0, time * 0.4)) * 0.35;
    height += perlinNoise(vec2(vUv.x * 2.5, time * 0.2)) * 0.15;
    height += perlinNoise(vec2(vUv.x * 2.0, time * 0.2)) * 0.2;
    height = clamp(height, -1.0, 1.0);
    height = height * 0.8 + 0.6;

    float thickness = 0.008;
    if(vUv.y < height - thickness) {
        float scalar = 1.0 - height + vUv.y;
        scalar = scalar * scalar * scalar * 0.6;
        gl_FragColor = vec4(color, scalar);
    } else if(vUv.y > height + thickness) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 0.0);
    } else {
        gl_FragColor = vec4(color, 1.0);
    }
}`

const CIRCLE_VERTEX_SHADER = `#define CIRCLE_VERT
varying vec2 vUv;
void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`

const CIRCLE_FRAGMENT_SHADER = `#define CIRCLE_FRAG
varying vec2 vUv;
uniform vec3 color;
float remap(float v, float inMin, float inMax, float outMin, float outMax) {
  float t = (v - inMin) / (inMax - inMin);
  return outMin + (outMax - outMin) * t;
}
void main() {
    float dis = distance(vUv, vec2(0.5));
    float thickness = 0.05;

    gl_FragColor = vec4(0.0);
    if(dis <= 0.35 && dis >= 0.35 - thickness) {
        gl_FragColor = vec4(color, 0.8);
    } else {
        // emissive
        float scalar = 0.0;
        if(dis >= 0.35) {
            scalar = clamp(0.5 - dis, 0.0, 0.15);
            scalar = remap(scalar, 0.0, 0.15, 0.0, 1.0);
        } else {
            scalar = clamp(0.35 - dis, 0.0, 0.5);
            scalar = remap(scalar, 0.0, 0.35, 1.0, 0.0);
        }
        scalar = clamp(scalar * scalar * scalar, 0.0, 1.0);
        gl_FragColor = vec4(color, scalar);
    }
}`

/* ------------------------------------- MeshStandardMaterial, as three.js has it */

const STANDARD_VERTEX_SHADER = `
attribute vec3 normal;
uniform mat3 normalMatrix;
varying vec3 vNormal;
varying vec3 vViewPosition;
varying vec2 vUv;
void main() {
    vUv = uv;
    vNormal = normalize(normalMatrix * normal);
    vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
    vViewPosition = -mvPosition.xyz;
    gl_Position = projectionMatrix * mvPosition;
}`

const STANDARD_FRAGMENT_SHADER = `
uniform sampler2D map;
uniform float alphaTest;
uniform vec3 ambientLightColor;
uniform vec3 directionalLightColor;
uniform vec3 directionalLightDirection;
varying vec3 vNormal;
varying vec3 vViewPosition;
varying vec2 vUv;

#define RECIPROCAL_PI 0.3183098861837907
#define EPSILON 1e-6

vec3 sRGBTransferEOTF(vec3 value) {
    return mix(pow(value * 0.9478672986 + vec3(0.0521327014), vec3(2.4)), value * 0.0773993808, vec3(lessThanEqual(value, vec3(0.04045))));
}

vec3 sRGBTransferOETF(vec3 value) {
    return mix(pow(value, vec3(0.41666)) * 1.055 - vec3(0.055), value * 12.92, vec3(lessThanEqual(value, vec3(0.0031308))));
}

vec3 F_Schlick(const in vec3 f0, const in float f90, const in float dotVH) {
    float fresnel = exp2((-5.55473 * dotVH - 6.98316) * dotVH);
    return f0 * (1.0 - fresnel) + (f90 * fresnel);
}

float V_GGX_SmithCorrelated(const in float alpha, const in float dotNL, const in float dotNV) {
    float a2 = alpha * alpha;
    float gv = dotNL * sqrt(a2 + (1.0 - a2) * dotNV * dotNV);
    float gl = dotNV * sqrt(a2 + (1.0 - a2) * dotNL * dotNL);
    return 0.5 / max(gv + gl, EPSILON);
}

float D_GGX(const in float alpha, const in float dotNH) {
    float a2 = alpha * alpha;
    float denom = dotNH * dotNH * (a2 - 1.0) + 1.0;
    return RECIPROCAL_PI * a2 / (denom * denom);
}

void main() {
    vec4 diffuseColor = texture2D(map, vUv);
    diffuseColor.rgb = sRGBTransferEOTF(diffuseColor.rgb);
    if (diffuseColor.a < alphaTest) discard;

    // Double-sided: back faces light their flipped normal.
    vec3 normal = normalize(vNormal) * (gl_FrontFacing ? 1.0 : -1.0);
    vec3 viewDir = normalize(vViewPosition);

    // metalness 0, roughness 1, IOR 1.5.
    vec3 specularColor = vec3(0.04);
    float alpha = 1.0;

    vec3 lightDir = directionalLightDirection;
    float dotNL = clamp(dot(normal, lightDir), 0.0, 1.0);
    vec3 irradiance = dotNL * directionalLightColor;

    vec3 halfDir = normalize(lightDir + viewDir);
    float dotNV = clamp(dot(normal, viewDir), 0.0, 1.0);
    float dotNH = clamp(dot(normal, halfDir), 0.0, 1.0);
    float dotVH = clamp(dot(viewDir, halfDir), 0.0, 1.0);
    vec3 F = F_Schlick(specularColor, 1.0, dotVH);
    float V = V_GGX_SmithCorrelated(alpha, dotNL, dotNV);
    float D = D_GGX(alpha, dotNH);

    vec3 directDiffuse = irradiance * RECIPROCAL_PI * diffuseColor.rgb;
    vec3 directSpecular = irradiance * F * (V * D);
    vec3 indirectDiffuse = ambientLightColor * RECIPROCAL_PI * diffuseColor.rgb;

    vec3 outgoingLight = directDiffuse + indirectDiffuse + directSpecular;
    gl_FragColor = vec4(sRGBTransferOETF(outgoingLight), diffuseColor.a);
}`

/* -------------------------------------------------------------- tiny math */

function mat4Identity(): Mat4 {
    const out = new Float32Array(16)
    out[0] = out[5] = out[10] = out[15] = 1
    return out
}

function mat4Multiply(a: Mat4, b: Mat4, out: Mat4 = new Float32Array(16)): Mat4 {
    for (let col = 0; col < 4; col++) {
        for (let row = 0; row < 4; row++) {
            let sum = 0
            for (let k = 0; k < 4; k++) sum += a[k * 4 + row] * b[col * 4 + k]
            out[col * 4 + row] = sum
        }
    }
    return out
}

/** `Matrix4.compose`. */
function mat4Compose(
    position: Vec3,
    quaternion: Quat,
    scale: Vec3,
    out: Mat4 = new Float32Array(16),
) {
    const [x, y, z, w] = quaternion
    const x2 = x + x
    const y2 = y + y
    const z2 = z + z
    const xx = x * x2
    const xy = x * y2
    const xz = x * z2
    const yy = y * y2
    const yz = y * z2
    const zz = z * z2
    const wx = w * x2
    const wy = w * y2
    const wz = w * z2
    const [sx, sy, sz] = scale
    out[0] = (1 - (yy + zz)) * sx
    out[1] = (xy + wz) * sx
    out[2] = (xz - wy) * sx
    out[3] = 0
    out[4] = (xy - wz) * sy
    out[5] = (1 - (xx + zz)) * sy
    out[6] = (yz + wx) * sy
    out[7] = 0
    out[8] = (xz + wy) * sz
    out[9] = (yz - wx) * sz
    out[10] = (1 - (xx + yy)) * sz
    out[11] = 0
    out[12] = position[0]
    out[13] = position[1]
    out[14] = position[2]
    out[15] = 1
    return out
}

function normalize(v: Vec3): Vec3 {
    const length = Math.hypot(v[0], v[1], v[2]) || 1
    return [v[0] / length, v[1] / length, v[2] / length]
}

function cross(a: Vec3, b: Vec3): Vec3 {
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/** `Quaternion.slerpFlat`, as the mixer's quaternion interpolant uses it. */
function slerp(a: Quat, b: Quat, t: number): Quat {
    let [x0, y0, z0, w0] = a
    const [x1, y1, z1, w1] = b
    if (t === 0) return [x0, y0, z0, w0]
    if (t === 1) return [x1, y1, z1, w1]
    if (w0 !== w1 || x0 !== x1 || y0 !== y1 || z0 !== z1) {
        let s = 1 - t
        const cos = x0 * x1 + y0 * y1 + z0 * z1 + w0 * w1
        const dir = cos >= 0 ? 1 : -1
        const sqrSin = 1 - cos * cos
        if (sqrSin > Number.EPSILON) {
            const sin = Math.sqrt(sqrSin)
            const len = Math.atan2(sin, cos * dir)
            s = Math.sin(s * len) / sin
            t = Math.sin(t * len) / sin
        }
        const tDir = t * dir
        x0 = x0 * s + x1 * tDir
        y0 = y0 * s + y1 * tDir
        z0 = z0 * s + z1 * tDir
        w0 = w0 * s + w1 * tDir
        if (s === 1 - t) {
            const f = 1 / Math.sqrt(x0 * x0 + y0 * y0 + z0 * z0 + w0 * w0)
            x0 *= f
            y0 *= f
            z0 *= f
            w0 *= f
        }
    }
    return [x0, y0, z0, w0]
}

/* ------------------------------------------------------------------ glTF */

interface GltfAccessor {
    bufferView: number
    byteOffset?: number
    componentType: number
    count: number
    type: 'SCALAR' | 'VEC2' | 'VEC3' | 'VEC4'
}

interface GltfDocument {
    scene?: number
    scenes: { nodes: number[] }[]
    nodes: {
        name?: string
        mesh?: number
        children?: number[]
        translation?: Vec3
        rotation?: Quat
        scale?: Vec3
    }[]
    meshes: {
        primitives: {
            attributes: { POSITION: number; NORMAL: number; TEXCOORD_0: number }
            indices: number
            material?: number
        }[]
    }[]
    materials?: { alphaMode?: string; alphaCutoff?: number }[]
    images: { uri: string }[]
    accessors: GltfAccessor[]
    bufferViews: { buffer: number; byteOffset?: number; byteLength: number; byteStride?: number }[]
    buffers: { uri: string }[]
    animations: {
        name: string
        channels: { sampler: number; target: { node: number; path: string } }[]
        samplers: { input: number; output: number; interpolation?: string }[]
    }[]
}

const COMPONENTS = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 } as const

function decodeDataUri(uri: string): Uint8Array {
    const base64 = uri.slice(uri.indexOf(',') + 1)
    const binary = atob(base64)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
    return bytes
}

function readAccessor(document: GltfDocument, buffers: Uint8Array[], index: number) {
    const accessor = document.accessors[index]
    const view = document.bufferViews[accessor.bufferView]
    const buffer = buffers[view.buffer]
    const components = COMPONENTS[accessor.type]
    const elementBytes = components * (accessor.componentType === 5126 ? 4 : 2)
    const stride = view.byteStride ?? elementBytes
    const start = buffer.byteOffset + (view.byteOffset ?? 0) + (accessor.byteOffset ?? 0)
    const data = new DataView(buffer.buffer)
    const out =
        accessor.componentType === 5126
            ? new Float32Array(accessor.count * components)
            : new Uint16Array(accessor.count * components)
    for (let i = 0; i < accessor.count; i++) {
        for (let c = 0; c < components; c++) {
            const offset = start + i * stride + c * (accessor.componentType === 5126 ? 4 : 2)
            out[i * components + c] =
                accessor.componentType === 5126
                    ? data.getFloat32(offset, true)
                    : data.getUint16(offset, true)
        }
    }
    return out
}

/* --------------------------------------------------------------- the scene */

export interface AboutSceneOptions {
    canvas: HTMLCanvasElement
    modelUrl: string
    /** The accent colour as a CSS colour (`--color-brand`). */
    accentColor: string
    darkMode: boolean
    /** Converts a pointer position to the launcher window's 1280 × 800 space. */
    toWindow: (clientX: number, clientY: number) => { x: number; y: number }
    windowSize: { width: number; height: number }
    /** Element whose pointer moves steer the camera (the app listens on its window). */
    pointerTarget: HTMLElement
}

/** `new THREE.Color(cssColor)`: parsed as sRGB and stored as linear sRGB. */
function linearColor(cssColor: string): Vec3 {
    const context = document.createElement('canvas').getContext('2d')
    let r = 0x44
    let g = 0x44
    let b = 0xff
    if (context) {
        context.fillStyle = '#4444ff'
        context.fillStyle = cssColor
        const parsed = context.fillStyle
        const hex = /^#([0-9a-f]{6})$/i.exec(parsed)
        const rgb = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/i.exec(parsed)
        if (hex) {
            const value = Number.parseInt(hex[1], 16)
            r = (value >> 16) & 255
            g = (value >> 8) & 255
            b = value & 255
        } else if (rgb) {
            r = Number(rgb[1])
            g = Number(rgb[2])
            b = Number(rgb[3])
        }
    }
    const toLinear = (channel: number) => {
        const c = channel / 255
        return c < 0.04045 ? c * 0.0773993808 : Math.pow(c * 0.9478672986 + 0.0521327014, 2.4)
    }
    return [toLinear(r), toLinear(g), toLinear(b)]
}

function scaleVec(v: Vec3, k: number): Vec3 {
    return [v[0] * k, v[1] * k, v[2] * k]
}

/**
 * Builds the scene on `canvas` and starts its animation loop. Throws when
 * WebGL is unavailable, as the app's renderer constructor does. Returns the
 * cleanup that stops the loop and releases every GL resource.
 */
export function startAboutScene(options: AboutSceneOptions): () => void {
    const { canvas } = options
    const contextAttributes: WebGLContextAttributes = {
        alpha: true,
        antialias: true,
        depth: true,
        stencil: false,
        premultipliedAlpha: true,
        preserveDrawingBuffer: false,
    }
    const gl = (canvas.getContext('webgl2', contextAttributes) ??
        canvas.getContext('webgl', contextAttributes)) as WebGLRenderingContext | null
    if (!gl) throw new Error('Error creating WebGL context.')

    let disposed = false
    let animationFrame: number | undefined
    const buffers: WebGLBuffer[] = []
    const textures: WebGLTexture[] = []
    const programs: WebGLProgram[] = []

    const highp =
        (gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.HIGH_FLOAT)?.precision ?? 0) > 0
    const precision = `precision ${highp ? 'highp' : 'mediump'} float;\n`

    function compile(type: number, source: string) {
        const shader = gl!.createShader(type)
        if (!shader) throw new Error('Could not create a WebGL shader.')
        gl!.shaderSource(shader, source)
        gl!.compileShader(shader)
        if (!gl!.getShaderParameter(shader, gl!.COMPILE_STATUS)) {
            const log = gl!.getShaderInfoLog(shader)
            gl!.deleteShader(shader)
            throw new Error(`About scene shader failed to compile: ${log}`)
        }
        return shader
    }

    /** A ShaderMaterial-like program: three.js prefixes the matrices and attributes. */
    function createProgram(vertexSource: string, fragmentSource: string) {
        const vertexPrefix = `${precision}uniform mat4 modelViewMatrix;\nuniform mat4 projectionMatrix;\nattribute vec3 position;\nattribute vec2 uv;\n`
        const vertex = compile(gl!.VERTEX_SHADER, vertexPrefix + vertexSource)
        const fragment = compile(gl!.FRAGMENT_SHADER, precision + fragmentSource)
        const program = gl!.createProgram()
        if (!program) throw new Error('Could not create a WebGL program.')
        gl!.attachShader(program, vertex)
        gl!.attachShader(program, fragment)
        gl!.linkProgram(program)
        gl!.deleteShader(vertex)
        gl!.deleteShader(fragment)
        if (!gl!.getProgramParameter(program, gl!.LINK_STATUS)) {
            throw new Error(`About scene program failed to link: ${gl!.getProgramInfoLog(program)}`)
        }
        programs.push(program)
        const uniforms = new Map<string, WebGLUniformLocation | null>()
        return {
            program,
            uniform(name: string) {
                if (!uniforms.has(name)) uniforms.set(name, gl!.getUniformLocation(program, name))
                return uniforms.get(name) ?? null
            },
            attribute(name: string) {
                return gl!.getAttribLocation(program, name)
            },
        }
    }

    function createBuffer(target: number, data: ArrayBufferView) {
        const buffer = gl!.createBuffer()
        if (!buffer) throw new Error('Could not create a WebGL buffer.')
        gl!.bindBuffer(target, buffer)
        gl!.bufferData(target, data, gl!.STATIC_DRAW)
        buffers.push(buffer)
        return buffer
    }

    function bindAttribute(location: number, buffer: WebGLBuffer, size: number) {
        if (location < 0) return
        gl!.bindBuffer(gl!.ARRAY_BUFFER, buffer)
        gl!.enableVertexAttribArray(location)
        gl!.vertexAttribPointer(location, size, gl!.FLOAT, false, 0, 0)
    }

    /* ---- camera: PerspectiveCamera(30 * 0.7, aspect, 1, 3000) at (-10, 5, 30) looking at the origin */

    const fov = 30 * 0.7
    const near = 1
    const far = 3000
    const originCameraPosition: Vec3 = [-10, 5, 30]
    let cameraPosition: Vec3 = [...originCameraPosition]
    const zAxis = normalize(originCameraPosition)
    const xAxis = normalize(cross([0, 1, 0], zAxis))
    const yAxis = cross(zAxis, xAxis)
    const projectionMatrix = new Float32Array(16)
    const viewMatrix = new Float32Array(16)
    let aspect = Math.max(1, canvas.clientWidth) / Math.max(1, canvas.clientHeight)

    function updateProjection() {
        const top = near * Math.tan((Math.PI / 180) * 0.5 * fov)
        const height = 2 * top
        const width = aspect * height
        projectionMatrix.fill(0)
        projectionMatrix[0] = (2 * near) / width
        projectionMatrix[5] = (2 * near) / height
        projectionMatrix[10] = -(far + near) / (far - near)
        projectionMatrix[11] = -1
        projectionMatrix[14] = (-2 * far * near) / (far - near)
    }

    function updateView() {
        // The inverse of the camera's world matrix; its rotation never changes
        // after lookAt, the pointer only moves its position.
        const [px, py, pz] = cameraPosition
        viewMatrix.set([
            xAxis[0],
            yAxis[0],
            zAxis[0],
            0,
            xAxis[1],
            yAxis[1],
            zAxis[1],
            0,
            xAxis[2],
            yAxis[2],
            zAxis[2],
            0,
            -(xAxis[0] * px + xAxis[1] * py + xAxis[2] * pz),
            -(yAxis[0] * px + yAxis[1] * py + yAxis[2] * pz),
            -(zAxis[0] * px + zAxis[1] * py + zAxis[2] * pz),
            1,
        ])
    }

    updateProjection()
    updateView()

    /* ---- lights: AmbientLight(0xffffff) and DirectionalLight(0xffffff, 4) from (-30, 30, 28) */

    const ambientLightColor: Vec3 = [1, 1, 1]
    const directionalLightColor: Vec3 = [4, 4, 4]
    const lightWorldDirection = normalize([-30, 30, 28])
    // transformDirection(viewMatrix): the light direction in view space.
    const lightViewDirection = normalize([
        xAxis[0] * lightWorldDirection[0] +
            xAxis[1] * lightWorldDirection[1] +
            xAxis[2] * lightWorldDirection[2],
        yAxis[0] * lightWorldDirection[0] +
            yAxis[1] * lightWorldDirection[1] +
            yAxis[2] * lightWorldDirection[2],
        zAxis[0] * lightWorldDirection[0] +
            zAxis[1] * lightWorldDirection[1] +
            zAxis[2] * lightWorldDirection[2],
    ])

    const accent = linearColor(options.accentColor)

    /* ---- shared plane geometry (PlaneGeometry, one segment) */

    const planeUvs = new Float32Array([0, 1, 1, 1, 0, 0, 1, 0])
    const planeIndices = new Uint16Array([0, 2, 1, 2, 3, 1])
    function planeBuffers(width: number, height: number) {
        const w = width / 2
        const h = height / 2
        return {
            position: createBuffer(
                gl!.ARRAY_BUFFER,
                new Float32Array([-w, h, 0, w, h, 0, -w, -h, 0, w, -h, 0]),
            ),
            uv: createBuffer(gl!.ARRAY_BUFFER, planeUvs),
            index: createBuffer(gl!.ELEMENT_ARRAY_BUFFER, planeIndices),
        }
    }

    /* ---- water: three planes sharing one material */

    const waterProgram = createProgram(WATER_VERTEX_SHADER, WATER_FRAGMENT_SHADER)
    const waterPlane = planeBuffers(120, 16)
    const waterSeed = Math.random() * 83 + 17
    const waterColor = scaleVec(accent, options.darkMode ? 0.6 : 2.4)
    const waterPositions: Vec3[] = [
        [0, -6.5, 4],
        [2, -8, -10],
        [16, -8, -26],
    ]

    /* ---- rising rings */

    const circleProgram = createProgram(CIRCLE_VERTEX_SHADER, CIRCLE_FRAGMENT_SHADER)
    const circlePlane = planeBuffers(0.6, 0.6)
    const circleColor = scaleVec(accent, options.darkMode ? 1.2 : 3)
    let circles: Vec3[] = []
    let nextCircleCreateTime = 0

    /* ---- the axolotl */

    const standardProgram = createProgram(STANDARD_VERTEX_SHADER, STANDARD_FRAGMENT_SHADER)
    interface ModelPart {
        node: number
        position: WebGLBuffer
        normal: WebGLBuffer
        uv: WebGLBuffer
        index: WebGLBuffer
        count: number
    }
    interface Channel {
        node: number
        path: 'rotation' | 'translation'
        times: Float32Array
        values: Float32Array
    }
    let model:
        | {
              document: GltfDocument
              parts: ModelPart[]
              texture: WebGLTexture
              alphaTest: number
              channels: Channel[]
              duration: number
              translations: Vec3[]
              rotations: Quat[]
          }
        | undefined
    let animationTime = 0

    async function loadModel() {
        const response = await fetch(options.modelUrl)
        if (!response.ok) throw new Error(`Failed to load ${options.modelUrl}: ${response.status}`)
        const document = (await response.json()) as GltfDocument
        if (disposed) return
        const bufferData = document.buffers.map((buffer) => decodeDataUri(buffer.uri))

        const image = new Image()
        image.src = document.images[0].uri
        await image.decode()
        if (disposed) return

        const texture = gl!.createTexture()
        if (!texture) throw new Error('Could not create a WebGL texture.')
        textures.push(texture)
        gl!.bindTexture(gl!.TEXTURE_2D, texture)
        // glTF textures are not flipped; the sampler is nearest / clamp.
        gl!.pixelStorei(gl!.UNPACK_FLIP_Y_WEBGL, false)
        gl!.pixelStorei(gl!.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false)
        gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA, gl!.RGBA, gl!.UNSIGNED_BYTE, image)
        gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.NEAREST)
        gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.NEAREST)
        gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_S, gl!.CLAMP_TO_EDGE)
        gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_T, gl!.CLAMP_TO_EDGE)

        const parts: ModelPart[] = []
        document.nodes.forEach((node, nodeIndex) => {
            if (node.mesh === undefined) return
            for (const primitive of document.meshes[node.mesh].primitives) {
                const indices = readAccessor(document, bufferData, primitive.indices)
                parts.push({
                    node: nodeIndex,
                    position: createBuffer(
                        gl!.ARRAY_BUFFER,
                        readAccessor(document, bufferData, primitive.attributes.POSITION),
                    ),
                    normal: createBuffer(
                        gl!.ARRAY_BUFFER,
                        readAccessor(document, bufferData, primitive.attributes.NORMAL),
                    ),
                    uv: createBuffer(
                        gl!.ARRAY_BUFFER,
                        readAccessor(document, bufferData, primitive.attributes.TEXCOORD_0),
                    ),
                    index: createBuffer(gl!.ELEMENT_ARRAY_BUFFER, indices),
                    count: indices.length,
                })
            }
        })

        const swim = document.animations.find((animation) => animation.name === 'swim')
        if (!swim) console.error('Missing animation swim')
        const channels: Channel[] = (swim?.channels ?? [])
            .filter((channel) => ['rotation', 'translation'].includes(channel.target.path))
            .map((channel) => {
                const sampler = swim!.samplers[channel.sampler]
                return {
                    node: channel.target.node,
                    path: channel.target.path as Channel['path'],
                    times: readAccessor(document, bufferData, sampler.input) as Float32Array,
                    values: readAccessor(document, bufferData, sampler.output) as Float32Array,
                }
            })
        const duration = channels.reduce(
            (longest, channel) => Math.max(longest, channel.times[channel.times.length - 1] ?? 0),
            0,
        )
        const material = document.materials?.[0]
        model = {
            document,
            parts,
            texture,
            alphaTest: material?.alphaMode === 'MASK' ? (material.alphaCutoff ?? 0.5) : 0,
            channels,
            duration,
            translations: document.nodes.map(
                (node) => [...(node.translation ?? [0, 0, 0])] as Vec3,
            ),
            rotations: document.nodes.map((node) => [...(node.rotation ?? [0, 0, 0, 1])] as Quat),
        }
    }

    /** The mixer's LoopRepeat update: sample every channel at `time`. */
    function applyAnimation(time: number) {
        if (!model) return
        for (const channel of model.channels) {
            const { times, values } = channel
            const size = channel.path === 'rotation' ? 4 : 3
            let index = 0
            while (index < times.length - 1 && times[index + 1] <= time) index++
            const next = Math.min(index + 1, times.length - 1)
            const span = times[next] - times[index]
            const t = span > 0 ? Math.min(1, Math.max(0, (time - times[index]) / span)) : 0
            const from = Array.from(values.subarray(index * size, index * size + size))
            const to = Array.from(values.subarray(next * size, next * size + size))
            if (channel.path === 'rotation') {
                model.rotations[channel.node] = slerp(
                    from as Quat,
                    to as Quat,
                    time <= times[0] ? 0 : t,
                )
            } else {
                const k = time <= times[0] ? 0 : t
                model.translations[channel.node] = [
                    from[0] + (to[0] - from[0]) * k,
                    from[1] + (to[1] - from[1]) * k,
                    from[2] + (to[2] - from[2]) * k,
                ]
            }
        }
    }

    /* ---- frame */

    const startedAt = performance.now()
    let lastFrame = startedAt
    const modelViewMatrix = new Float32Array(16)
    const projScreen = new Float32Array(16)
    const normalMatrix = new Float32Array(9)

    function setNormalMatrix(modelView: Mat4) {
        // Uniform scale and rotations only: the upper 3 × 3, renormalized in the shader.
        normalMatrix.set([
            modelView[0],
            modelView[1],
            modelView[2],
            modelView[4],
            modelView[5],
            modelView[6],
            modelView[8],
            modelView[9],
            modelView[10],
        ])
    }

    function ndcDepth(world: Vec3) {
        const m = projScreen
        const x = world[0]
        const y = world[1]
        const z = world[2]
        const w = m[3] * x + m[7] * y + m[11] * z + m[15]
        return (m[2] * x + m[6] * y + m[10] * z + m[14]) / w
    }

    function drawModel(elapsedTime: number) {
        if (!model) return
        // axlModel: scale × 5, rotated about Y, bobbing on a sine.
        const rotationY = Math.sin(elapsedTime * 0.3) * 0.2 + (Math.PI * 100) / 180
        const root = mat4Compose(
            [0, -2.5 + Math.sin(elapsedTime), 0],
            [0, Math.sin(rotationY / 2), 0, Math.cos(rotationY / 2)],
            [5, 5, 5],
        )
        const local = model.document.nodes.map((_, index) =>
            mat4Compose(model!.translations[index], model!.rotations[index], [1, 1, 1]),
        )
        const world: (Mat4 | undefined)[] = []
        const visit = (index: number, parent: Mat4) => {
            world[index] = mat4Multiply(parent, local[index])
            for (const child of model!.document.nodes[index].children ?? [])
                visit(child, world[index]!)
        }
        for (const index of model.document.scenes[model.document.scene ?? 0].nodes)
            visit(index, root)

        const program = standardProgram
        gl!.useProgram(program.program)
        gl!.disable(gl!.BLEND)
        gl!.disable(gl!.CULL_FACE)
        gl!.depthMask(true)
        gl!.uniformMatrix4fv(program.uniform('projectionMatrix'), false, projectionMatrix)
        gl!.uniform3fv(program.uniform('ambientLightColor'), ambientLightColor)
        gl!.uniform3fv(program.uniform('directionalLightColor'), directionalLightColor)
        gl!.uniform3fv(program.uniform('directionalLightDirection'), lightViewDirection)
        gl!.uniform1f(program.uniform('alphaTest'), model.alphaTest)
        gl!.activeTexture(gl!.TEXTURE0)
        gl!.bindTexture(gl!.TEXTURE_2D, model.texture)
        gl!.uniform1i(program.uniform('map'), 0)
        for (const part of model.parts) {
            const worldMatrix = world[part.node]
            if (!worldMatrix) continue
            mat4Multiply(viewMatrix, worldMatrix, modelViewMatrix)
            setNormalMatrix(modelViewMatrix)
            gl!.uniformMatrix4fv(program.uniform('modelViewMatrix'), false, modelViewMatrix)
            gl!.uniformMatrix3fv(program.uniform('normalMatrix'), false, normalMatrix)
            bindAttribute(program.attribute('position'), part.position, 3)
            bindAttribute(program.attribute('normal'), part.normal, 3)
            bindAttribute(program.attribute('uv'), part.uv, 2)
            gl!.bindBuffer(gl!.ELEMENT_ARRAY_BUFFER, part.index)
            gl!.drawElements(gl!.TRIANGLES, part.count, gl!.UNSIGNED_SHORT, 0)
        }
    }

    function drawPlane(
        program: ReturnType<typeof createProgram>,
        plane: ReturnType<typeof planeBuffers>,
        position: Vec3,
    ) {
        const worldMatrix = mat4Identity()
        worldMatrix[12] = position[0]
        worldMatrix[13] = position[1]
        worldMatrix[14] = position[2]
        mat4Multiply(viewMatrix, worldMatrix, modelViewMatrix)
        gl!.uniformMatrix4fv(program.uniform('modelViewMatrix'), false, modelViewMatrix)
        bindAttribute(program.attribute('position'), plane.position, 3)
        bindAttribute(program.attribute('uv'), plane.uv, 2)
        gl!.bindBuffer(gl!.ELEMENT_ARRAY_BUFFER, plane.index)
        gl!.drawElements(gl!.TRIANGLES, 6, gl!.UNSIGNED_SHORT, 0)
    }

    function render(elapsedTime: number) {
        gl!.viewport(0, 0, canvas.width, canvas.height)
        gl!.clearColor(0, 0, 0, 0)
        gl!.clearDepth(1)
        gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT)
        gl!.enable(gl!.DEPTH_TEST)
        gl!.depthFunc(gl!.LEQUAL)
        mat4Multiply(projectionMatrix, viewMatrix, projScreen)

        // Opaque first: the axolotl (alpha-masked, so not transparent).
        drawModel(elapsedTime)

        // Transparent objects, farthest first, normal blending, writing depth.
        type Draw = { depth: number; draw: () => void }
        const transparent: Draw[] = [
            ...waterPositions.map((position) => ({
                depth: ndcDepth(position),
                draw: () => {
                    gl!.useProgram(waterProgram.program)
                    gl!.uniformMatrix4fv(
                        waterProgram.uniform('projectionMatrix'),
                        false,
                        projectionMatrix,
                    )
                    gl!.uniform1f(waterProgram.uniform('time'), elapsedTime)
                    gl!.uniform1f(waterProgram.uniform('seed'), waterSeed)
                    gl!.uniform3fv(waterProgram.uniform('color'), waterColor)
                    drawPlane(waterProgram, waterPlane, position)
                },
            })),
            ...circles.map((position) => ({
                depth: ndcDepth(position),
                draw: () => {
                    gl!.useProgram(circleProgram.program)
                    gl!.uniformMatrix4fv(
                        circleProgram.uniform('projectionMatrix'),
                        false,
                        projectionMatrix,
                    )
                    gl!.uniform3fv(circleProgram.uniform('color'), circleColor)
                    drawPlane(circleProgram, circlePlane, position)
                },
            })),
        ].sort((a, b) => b.depth - a.depth)

        gl!.enable(gl!.BLEND)
        gl!.blendEquation(gl!.FUNC_ADD)
        gl!.blendFuncSeparate(
            gl!.SRC_ALPHA,
            gl!.ONE_MINUS_SRC_ALPHA,
            gl!.ONE,
            gl!.ONE_MINUS_SRC_ALPHA,
        )
        gl!.enable(gl!.CULL_FACE)
        gl!.cullFace(gl!.BACK)
        gl!.frontFace(gl!.CCW)
        gl!.depthMask(true)
        for (const entry of transparent) entry.draw()
    }

    function updateCircles(deltaTime: number, elapsedTime: number) {
        circles = circles.filter((circle) => {
            circle[1] += deltaTime * 2.0
            return circle[1] < 32
        })
        if (elapsedTime >= nextCircleCreateTime) {
            nextCircleCreateTime = elapsedTime + Math.random() * 0.8
            circles.push([Math.random() * 64 - 32 - 12, -20, Math.random() * 6 + 1])
        }
    }

    function animate() {
        if (disposed) return
        animationFrame = requestAnimationFrame(animate)
        const now = performance.now()
        const deltaTime = (now - lastFrame) / 1000
        lastFrame = now
        const elapsedTime = (now - startedAt) / 1000
        if (model) {
            animationTime += deltaTime
            applyAnimation(model.duration > 0 ? animationTime % model.duration : 0)
        }
        updateCircles(deltaTime, elapsedTime)
        render(elapsedTime)
    }

    function updateSize() {
        if (disposed) return
        const width = canvas.clientWidth
        const height = canvas.clientHeight
        if (width > 0 && height > 0) {
            const ratio = Math.min(devicePixelRatio, 2)
            canvas.width = Math.floor(width * ratio)
            canvas.height = Math.floor(height * ratio)
            aspect = width / height
            updateProjection()
        }
    }

    function onMouseMove(event: MouseEvent) {
        const { width, height } = options.windowSize
        const point = options.toWindow(event.clientX, event.clientY)
        const mouseXOffsetRatio = ((point.x - width / 2) / width) * 2
        const mouseYOffsetRatio = ((point.y - height / 2) / height) * 2
        cameraPosition = [
            originCameraPosition[0] + mouseXOffsetRatio,
            originCameraPosition[1] + mouseYOffsetRatio * 0.5,
            originCameraPosition[2],
        ]
        updateView()
    }

    const resizeObserver = new ResizeObserver(updateSize)
    resizeObserver.observe(canvas)
    updateSize()
    options.pointerTarget.addEventListener('mousemove', onMouseMove)

    void loadModel().catch((error) => console.error('Failed to load About scene model', error))
    animate()

    return () => {
        disposed = true
        if (animationFrame !== undefined) cancelAnimationFrame(animationFrame)
        options.pointerTarget.removeEventListener('mousemove', onMouseMove)
        resizeObserver.disconnect()
        model = undefined
        circles = []
        for (const buffer of buffers) gl.deleteBuffer(buffer)
        for (const texture of textures) gl.deleteTexture(texture)
        for (const program of programs) gl.deleteProgram(program)
        gl.getExtension('WEBGL_lose_context')?.loseContext()
    }
}
