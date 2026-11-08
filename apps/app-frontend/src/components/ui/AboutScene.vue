<template>
    <canvas ref="canvas" class="about-scene-canvas size-full" />
</template>

<script setup lang="ts">
import * as THREE from 'three'
import { type GLTF, GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { onMounted, onScopeDispose, ref } from 'vue'

import { useTheming } from '@/store/theme'

const themeStore = useTheming()
const canvas = ref<HTMLCanvasElement>()
let cleanup: (() => void) | undefined
onScopeDispose(() => cleanup?.())
function isDarkMode() {
    if (themeStore.selectedTheme == 'system') {
        return matchMedia('(prefers-color-scheme: dark)').matches
    }
    return ['dark', 'oled'].includes(themeStore.selectedTheme)
}

function loadGLTF(url: string): Promise<GLTF> {
    return new Promise((res, rej) => {
        const loader = new GLTFLoader()
        loader.load(
            url,
            (data) => {
                res(data)
            },
            undefined,
            rej,
        )
    })
}

function createTip(position: THREE.Vector3, color: THREE.ColorRepresentation = 0x00ff00) {
    const tipGeometry = new THREE.SphereGeometry(2)
    const tipMaterial = new THREE.MeshBasicMaterial({ color })
    const tipMesh = new THREE.Mesh(tipGeometry, tipMaterial)
    tipMesh.position.copy(position)
    return tipMesh
}

function createWaterMaterial(): THREE.ShaderMaterial {
    return new THREE.ShaderMaterial({
        uniforms: {
            time: { value: 0 },
            seed: { value: Math.random() * 83 + 17 },
            color: { value: new THREE.Color(0.3, 0.3, 1.0) },
        },
        transparent: true,
        vertexShader: `#define WATER_VERT
varying vec2 vUv;
void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`,
        fragmentShader: `#define WATER_FRAG
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
}`,
    })
}

function createWater(material: THREE.ShaderMaterial, position: THREE.Vector3) {
    const geometry = new THREE.PlaneGeometry(120, 16)
    const waterMesh = new THREE.Mesh(geometry, material)
    waterMesh.position.copy(position)
    return waterMesh
}

function createCircleMaterial(): THREE.ShaderMaterial {
    return new THREE.ShaderMaterial({
        uniforms: {
            color: { value: new THREE.Color(0.3, 0.3, 1.0) },
        },
        transparent: true,
        vertexShader: `#define CIRCLE_VERT
varying vec2 vUv;
void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`,
        fragmentShader: `#define CIRCLE_FRAG
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
}`,
    })
}

function createCircle(material: THREE.ShaderMaterial, position: THREE.Vector3) {
    const geometry = new THREE.PlaneGeometry(0.6, 0.6)
    const mesh = new THREE.Mesh(geometry, material)
    mesh.position.copy(position)
    return mesh
}

function disposeObject(object: THREE.Object3D) {
    const geometries = new Set<THREE.BufferGeometry>()
    const materials = new Set<THREE.Material>()
    const textures = new Set<THREE.Texture>()
    const bitmaps = new Set<ImageBitmap>()
    object.traverse((child) => {
        if (child instanceof THREE.SkinnedMesh) child.skeleton.dispose()
        if (!('geometry' in child) || !('material' in child)) return
        const renderable = child as THREE.Mesh
        geometries.add(renderable.geometry)
        const meshMaterials = Array.isArray(renderable.material)
            ? renderable.material
            : [renderable.material]
        for (const material of meshMaterials) {
            materials.add(material)
            for (const value of Object.values(material)) {
                if (value instanceof THREE.Texture) {
                    textures.add(value)
                    if (typeof ImageBitmap !== 'undefined' && value.image instanceof ImageBitmap) {
                        bitmaps.add(value.image)
                    }
                }
            }
        }
    })
    for (const geometry of geometries) geometry.dispose()
    for (const material of materials) material.dispose()
    for (const texture of textures) texture.dispose()
    for (const bitmap of bitmaps) bitmap.close()
    object.clear()
}

function main() {
    const sceneCanvas = canvas.value
    if (!sceneCanvas) return console.error('About scene canvas is unavailable')

    let isUpdating = true
    let animationFrame: number | undefined
    let disposed = false

    const canvasSize = new THREE.Vector2(
        Math.max(1, sceneCanvas.clientWidth),
        Math.max(1, sceneCanvas.clientHeight),
    )

    const renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: true,
        canvas: sceneCanvas,
    })
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2))
    renderer.setSize(canvasSize.x, canvasSize.y, false)

    // three@0.185 deprecates Clock; use a tiny elapsed-time helper instead.
    const startedAt = performance.now()
    const deltaAt = { last: startedAt }

    function nextDelta() {
        const now = performance.now()
        const dt = (now - deltaAt.last) / 1000
        deltaAt.last = now
        return dt
    }

    function elapsedSeconds() {
        return (performance.now() - startedAt) / 1000
    }

    const scene = new THREE.Scene()

    const camera = new THREE.PerspectiveCamera(30, canvasSize.x / canvasSize.y, 1, 3000)
    camera.fov *= 0.7
    camera.updateProjectionMatrix()
    camera.position.set(-10, 5, 30)
    camera.lookAt(0, 0, 0)

    const ambientLight = new THREE.AmbientLight(0xffffff)
    scene.add(ambientLight)

    const dirLight = new THREE.DirectionalLight(0xffffff, 4.0)
    dirLight.position.set(-30, 30, 28)
    scene.add(dirLight)

    scene.add(createTip(dirLight.position, 0xffff00))
    scene.add(createTip(camera.position))

    const accentColor =
        getComputedStyle(document.documentElement).getPropertyValue('--color-brand').trim() ||
        '#4444ff'

    const waterMaterial = createWaterMaterial()
    waterMaterial.uniforms.color.value = new THREE.Color(accentColor).multiplyScalar(
        isDarkMode() ? 0.6 : 2.4,
    )
    // .multiplyScalar(0.6)
    // .multiplyScalar(2.4)

    scene.add(createWater(waterMaterial, new THREE.Vector3(0, -6.5, 4)))
    scene.add(createWater(waterMaterial, new THREE.Vector3(2, -8, -10)))
    scene.add(createWater(waterMaterial, new THREE.Vector3(16, -8, -26)))

    let mixer: THREE.AnimationMixer | undefined
    async function load() {
        const axlGLTF = await loadGLTF('/models/axolotl.gltf')
        if (disposed) {
            disposeObject(axlGLTF.scene)
            return
        }

        const axlModel = axlGLTF.scene
        axlModel.scale.multiplyScalar(5)
        axlModel.rotateY(Math.PI / 2)
        axlModel.position.add(new THREE.Vector3(0, -2.5, 0))
        scene.add(axlModel)

        const modelMixer = new THREE.AnimationMixer(axlModel)
        mixer = modelMixer
        const axlSwimAnim = axlGLTF.animations.filter((a) => a.name === 'swim')[0]
        if (!axlSwimAnim) return console.error('Missing animation swim')
        modelMixer.clipAction(axlSwimAnim).play()

        // // Axl Label
        // const axlLabelGLTF = await loadGLTF('/models/axl_label.glb')
        // const axlLabel = axlLabelGLTF.scene
        // axlLabel.scale.multiplyScalar(8)
        // axlLabel.rotateY(-Math.PI / 2)
        // axlLabel.position.set(0, 5.2, 0)
        // scene.add(axlLabel)

        const originAxlModelPosition = axlModel.position.clone()
        return function (deltaTime: number, elapsedTime: number) {
            axlModel.position.set(
                originAxlModelPosition.x,
                originAxlModelPosition.y + Math.sin(elapsedTime),
                originAxlModelPosition.z,
            )
            axlModel.rotation.y = Math.sin(elapsedTime * 0.3) * 0.2 + (Math.PI * 100) / 180
            modelMixer.update(deltaTime)
        }
    }
    let updateGLTF = (_deltaTime: number, _elapsedTime: number) => {}
    void load()
        .then((updateFn) => {
            if (!disposed && updateFn) updateGLTF = updateFn
        })
        .catch((error) => console.error('Failed to load About scene model', error))

    const circleMaterial = createCircleMaterial()
    circleMaterial.uniforms.color.value = new THREE.Color(accentColor).multiplyScalar(
        isDarkMode() ? 1.2 : 3,
    )
    // .multiplyScalar(1.2)
    // .multiplyScalar(3)

    let circleMeshList: THREE.Mesh[] = []
    let nextCircleCreateTime = 0.0
    function updateCircle(deltaTime: number, elapsedTime: number) {
        circleMeshList = circleMeshList.filter((m) => {
            m.position.y += deltaTime * 2.0
            if (m.position.y >= 32) {
                scene.remove(m)
                m.geometry.dispose()
                return false
            }
            return true
        })

        if (elapsedTime >= nextCircleCreateTime) {
            nextCircleCreateTime = elapsedTime + Math.random() * 0.8
            const circle = createCircle(
                circleMaterial,
                new THREE.Vector3(Math.random() * 64 - 32 - 12, -20, Math.random() * 6 + 1),
            )
            scene.add(circle)
            circleMeshList.push(circle)
        }
    }

    function animate(_time: number) {
        if (isUpdating === false) return
        animationFrame = requestAnimationFrame(animate)

        const deltaTime = nextDelta()
        const elapsedTime = elapsedSeconds()

        updateGLTF(deltaTime, elapsedTime)
        waterMaterial.uniforms.time.value = elapsedTime

        updateCircle(deltaTime, elapsedTime)

        renderer.render(scene, camera)
    }
    animate(Date.now())

    const originCameraPosition = camera.position.clone()
    function onMouseMove(event: MouseEvent) {
        const mouseXOffsetRatio = ((event.clientX - innerWidth / 2) / innerWidth) * 2
        const mouseYOffsetRatio = ((event.clientY - innerHeight / 2) / innerHeight) * 2
        const newPosition = new THREE.Vector3(
            originCameraPosition.x + mouseXOffsetRatio,
            originCameraPosition.y + mouseYOffsetRatio * 0.5,
            originCameraPosition.z,
        )
        camera.position.copy(newPosition)
    }

    function updateSize() {
        if (!isUpdating || !sceneCanvas) return
        const w = sceneCanvas.clientWidth
        const h = sceneCanvas.clientHeight
        if (w > 0 && h > 0) {
            renderer.setSize(w, h, false)
            camera.aspect = w / h
            camera.updateProjectionMatrix()
        }
    }

    const resizeObserver = new ResizeObserver(updateSize)
    resizeObserver.observe(sceneCanvas)
    updateSize()

    addEventListener('mousemove', onMouseMove)
    cleanup = () => {
        disposed = true
        isUpdating = false
        if (animationFrame !== undefined) cancelAnimationFrame(animationFrame)
        removeEventListener('mousemove', onMouseMove)
        resizeObserver.disconnect()
        mixer?.stopAllAction()
        if (mixer) mixer.uncacheRoot(mixer.getRoot())
        mixer = undefined
        updateGLTF = () => {}
        for (const circle of circleMeshList) {
            scene.remove(circle)
            circle.geometry.dispose()
        }
        circleMeshList = []
        circleMaterial.dispose()
        disposeObject(scene)
        renderer.renderLists.dispose()
        renderer.forceContextLoss()
        renderer.dispose()
    }
}

onMounted(main)
</script>
<style>
.about-scene-canvas {
    display: block;
    width: 100%;
    height: 100%;
    background: linear-gradient(
        to bottom,
        color-mix(in srgb, var(--color-brand) 36%, var(--surface-1) 100%),
        #00000000 40%
    );
}
</style>
