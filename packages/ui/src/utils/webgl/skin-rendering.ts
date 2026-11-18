import * as THREE from 'three'
import type { GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

import { isArmorPreviewMesh } from './armor-preview-object.ts'
import { createSolidSkinLayerGeometry, type SolidSkinLayerDefinition } from './solid-skin-layer.ts'

export interface SkinRendererConfig {
    textureColorSpace?: THREE.ColorSpace
    textureFlipY?: boolean
    textureMagFilter?: THREE.MagnificationTextureFilter
    textureMinFilter?: THREE.MinificationTextureFilter
}

const MODEL_PIXEL_SIZE = 1 / 16
const NON_LEG_VERTICAL_OFFSET = -MODEL_PIXEL_SIZE / 2
const BASE_VOXEL_SCALE = 1.15
const BODY_VOXEL_WIDTH_SCALE = 1.05
const VOXEL_HEIGHT_SCALE = 1.035
const HEAD_VOXEL_SCALE = 1.18
const MOD_HEAD_ROTATION_OFFSET = 0.6
const MOD_HEAD_SCALE_PIVOT = 0.25
const MOD_HEAD_VERTICAL_OFFSET = -0.04
const MOD_BODY_POSITION_Y = -0.2
const MOD_ARM_POSITION_Y = -0.1
const MOD_ARM_POSITION_X = 0.998
const MOD_SLIM_ARM_POSITION_X = 0.499
// Skin textures use 8-bit alpha. Keep every non-zero alpha value while still
// discarding fully transparent fragments before they can write to the depth buffer.
const SKIN_LAYER_ALPHA_TEST = 0.5 / 255

export function configureSkinMaterial(
    material: THREE.MeshStandardMaterial,
    hasTranslucentPixels: boolean,
): void {
    material.transparent = hasTranslucentPixels
    material.alphaTest = hasTranslucentPixels ? SKIN_LAYER_ALPHA_TEST : 0.1
    // Treat the player as a textured surface, not a glass volume: the nearest
    // face writes depth so hidden inner and back faces do not accumulate color.
    material.depthWrite = true
    // Three.js rewrites fragment alpha through smoothstep when alpha-to-coverage
    // is enabled. Use it only for binary cutouts, never for translucent texels.
    material.alphaToCoverage = !hasTranslucentPixels
    material.needsUpdate = true
}

/** Aligns the torso, arms, head, and cape with the stationary legs. */
function offsetNonLegModelParts(model: THREE.Object3D): void {
    if (model.userData.nonLegPartsOffsetApplied) return

    const nonLegRoots = new Set(['Head', 'Right_Arm', 'Left_Arm', 'Body_2', 'Body_Layer', 'Cape'])
    model.traverse((node) => {
        if (nonLegRoots.has(node.name)) node.position.y += NON_LEG_VERTICAL_OFFSET
    })
    model.userData.nonLegPartsOffsetApplied = true
}

/**
 * Rebuilds the outer layer using 3D Skin Layers' fast-render structure: a
 * correctly mapped surface box plus per-pixel side and back extrusion.
 */
function minecraftAxisOffset(
    voxelCenter: number,
    meshPosition: number,
    scale: number,
    vanillaCenter: number,
): number {
    return -((voxelCenter + meshPosition) * scale - vanillaCenter) * MODEL_PIXEL_SIZE
}

function headCenterOffset(): number {
    const voxelCenter = (-4 + MOD_HEAD_ROTATION_OFFSET) * MODEL_PIXEL_SIZE
    const vanillaCenter = -4 * MODEL_PIXEL_SIZE
    const transformedCenter =
        -MOD_HEAD_SCALE_PIVOT +
        HEAD_VOXEL_SCALE * (MOD_HEAD_SCALE_PIVOT + MOD_HEAD_VERTICAL_OFFSET + voxelCenter)
    return -(transformedCenter - vanillaCenter)
}

function getSkinLayerDefinition(name: string, isSlimArm: boolean): SolidSkinLayerDefinition | null {
    if (name === 'Hat_Layer') {
        return {
            width: 8,
            height: 8,
            depth: 8,
            u: 32,
            v: 0,
            pixelScale: [HEAD_VOXEL_SCALE, HEAD_VOXEL_SCALE, HEAD_VOXEL_SCALE],
            centerOffset: [0, headCenterOffset(), 0],
        }
    }
    const bodyScale = [BODY_VOXEL_WIDTH_SCALE, VOXEL_HEIGHT_SCALE, BASE_VOXEL_SCALE] as const
    const limbScale = [BASE_VOXEL_SCALE, VOXEL_HEIGHT_SCALE, BASE_VOXEL_SCALE] as const
    const bodyCenterOffset = minecraftAxisOffset(6, MOD_BODY_POSITION_Y, VOXEL_HEIGHT_SCALE, 6)
    if (name === 'Body_Layer') {
        return {
            width: 8,
            height: 12,
            depth: 4,
            u: 16,
            v: 32,
            pixelScale: bodyScale,
            centerOffset: [0, bodyCenterOffset, 0],
        }
    }
    if (name === 'Right_Leg_Layer') {
        return {
            width: 4,
            height: 12,
            depth: 4,
            u: 0,
            v: 32,
            pixelScale: limbScale,
            centerOffset: [0, bodyCenterOffset, 0],
        }
    }
    if (name === 'Left_Leg_Layer') {
        return {
            width: 4,
            height: 12,
            depth: 4,
            u: 0,
            v: 48,
            pixelScale: limbScale,
            centerOffset: [0, bodyCenterOffset, 0],
        }
    }
    if (name === 'Right_Arm_Layer' || name === 'Left_Arm_Layer') {
        const side = name === 'Right_Arm_Layer' ? -1 : 1
        const armCenter = isSlimArm ? 0.5 : 1
        const armPosition = isSlimArm ? MOD_SLIM_ARM_POSITION_X : MOD_ARM_POSITION_X
        return {
            width: isSlimArm ? 3 : 4,
            height: 12,
            depth: 4,
            u: name === 'Right_Arm_Layer' ? 40 : 48,
            v: name === 'Right_Arm_Layer' ? 32 : 48,
            pixelScale: limbScale,
            centerOffset: [
                minecraftAxisOffset(0, side * armPosition, BASE_VOXEL_SCALE, side * armCenter),
                minecraftAxisOffset(4, MOD_ARM_POSITION_Y, VOXEL_HEIGHT_SCALE, 4),
                0,
            ],
        }
    }
    return null
}

function readSkinPixels(texture: THREE.Texture): Uint8ClampedArray | null {
    const image = texture.image as CanvasImageSource | undefined
    if (!image) return null

    try {
        const canvas = document.createElement('canvas')
        canvas.width = canvas.height = 64
        const context = canvas.getContext('2d')
        if (!context) return null
        context.drawImage(image, 0, 0, 64, 64)
        return context.getImageData(0, 0, 64, 64).data
    } catch {
        // Cross-origin images may not be readable. The regular GLTF layer remains
        // as a graceful fallback in that case.
        return null
    }
}

function hasTranslucentSkinPixels(pixels: Uint8ClampedArray | null): boolean {
    if (!pixels) return true

    for (let alphaIndex = 3; alphaIndex < pixels.length; alphaIndex += 4) {
        const alpha = pixels[alphaIndex]
        if (alpha > 0 && alpha < 255) return true
    }

    return false
}

export function applyThreeDSkinLayers(model: THREE.Object3D, texture?: THREE.Texture): void {
    offsetNonLegModelParts(model)
    const pixels = texture ? readSkinPixels(texture) : null
    const hasTranslucentPixels = hasTranslucentSkinPixels(pixels)
    if (!pixels || !texture) return
    model.traverse((child) => {
        const mesh = child as THREE.Mesh
        if (!mesh.isMesh || !mesh.name.endsWith('_Layer') || !mesh.geometry) return

        // Voxel occupancy depends on the current texture's alpha channel. Preserve an
        // untouched source geometry and rebuild from it whenever the skin changes;
        // otherwise a new texture is mapped onto the previous skin's voxel silhouette.
        const sourceGeometry = mesh.userData.skinLayerSourceGeometry as
            THREE.BufferGeometry | undefined
        if (sourceGeometry) {
            mesh.geometry.dispose()
            mesh.geometry = sourceGeometry.clone()
        } else {
            // GLTF clones share BufferGeometry objects. Keep both the renderer's working
            // copy and a pristine source owned by this preview instance.
            mesh.geometry = mesh.geometry.clone()
            mesh.userData.skinLayerSourceGeometry = mesh.geometry.clone()
        }
        const meshBounds = new THREE.Box3().setFromBufferAttribute(
            mesh.geometry.getAttribute('position') as THREE.BufferAttribute,
        )
        const isSlimArm =
            mesh.name.includes('Arm') && meshBounds.getSize(new THREE.Vector3()).x < 0.25
        const definition = getSkinLayerDefinition(mesh.name, isSlimArm)
        if (definition) {
            const voxelGeometry = createSolidSkinLayerGeometry(mesh, pixels, definition)
            if (voxelGeometry) {
                const voxelMaterials = Array.isArray(mesh.material)
                    ? mesh.material
                    : [mesh.material]
                voxelMaterials.forEach((material) => {
                    if (!(material instanceof THREE.MeshStandardMaterial)) return
                    configureSkinMaterial(material, hasTranslucentPixels)
                    material.polygonOffset = false
                    material.polygonOffsetFactor = 0
                    material.polygonOffsetUnits = 0
                })
                mesh.geometry.dispose()
                mesh.geometry = voxelGeometry
                mesh.userData.threeDSkinLayersApplied = true
                return
            }
        }
    })
}

const modelCache: Map<string, GLTF> = new Map()
const modelPromiseCache: Map<string, Promise<GLTF>> = new Map()
const textureCache: Map<string, THREE.Texture> = new Map()
const texturePromiseCache: Map<string, Promise<THREE.Texture>> = new Map()

export async function loadModel(modelUrl: string): Promise<GLTF> {
    if (modelCache.has(modelUrl)) {
        return modelCache.get(modelUrl)!
    }

    if (modelPromiseCache.has(modelUrl)) {
        return modelPromiseCache.get(modelUrl)!
    }

    const loader = new GLTFLoader()
    const promise = new Promise<GLTF>((resolve, reject) => {
        loader.load(
            modelUrl,
            (gltf) => {
                modelCache.set(modelUrl, gltf)
                resolve(gltf)
            },
            undefined,
            reject,
        )
    }).finally(() => {
        modelPromiseCache.delete(modelUrl)
    })

    modelPromiseCache.set(modelUrl, promise)
    return promise
}

export async function loadTexture(
    textureUrl: string,
    config: SkinRendererConfig = {},
): Promise<THREE.Texture> {
    const cacheKey = `${textureUrl}_${JSON.stringify(config)}`

    if (textureCache.has(cacheKey)) {
        return textureCache.get(cacheKey)!
    }

    if (texturePromiseCache.has(cacheKey)) {
        return texturePromiseCache.get(cacheKey)!
    }

    const textureLoader = new THREE.TextureLoader()
    const promise = new Promise<THREE.Texture>((resolve, reject) => {
        textureLoader.load(
            textureUrl,
            (texture) => {
                texture.colorSpace = config.textureColorSpace ?? THREE.SRGBColorSpace
                texture.flipY = config.textureFlipY ?? false
                texture.magFilter = config.textureMagFilter ?? THREE.NearestFilter
                texture.minFilter = config.textureMinFilter ?? THREE.NearestFilter

                textureCache.set(cacheKey, texture)
                resolve(texture)
            },
            undefined,
            reject,
        )
    }).finally(() => {
        texturePromiseCache.delete(cacheKey)
    })

    texturePromiseCache.set(cacheKey, promise)
    return promise
}

function applyMap(mat: THREE.MeshStandardMaterial, texture: THREE.Texture | null): boolean {
    const hadMap = mat.map !== null
    const hasMap = texture !== null

    if (mat.map !== texture) {
        mat.map = texture
    }

    return hadMap !== hasMap
}

function setShaderMaterialProperties(
    mat: THREE.MeshStandardMaterial,
    properties: {
        alphaTest: number
        flatShading: boolean
        side: THREE.Side
        toneMapped: boolean
        transparent?: boolean
    },
): boolean {
    let needsUpdate = false

    if (mat.alphaTest !== properties.alphaTest) {
        mat.alphaTest = properties.alphaTest
        needsUpdate = true
    }

    if (mat.flatShading !== properties.flatShading) {
        mat.flatShading = properties.flatShading
        needsUpdate = true
    }

    if (mat.side !== properties.side) {
        mat.side = properties.side
        needsUpdate = true
    }

    if (mat.toneMapped !== properties.toneMapped) {
        mat.toneMapped = properties.toneMapped
        needsUpdate = true
    }

    if (properties.transparent !== undefined && mat.transparent !== properties.transparent) {
        mat.transparent = properties.transparent
        needsUpdate = true
    }

    return needsUpdate
}

function setCommonMaterialProperties(mat: THREE.MeshStandardMaterial): void {
    if (mat.metalness !== 0) {
        mat.metalness = 0
    }

    if (mat.color.getHex() !== 0xffffff) {
        mat.color.set(0xffffff)
    }

    if (mat.roughness !== 1) {
        mat.roughness = 1
    }

    if (!mat.depthTest) {
        mat.depthTest = true
    }

    if (!mat.depthWrite) {
        mat.depthWrite = true
    }
}

export function applyTexture(model: THREE.Object3D, texture: THREE.Texture): void {
    const hasTranslucentPixels = hasTranslucentSkinPixels(readSkinPixels(texture))
    model.traverse((child) => {
        if ((child as THREE.Mesh).isMesh) {
            const mesh = child as THREE.Mesh
            if (isArmorPreviewMesh(mesh)) return
            const isSkinLayer = mesh.name.endsWith('_Layer')
            mesh.renderOrder = isSkinLayer ? 1 : 0
            const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]

            materials.forEach((mat: THREE.Material) => {
                if (mat instanceof THREE.MeshStandardMaterial) {
                    if (mat.name !== 'cape') {
                        const mapNeedsUpdate = applyMap(mat, texture)
                        const propertiesNeedUpdate = setShaderMaterialProperties(mat, {
                            alphaTest: hasTranslucentPixels ? SKIN_LAYER_ALPHA_TEST : 0.1,
                            flatShading: true,
                            side: THREE.FrontSide,
                            toneMapped: false,
                            transparent: hasTranslucentPixels,
                        })

                        setCommonMaterialProperties(mat)
                        configureSkinMaterial(mat, hasTranslucentPixels)

                        if (mapNeedsUpdate || propertiesNeedUpdate) {
                            mat.needsUpdate = true
                        }
                    }
                }
            })
        }
    })
}

export function applyCapeTexture(
    model: THREE.Object3D,
    texture: THREE.Texture | null,
    transparentTexture?: THREE.Texture,
): void {
    model.traverse((child) => {
        if ((child as THREE.Mesh).isMesh) {
            const mesh = child as THREE.Mesh
            const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]

            materials.forEach((mat: THREE.Material) => {
                if (mat instanceof THREE.MeshStandardMaterial) {
                    if (mat.name === 'cape') {
                        const nextMap = texture || transparentTexture || null
                        const mapNeedsUpdate = applyMap(mat, nextMap)
                        const propertiesNeedUpdate = setShaderMaterialProperties(mat, {
                            alphaTest: 0.1,
                            flatShading: true,
                            side: THREE.DoubleSide,
                            toneMapped: false,
                            transparent: !texture || !!transparentTexture,
                        })

                        setCommonMaterialProperties(mat)

                        if (mapNeedsUpdate || propertiesNeedUpdate) {
                            mat.needsUpdate = true
                        }

                        mat.visible = !!texture
                    }
                }
            })
        }
    })
}

export function findBodyNode(model: THREE.Object3D): THREE.Object3D | null {
    let bodyNode: THREE.Object3D | null = null

    model.traverse((node) => {
        if (node.name === 'Body') {
            bodyNode = node
        }
    })

    return bodyNode
}

export function createTransparentTexture(): THREE.Texture {
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = 1
    const ctx = canvas.getContext('2d') as CanvasRenderingContext2D
    ctx.clearRect(0, 0, 1, 1)

    const texture = new THREE.CanvasTexture(canvas)
    texture.needsUpdate = true
    texture.colorSpace = THREE.SRGBColorSpace
    texture.flipY = false
    texture.magFilter = THREE.NearestFilter
    texture.minFilter = THREE.NearestFilter

    return texture
}

export async function setupSkinModel(
    modelUrl: string,
    textureUrl: string,
    capeTextureUrl?: string,
    config: SkinRendererConfig = {},
): Promise<{
    model: THREE.Object3D
    bodyNode: THREE.Object3D | null
    capeRendered: boolean
}> {
    const [gltf, texture] = await Promise.all([
        loadModel(modelUrl),
        loadTexture(textureUrl, config),
    ])

    const model = gltf.scene.clone()
    applyTexture(model, texture)
    applyThreeDSkinLayers(model, texture)

    const capeTexture = capeTextureUrl
        ? await loadTexture(capeTextureUrl, config).catch((error) => {
              console.warn('Could not load the optional cape texture:', error)
              return null
          })
        : null
    applyCapeTexture(model, capeTexture)

    const bodyNode = findBodyNode(model)

    return { model, bodyNode, capeRendered: capeTexture !== null }
}

export function disposeCaches(): void {
    Array.from(textureCache.values()).forEach((texture) => {
        texture.dispose()
    })

    textureCache.clear()
    texturePromiseCache.clear()
    modelCache.clear()
    modelPromiseCache.clear()
}
