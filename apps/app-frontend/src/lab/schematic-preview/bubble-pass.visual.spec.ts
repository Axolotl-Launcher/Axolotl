import { expect, it, vi } from 'vitest'

import { createSchematicResources } from '@/lab/schematic-preview/resources'
import { SchematicPreviewScene } from '@/lab/schematic-preview/scene'

type Mesh = {
    positions: Float32Array
    normals: Float32Array
    uvs: Float32Array
    colors: Float32Array
    blockPositions: Float32Array
}

const CORNERS: [number, number, number][] = [
    [0.3, 0.7, 0.5],
    [0.7, 0.7, 0.5],
    [0.7, 0.3, 0.5],
    [0.3, 0.3, 0.5],
]

/**
 * One quad on the plane z = 0.5, sampling the bubble sprite's rectangle of the atlas. `appendQuad`
 * takes the front normal from `(v1 - v0) x (v2 - v0)`, so the order decides which side is the front.
 */
function quadMesh(front: 'camera' | 'away', uv: [number, number, number, number]): Mesh {
    const order = front === 'away' ? [0, 1, 2, 0, 2, 3] : [0, 3, 2, 0, 2, 1]
    const vertexUvs = [
        [uv[0], uv[1]],
        [uv[2], uv[1]],
        [uv[2], uv[3]],
        [uv[0], uv[3]],
    ]
    const positions: number[] = []
    const normals: number[] = []
    const uvs: number[] = []
    const colors: number[] = []
    const blockPositions: number[] = []
    for (const index of order) {
        positions.push(...CORNERS[index])
        normals.push(0, 0, front === 'away' ? -1 : 1)
        uvs.push(...vertexUvs[index])
        colors.push(1, 1, 1)
        blockPositions.push(0, 0, 0)
    }
    return {
        positions: new Float32Array(positions),
        normals: new Float32Array(normals),
        uvs: new Float32Array(uvs),
        colors: new Float32Array(colors),
        blockPositions: new Float32Array(blockPositions),
    }
}

function frame() {
    return new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve(undefined))),
    )
}

function readPixels(canvas: HTMLCanvasElement) {
    const target = document.createElement('canvas')
    target.width = canvas.width
    target.height = canvas.height
    const context = target.getContext('2d')
    if (!context) throw new Error('no 2d context')
    context.drawImage(canvas, 0, 0)
    return context.getImageData(0, 0, target.width, target.height).data
}

/** The clear colour is `#0a0a0a`, so anything brighter than the background was drawn. */
function isLit(data: Uint8ClampedArray, pixel: number) {
    const index = pixel * 4
    return Math.max(data[index], data[index + 1], data[index + 2]) > 30
}

/** The pixels the last draw added, so the grid the scene always shows cannot mask the answer. */
function addedPixels(before: Uint8ClampedArray, after: Uint8ClampedArray) {
    const added: number[] = []
    for (let pixel = 0; pixel < after.length / 4; pixel += 1) {
        if (!isLit(before, pixel) && isLit(after, pixel)) added.push(pixel)
    }
    return added
}

function mountScene() {
    const host = document.createElement('div')
    host.style.cssText = 'position:relative;width:400px;height:400px'
    document.body.append(host)
    const canvas = document.createElement('canvas')
    canvas.width = 400
    canvas.height = 400
    host.append(canvas)
    const scene = new SchematicPreviewScene({
        canvas,
        onSelect: () => undefined,
        onFocus: () => undefined,
        onContextLost: () => undefined,
        onContextRestored: () => undefined,
        onViewModeChange: () => undefined,
        onWalkLockChange: () => undefined,
        onWalkSpeedChange: () => undefined,
    })
    return { host, canvas, scene }
}

const palette = [{ name: 'minecraft:bubble_column', properties: { drag: 'false' } }]

it('a bubble samples the sprite and is drawn from the back as well as the front', async () => {
    const { host, canvas, scene } = mountScene()
    try {
        const resources = await createSchematicResources('latest', palette)
        const uv = resources.workerResources.bubbleUv
        // Below the block textures the atlas is empty apart from the sprite, so an 8 texel square
        // well away from it samples nothing at all.
        const blank: [number, number, number, number] = [
            100 / 2048,
            3000 / 4096,
            108 / 2048,
            3008 / 4096,
        ]
        scene.setTextures({ atlas: resources.texture })
        scene.setRegions([{ id: 'region', min: [0, 0, 0], max: [1, 1, 1] }])
        // The camera looks down the (1, 0.8, 1) diagonal from the bounds, so the quad's `away`
        // winding is the one turned away from it.
        scene.fitView()
        await frame()
        const empty = readPixels(canvas)

        scene.setChunk('region', [0, 0, 0], { bubble: quadMesh('camera', uv) })
        await frame()
        expect(addedPixels(empty, readPixels(canvas)).length).toBeGreaterThan(0)

        // The same quad pointed at empty atlas space draws nothing, so what showed up above came
        // from the sprite and not from a pass painting the quad without a texture.
        scene.setChunk('region', [0, 0, 0], { bubble: quadMesh('camera', blank) })
        await frame()
        expect(addedPixels(empty, readPixels(canvas)).length).toBe(0)

        scene.setChunk('region', [0, 0, 0], { bubble: quadMesh('away', uv) })
        await frame()
        expect(addedPixels(empty, readPixels(canvas)).length).toBeGreaterThan(0)
    } finally {
        scene.dispose()
        host.remove()
    }
})

it('a sprite that will not load costs the bubbles rather than the whole preview', async () => {
    const realFetch = globalThis.fetch
    vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) =>
        String(input).includes('particle-bubble')
            ? new Response(null, { status: 404 })
            : realFetch(input, init),
    )
    const { host, canvas, scene } = mountScene()
    try {
        // The atlas is what every other pass needs, so the sprite failing must still produce
        // resources; it may only cost the bubbles themselves.
        const resources = await createSchematicResources('latest', palette)
        expect(resources.warnings).toEqual([expect.stringContaining('sprite failed to load')])
        const uv = resources.workerResources.bubbleUv
        // The pass keeps sampling the atlas at an empty spot, so a bubble column draws nothing
        // instead of the white quads a pass without a texture would draw.
        expect(uv[0]).toBe(uv[2])
        expect(uv[1]).toBe(uv[3])
        scene.setTextures({ atlas: resources.texture })
        scene.setRegions([{ id: 'region', min: [0, 0, 0], max: [1, 1, 1] }])
        scene.fitView()
        await frame()
        const empty = readPixels(canvas)
        scene.setChunk('region', [0, 0, 0], { bubble: quadMesh('camera', uv) })
        await frame()
        expect(addedPixels(empty, readPixels(canvas)).length).toBe(0)
    } finally {
        scene.dispose()
        host.remove()
        vi.unstubAllGlobals()
    }
})
