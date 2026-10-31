import { expect, it } from 'vitest'

type WorkerResponse = { type: string } & Record<string, unknown>

type WorkerScope = {
    onmessage?: (event: { data: unknown }) => void
    postMessage: (message: WorkerResponse, transfer?: unknown) => void
}

const posted: WorkerResponse[] = []

/** `mesh-worker` reads `self` at import time, so the stub has to exist before the module loads. */
async function startWorker() {
    const scope: WorkerScope = { postMessage: (message) => posted.push(message) }
    ;(globalThis as unknown as { self: WorkerScope }).self = scope
    await import('./mesh-worker')
    return (data: unknown) => scope.onmessage?.({ data })
}

const resources = {
    blockDefinitions: {},
    blockModels: {},
    defaultBlockProperties: {},
    textureUvs: {},
    missingTextureUv: [0, 0, 1, 1] as [number, number, number, number],
    bubbleUv: [0, 0.52, 0.004, 0.522] as [number, number, number, number],
}

it('a bubble column is water with a bubble pass on top', async () => {
    const send = await startWorker()
    send({
        type: 'init',
        epoch: 1,
        palette: [
            { name: 'minecraft:air', properties: {} },
            { name: 'minecraft:bubble_column', properties: { drag: 'false' } },
        ],
        resources,
        seamlessGlass: false,
    })
    posted.length = 0

    const blocks = new Uint32Array(16 * 16 * 16)
    blocks[1 * 256 + 1 * 16 + 1] = 1
    send({
        type: 'mesh',
        epoch: 1,
        jobId: 'job',
        regionId: 'region',
        chunkPosition: [0, 0, 0],
        blocks: blocks.buffer,
        neighborFaces: {},
    })

    const response = posted.find((message) => message.type === 'mesh')
    expect(response, posted.map((message) => message.type).join(',')).toBeDefined()
    const mesh = response as unknown as Record<
        string,
        { positions: Float32Array; uvs: Float32Array }
    >
    // The block has no model of its own, so everything drawn has to come from the fluid pass and
    // the baked bubbles; the response has to carry both, or the page cannot draw either.
    expect(mesh.opaque.positions.length).toBe(0)
    expect(mesh.water.positions.length).toBeGreaterThan(0)
    expect(mesh.bubble.positions.length).toBeGreaterThan(0)
    // The bubbles have to sample the sprite's corner of the atlas; the whole texture would stretch
    // every block onto each bubble and read as a white square.
    expect(mesh.bubble.uvs[0]).toBeCloseTo(0, 6)
    expect(mesh.bubble.uvs[1]).toBeCloseTo(0.52, 6)
    expect(Math.max(...mesh.bubble.uvs)).toBeCloseTo(0.522, 6)
})
