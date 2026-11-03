import { expect, it } from 'vitest'

import type { SeedMapTile, SeedMapTileRequest } from './backend.ts'
import {
    createSeedMapTileLayer,
    seedMapPreviewLevel,
    seedMapTileContextKey,
    seedMapTileLevel,
    type SeedMapTileView,
} from './tile-layer.ts'

function view(overrides: Partial<SeedMapTileView> = {}): SeedMapTileView {
    return {
        epoch: 1,
        context: {
            seed: '123',
            edition: 'java',
            version: '26.3',
            dimension: 'overworld',
            terrain: true,
            contours: false,
        },
        bounds: { minX: 0, minZ: 0, maxX: 1024, maxZ: 1024 },
        center: { x: 512, z: 512 },
        level: { scale: 4, pixels: 256 },
        concurrency: 1,
        ...overrides,
    }
}

function harness() {
    const jobs: {
        request: SeedMapTileRequest
        resolve: (tile: SeedMapTile) => void
        reject: (error: Error) => void
    }[] = []
    const bitmaps: { width: number; height: number; closed: boolean; close: () => void }[] = []
    const coverage: number[] = []
    const errors: string[] = []
    let loading = false
    const layer = createSeedMapTileLayer({
        render: (request) =>
            new Promise((resolve, reject) => jobs.push({ request, resolve, reject })),
        onUpdate: () => {},
        onLoading: (value) => {
            loading = value
        },
        onCoverage: (epoch) => {
            coverage.push(epoch)
        },
        onError: (message) => {
            errors.push(message)
        },
    })
    async function complete() {
        const job = jobs.shift()!
        expect(job).toBeTruthy()
        const bitmap = {
            width: job.request.width,
            height: job.request.height,
            closed: false,
            close() {
                expect(this.closed).toBe(false)
                this.closed = true
            },
        }
        bitmaps.push(bitmap)
        job.resolve({
            epoch: job.request.epoch,
            width: bitmap.width,
            height: bitmap.height,
            bitmap: bitmap as unknown as ImageBitmap,
            approximate: true,
        })
        await new Promise((resolve) => setImmediate(resolve))
        return bitmap
    }
    async function drain() {
        let count = 0
        while (jobs.length) {
            expect(count++ < 2000, 'tile jobs must terminate').toBeTruthy()
            await complete()
        }
    }
    return { layer, jobs, bitmaps, coverage, errors, complete, drain, loading: () => loading }
}

it('LoD bounds surface sampling to two samples per screen pixel', () => {
    for (const blocksPerPixel of [0.25, 1, 2, 2.1, 4, 8, 8.1, 16, 32, 64, 128, 256]) {
        const level = seedMapTileLevel(blocksPerPixel)
        expect([1, 4, 16, 64, 256].includes(level.scale)).toBeTruthy()
        expect(blocksPerPixel / level.scale <= 2).toBeTruthy()
        expect(level.pixels).toBe(256)
    }
})

it('clearing the 2D map frees cached and late bitmaps while allowing the map to reopen', async () => {
    const state = harness()
    state.layer.refresh(view())
    const first = await state.complete()
    state.layer.clear()
    expect(first.closed).toBe(true)
    await state.drain()
    expect(state.bitmaps.every((bitmap) => bitmap.closed)).toBeTruthy()
    expect(state.layer.drawable(view()).length).toBe(0)
    state.layer.refresh(view({ epoch: 2 }))
    const reopened = await state.complete()
    expect(reopened.closed).toBe(false)
    state.layer.dispose()
    await state.drain()
})

it('preview covers the same world area using sixteen times fewer pixels', () => {
    for (const scale of [1, 4, 16, 64]) {
        const level = { scale, pixels: 256 }
        const preview = seedMapPreviewLevel(level)!
        expect(preview.scale * preview.pixels).toBe(level.scale * level.pixels)
        expect(level.pixels ** 2 / preview.pixels ** 2).toBe(16)
    }
    expect(seedMapPreviewLevel({ scale: 256, pixels: 256 })).toBe(undefined)
})

it('query identity excludes view changes and includes every generation input', () => {
    const context = view().context
    const key = seedMapTileContextKey(context)
    for (const change of [
        { seed: '456' },
        { edition: 'java-large-biomes' },
        { version: '26.2' },
        { dimension: 'nether' },
        { elevation: -32 },
        { terrain: false },
        { contours: true },
        { highlightBiomes: [1] },
    ] as Partial<typeof context>[]) {
        expect(seedMapTileContextKey({ ...context, ...change })).not.toBe(key)
    }
    expect(seedMapTileContextKey({ ...context, highlightBiomes: [3, 1] })).toBe(
        seedMapTileContextKey({ ...context, highlightBiomes: [1, 3] }),
    )
})

it('renders visible previews before detail, then prefetches outside the viewport', async () => {
    const h = harness()
    const next = view({ bounds: { minX: 0, minZ: 0, maxX: 2048, maxZ: 1024 } })
    h.layer.refresh(next)
    expect(h.jobs.map((job) => [job.request.x, job.request.width])).toEqual([[0, 64]])
    await h.complete()
    expect(h.jobs.map((job) => [job.request.x, job.request.width])).toEqual([[1024, 64]])
    await h.complete()
    expect(h.coverage).toEqual([1])
    expect(h.loading()).toBe(true)
    expect(h.jobs.map((job) => [job.request.x, job.request.width])).toEqual([[0, 256]])
    await h.complete()
    expect(h.jobs[0].request.x).toBe(1024)
    await h.complete()
    expect(h.loading()).toBe(false)
    expect(
        h.jobs[0].request.x < 0 ||
            h.jobs[0].request.z < 0 ||
            h.jobs[0].request.x >= 2048 ||
            h.jobs[0].request.z >= 1024,
    ).toBeTruthy()
    h.layer.dispose()
    await h.drain()
})

it('aligns tiles across negative coordinates without gaps at zero', async () => {
    const h = harness()
    const next = view({ bounds: { minX: -1024, minZ: -1024, maxX: 0, maxZ: 0 } })
    h.layer.refresh(next)
    expect(h.jobs[0].request.x).toBe(-1024)
    expect(h.jobs[0].request.z).toBe(-1024)
    await h.complete()
    expect(h.coverage).toEqual([1])
    h.layer.dispose()
    await h.drain()
})

it('coverage accepts a mix of preview and detail and a preview failure does not hide valid detail', async () => {
    const h = harness()
    const next = view({ bounds: { minX: 0, minZ: 0, maxX: 2048, maxZ: 1024 } })
    h.layer.refresh(next)
    h.jobs.shift()!.reject(new Error('preview failed'))
    await new Promise((resolve) => setImmediate(resolve))
    await h.complete()
    expect(h.coverage).toEqual([])
    await h.complete()
    expect(h.coverage).toEqual([1])
    expect(h.errors).toEqual([])
    h.layer.dispose()
    await h.drain()
})

it('LoD changes retain old bitmaps, draw new target last and reuse them on reverse zoom', async () => {
    const h = harness()
    const fine = view()
    h.layer.refresh(fine)
    await h.drain()
    const old = h.layer.drawable(fine).at(-1)!
    const coarse = view({ epoch: 2, level: { scale: 16, pixels: 256 } })
    h.layer.invalidate(coarse)
    expect(h.layer.drawable(coarse).some((tile) => tile.bitmap === old.bitmap)).toBeTruthy()
    expect(h.bitmaps.every((bitmap) => !bitmap.closed)).toBeTruthy()
    h.layer.refresh(coarse)
    await h.complete()
    expect(h.layer.drawable(coarse).some((tile) => tile.bitmap === old.bitmap)).toBeTruthy()
    await h.complete()
    expect(h.layer.drawable(coarse).at(-1)!.scale).toBe(16)
    const reverse = { ...fine, epoch: 3 }
    h.layer.refresh(reverse)
    expect(h.layer.drawable(reverse).at(-1)!.bitmap).toBe(old.bitmap)
    expect(h.bitmaps.every((bitmap) => !bitmap.closed)).toBeTruthy()
    h.layer.dispose()
    await h.drain()
    expect(h.bitmaps.every((bitmap) => bitmap.closed)).toBeTruthy()
})

it('panning across epochs reuses in-flight tiles and reports current coverage once', async () => {
    const h = harness()
    h.layer.refresh(view())
    const next = view({ epoch: 2, center: { x: 520, z: 512 } })
    h.layer.invalidate(next)
    h.layer.refresh(next)
    expect(h.jobs.length).toBe(1)
    await h.complete()
    expect(h.coverage).toEqual([2])
    await h.drain()
    expect(h.coverage).toEqual([2])
    const count = h.bitmaps.length
    h.layer.refresh(view({ epoch: 3 }))
    expect(h.jobs.length).toBe(0)
    expect(h.bitmaps.length).toBe(count)
    expect(h.coverage).toEqual([2, 3])
    h.layer.dispose()
})

it('seed changes discard stale results while all epochs share one concurrency cap', async () => {
    const h = harness()
    h.layer.refresh(view({ concurrency: 8 }))
    expect(h.jobs.length).toBe(4)
    const next = view({ epoch: 2, concurrency: 8, context: { ...view().context, seed: '456' } })
    h.layer.refresh(next)
    expect(h.jobs.length).toBe(4)
    const stale = await h.complete()
    expect(stale.closed).toBe(true)
    expect(h.jobs.length).toBe(4)
    expect(h.jobs.at(-1)!.request.seed).toBe('456')
    expect(h.layer.drawable(next)).toEqual([])
    await h.drain()
    expect(h.layer.drawable(next).length > 0).toBeTruthy()
    h.layer.dispose()
})

it('invalidation cancels queued work and pauses coverage callbacks until refresh', async () => {
    const h = harness()
    h.layer.refresh(view())
    const next = view({ epoch: 2 })
    h.layer.invalidate(next)
    await h.complete()
    expect(h.jobs.length).toBe(0)
    expect(h.coverage).toEqual([])
    expect(h.loading()).toBe(false)
    h.layer.refresh(next)
    expect(h.coverage).toEqual([2])
    expect(h.jobs[0].request.width).toBe(256)
    h.layer.dispose()
    await h.drain()
})

it('failed tiles can be retried without closing already visible tiles', async () => {
    const h = harness()
    h.layer.refresh(view())
    const preview = await h.complete()
    const failure = h.jobs.shift()!
    failure.reject(new Error('native renderer unavailable'))
    await new Promise((resolve) => setImmediate(resolve))
    h.layer.invalidate(view({ epoch: 2 }))
    await h.drain()
    expect(h.errors).toEqual(['native renderer unavailable'])
    expect(preview.closed).toBe(false)
    h.layer.refresh(view({ epoch: 2 }))
    expect(h.jobs[0].request.width).toBe(256)
    expect(h.jobs[0].request.x).toBe(0)
    await h.drain()
    h.layer.dispose()
})

it('bitmap cache stays within 32 MiB and frees evicted and disposed resources', async () => {
    const h = harness()
    let next = view()
    for (let epoch = 1; epoch <= 20; epoch++) {
        next = view({
            epoch,
            context: { ...view().context, seed: String(epoch) },
            level: { scale: 256, pixels: 256 },
        })
        h.layer.refresh(next)
        await h.drain()
    }
    const live = h.bitmaps.filter((bitmap) => !bitmap.closed)
    expect(
        live.reduce((sum, bitmap) => sum + bitmap.width * bitmap.height * 4, 0) <= 32 * 1024 * 1024,
    ).toBeTruthy()
    expect(h.bitmaps.some((bitmap) => bitmap.closed)).toBeTruthy()
    expect(h.layer.drawable(next).length > 0).toBeTruthy()
    h.layer.dispose()
    expect(h.bitmaps.every((bitmap) => bitmap.closed)).toBeTruthy()
})

it('large viewports retain every visible preview and detail tile instead of evicting coverage', async () => {
    const h = harness()
    const next = view({
        bounds: { minX: 0, minZ: 0, maxX: 3840, maxZ: 2160 },
        center: { x: 1920, z: 1080 },
        level: { scale: 1, pixels: 256 },
        concurrency: 4,
    })
    h.layer.refresh(next)
    await h.drain()
    const drawn = h.layer.drawable(next)
    const visibleCount = Math.ceil(3840 / 256) * Math.ceil(2160 / 256)
    expect(drawn.filter((tile) => tile.scale === 1 && tile.pixels === 256).length).toBe(
        visibleCount,
    )
    expect(drawn.filter((tile) => tile.scale === 4 && tile.pixels === 64).length).toBe(visibleCount)
    expect(h.loading()).toBe(false)
    expect(h.coverage).toEqual([1])
    h.layer.dispose()
})
