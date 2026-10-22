import { ClassicPlayerModel, SlimPlayerModel } from '@modrinth/assets'
import { loadSkinRendering } from '@modrinth/ui'
import * as THREE from 'three'

import type { Cape, Skin } from '../skins'
import { determineModelType, get_normalized_skin_texture } from '../skins'
import { headStorage } from '../storage/head-storage'
import { skinPreviewStorage } from '../storage/skin-preview-storage'
import {
    cleanupUnusedPreviews,
    generateHeadRender,
    getHeadRenderKey,
    headBlobUrlMap,
    pruneSkinPreviewMemory,
    type RawRenderResult,
    type RenderResult,
    setActiveSkinPreviews,
    skinBlobUrlMap,
} from './batch-skin-renderer'

class BatchSkinRenderer {
    private renderer: THREE.WebGLRenderer | null = null
    private scene: THREE.Scene | null = null
    private camera: THREE.PerspectiveCamera | null = null
    private currentModel: THREE.Group | null = null
    private transparentTexture: THREE.Texture | null = null
    private readonly width: number
    private readonly height: number

    constructor(
        private readonly skinRendering: Awaited<ReturnType<typeof loadSkinRendering>>,
        width: number = 360,
        height: number = 504,
    ) {
        this.width = width
        this.height = height
    }

    private initializeRenderer(): void {
        if (this.renderer) return

        const canvas = document.createElement('canvas')
        canvas.width = this.width
        canvas.height = this.height

        this.renderer = new THREE.WebGLRenderer({
            canvas: canvas,
            antialias: true,
            alpha: true,
            preserveDrawingBuffer: true,
        })

        this.renderer.outputColorSpace = THREE.SRGBColorSpace
        this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
        this.renderer.shadowMap.enabled = false
        this.renderer.toneMapping = THREE.NoToneMapping
        this.renderer.toneMappingExposure = 10.0
        this.renderer.setClearColor(0x000000, 0)
        this.renderer.setSize(this.width, this.height)

        this.scene = new THREE.Scene()
        this.camera = new THREE.PerspectiveCamera(20, this.width / this.height, 0.4, 1000)

        const ambientLight = new THREE.AmbientLight(0xffffff, 2)
        const directionalLight = new THREE.DirectionalLight(0xffffff, 1.2)
        directionalLight.castShadow = false
        directionalLight.position.set(2, 4, 3)
        this.scene.add(ambientLight)
        this.scene.add(directionalLight)
    }

    public async renderSkin(
        textureUrl: string,
        modelUrl: string,
        capeUrl?: string,
    ): Promise<RawRenderResult> {
        this.initializeRenderer()

        this.clearScene()

        await this.setupModel(modelUrl, textureUrl, capeUrl)

        const headPart = this.currentModel!.getObjectByName('Head')
        let lookAtTarget: [number, number, number]

        if (headPart) {
            const headPosition = new THREE.Vector3()
            headPart.getWorldPosition(headPosition)
            lookAtTarget = [headPosition.x, headPosition.y - 0.3, headPosition.z]
        } else {
            throw new Error("Failed to find 'Head' object in model.")
        }

        const frontCameraPos: [number, number, number] = [-1.3, 1, 6.3]
        const forwards = await this.renderView(frontCameraPos, lookAtTarget)

        return { forwards }
    }

    private async renderView(
        cameraPosition: [number, number, number],
        lookAtPosition: [number, number, number],
    ): Promise<Blob> {
        if (!this.camera || !this.renderer || !this.scene) {
            throw new Error('Renderer not initialized')
        }

        this.camera.position.set(...cameraPosition)
        this.camera.lookAt(...lookAtPosition)

        this.renderer.render(this.scene, this.camera)

        return await new Promise<Blob>((resolve, reject) => {
            this.renderer!.domElement.toBlob(
                (blob) => {
                    if (blob) {
                        resolve(blob)
                    } else {
                        reject(new Error('Failed to create blob from rendered canvas'))
                    }
                },
                'image/webp',
                0.9,
            )
        })
    }

    private async setupModel(
        modelUrl: string,
        textureUrl: string,
        capeUrl?: string,
    ): Promise<void> {
        if (!this.scene) {
            throw new Error('Renderer not initialized')
        }

        const { model } = await this.skinRendering.setupSkinModel(modelUrl, textureUrl, capeUrl)

        if (!capeUrl) {
            this.skinRendering.applyCapeTexture(model, null, this.getTransparentTexture())
        }

        const group = new THREE.Group()
        group.add(model)
        group.position.set(0, 0.3, 1.95)
        group.scale.set(0.8, 0.8, 0.8)

        this.scene.add(group)
        this.currentModel = group
    }

    private getTransparentTexture(): THREE.Texture {
        if (!this.transparentTexture) {
            this.transparentTexture = this.skinRendering.createTransparentTexture()
        }

        return this.transparentTexture
    }

    private clearScene(): void {
        if (!this.scene || !this.currentModel) return

        this.currentModel.traverse((object) => {
            const mesh = object as THREE.Mesh
            if (mesh.isMesh && mesh.userData.threeDSkinLayersApplied) {
                mesh.geometry.dispose()
            }
        })
        this.scene.remove(this.currentModel)
        this.currentModel.clear()
        this.currentModel = null
    }

    public dispose(): void {
        this.clearScene()

        if (this.transparentTexture) {
            this.transparentTexture.dispose()
            this.transparentTexture = null
        }

        if (this.renderer) {
            this.renderer.dispose()
        }

        this.renderer = null
        this.scene = null
        this.camera = null

        this.skinRendering.disposeCaches()
    }
}

function getModelUrlForVariant(variant: string): string {
    switch (variant) {
        case 'SLIM':
            return SlimPlayerModel
        case 'CLASSIC':
        case 'UNKNOWN':
        default:
            return ClassicPlayerModel
    }
}

const DEBUG_MODE = false

let sharedRenderer: BatchSkinRenderer | null = null
let latestPreviewGeneration = 0
let previewGenerationQueue: Promise<void> = Promise.resolve()

async function getSharedRenderer(): Promise<BatchSkinRenderer> {
    if (!sharedRenderer) {
        const skinRendering = await loadSkinRendering()
        sharedRenderer ??= new BatchSkinRenderer(skinRendering)
    }
    return sharedRenderer
}

export function disposeSharedRenderer(): void {
    if (sharedRenderer) {
        sharedRenderer.dispose()
        sharedRenderer = null
    }
}

export function generateSkinPreviews(skins: Skin[], capes: Cape[]): Promise<void> {
    setActiveSkinPreviews(skins)
    const generation = ++latestPreviewGeneration
    const skinsSnapshot = [...skins]
    const capesSnapshot = [...capes]

    const generationPromise = previewGenerationQueue.then(() =>
        generateSkinPreviewsForGeneration(skinsSnapshot, capesSnapshot, generation),
    )

    previewGenerationQueue = generationPromise.catch(() => {})

    return generationPromise
}

export function cancelSkinPreviews(): void {
    latestPreviewGeneration++
    setActiveSkinPreviews([])
}

async function generateSkinPreviewsForGeneration(
    skins: Skin[],
    capes: Cape[],
    generation: number,
): Promise<void> {
    const isCurrentGeneration = () => generation === latestPreviewGeneration

    try {
        const skinKeys = skins.map(
            (skin) => `${skin.texture_key}+${skin.variant}+${skin.cape_id ?? 'no-cape'}`,
        )
        const headKeys = skins.map((skin) => getHeadRenderKey(skin.texture_key))

        const [cachedSkinPreviews, cachedHeadPreviews] = await Promise.all([
            skinPreviewStorage.batchRetrieve(skinKeys),
            headStorage.batchRetrieve(headKeys),
        ])

        if (!isCurrentGeneration()) return

        for (let i = 0; i < skins.length; i++) {
            const skinKey = skinKeys[i]
            const headKey = headKeys[i]

            const rawCached = cachedSkinPreviews[skinKey]
            if (rawCached && !skinBlobUrlMap.has(skinKey)) {
                const cached: RenderResult = {
                    forwards: URL.createObjectURL(rawCached.forwards),
                }
                skinBlobUrlMap.set(skinKey, cached)
            }

            const cachedHead = cachedHeadPreviews[headKey]
            if (cachedHead && !headBlobUrlMap.has(headKey)) {
                headBlobUrlMap.set(headKey, URL.createObjectURL(cachedHead))
            }
        }

        for (const skin of skins) {
            if (!isCurrentGeneration()) return

            const key = `${skin.texture_key}+${skin.variant}+${skin.cape_id ?? 'no-cape'}`

            if (skinBlobUrlMap.has(key)) {
                if (DEBUG_MODE) {
                    const result = skinBlobUrlMap.get(key)!
                    URL.revokeObjectURL(result.forwards)
                    skinBlobUrlMap.delete(key)
                } else continue
            }

            const renderer = await getSharedRenderer()

            let variant = skin.variant
            if (variant === 'UNKNOWN') {
                try {
                    variant = await determineModelType(skin.texture)
                } catch (error) {
                    console.error(`Failed to determine model type for skin ${key}:`, error)
                    variant = 'CLASSIC'
                }
            }

            const modelUrl = getModelUrlForVariant(variant)
            const cape: Cape | undefined = capes.find((_cape) => _cape.id === skin.cape_id)
            const rawRenderResult = await renderer.renderSkin(
                await get_normalized_skin_texture(skin),
                modelUrl,
                cape?.texture,
            )

            if (!isCurrentGeneration()) return

            const renderResult: RenderResult = {
                forwards: URL.createObjectURL(rawRenderResult.forwards),
            }

            skinBlobUrlMap.set(key, renderResult)

            try {
                await skinPreviewStorage.store(key, rawRenderResult)
            } catch (error) {
                console.warn('Failed to store skin preview in persistent storage:', error)
            }

            const headKey = getHeadRenderKey(skin.texture_key)
            if (!headBlobUrlMap.has(headKey)) {
                await generateHeadRender(skin)
            }
        }
    } finally {
        disposeSharedRenderer()
        pruneSkinPreviewMemory()

        if (isCurrentGeneration()) {
            await cleanupUnusedPreviews(skins)

            if (DEBUG_MODE) {
                await skinPreviewStorage.debugCalculateStorage()
                await headStorage.debugCalculateStorage()
            }
        }
    }
}
