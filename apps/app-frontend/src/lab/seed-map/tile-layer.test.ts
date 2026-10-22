import assert from 'node:assert/strict'
import test from 'node:test'

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
        assert.ok(job)
        const bitmap = {
            width: job.request.width,
            height: job.request.height,
            closed: false,
            close() {
                assert.equal(this.closed, false)
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
            assert.ok(count++ < 2000, 'tile jobs must terminate')
            await complete()
        }
    }
    return { layer, jobs, bitmaps, coverage, errors, complete, drain, loading: () => loading }
}

test('LoD bounds surface sampling to two samples per screen pixel', () => {
    for (const blocksPerPixel of [0.25, 1, 2, 2.1, 4, 8, 8.1, 16, 32, 64, 128, 256]) {
        const level = seedMapTileLevel(blocksPerPixel)
        assert.ok([1, 4, 16, 64, 256].includes(level.scale))
        assert.ok(blocksPerPixel / level.scale <= 2)
        assert.equal(level.pixels, 256)
    }
})

test('clearing the 2D map frees cached and late bitmaps while allowing the map to reopen', async () => {
    const state = harness()
    state.layer.refresh(view())
    const first = await state.complete()
    state.layer.clear()
    assert.equal(first.closed, true)
    await state.drain()
    assert.ok(state.bitmaps.every((bitmap) => bitmap.closed))
    assert.equal(state.layer.drawable(view()).length, 0)
    state.layer.refresh(view({ epoch: 2 }))
    const reopened = await state.complete()
    assert.equal(reopened.closed, false)
    state.layer.dispose()
    await state.drain()
})

test('preview covers the same world area using sixteen times fewer pixels', () => {
    for (const scale of [1, 4, 16, 64]) {
        const level = { scale, pixels: 256 }
        const preview = seedMapPreviewLevel(level)!
        assert.equal(preview.scale * preview.pixels, level.scale * level.pixels)
        assert.equal(level.pixels ** 2 / preview.pixels ** 2, 16)
    }
    assert.equal(seedMapPreviewLevel({ scale: 256, pixels: 256 }), undefined)
})

test('query identity excludes view changes and includes every generation input', () => {
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
        assert.notEqual(seedMapTileContextKey({ ...context, ...change }), key)
    }
    assert.equal(
        seedMapTileContextKey({ ...context, highlightBiomes: [3, 1] }),
        seedMapTileContextKey({ ...context, highlightBiomes: [1, 3] }),
    )
})

test('renders visible previews before detail, then prefetches outside the viewport', async () => {
    const h = harness()
    const next = view({ bounds: { minX: 0, minZ: 0, maxX: 2048, maxZ: 1024 } })
    h.layer.refresh(next)
    assert.deepEqual(
        h.jobs.map((job) => [job.request.x, job.request.width]),
        [[0, 64]],
    )
    await h.complete()
    assert.deepEqual(
        h.jobs.map((job) => [job.request.x, job.request.width]),
        [[1024, 64]],
    )
    await h.complete()
    assert.deepEqual(h.coverage, [1])
    assert.equal(h.loading(), true)
    assert.deepEqual(
        h.jobs.map((job) => [job.request.x, job.request.width]),
        [[0, 256]],
    )
    await h.complete()
    assert.equal(h.jobs[0].request.x, 1024)
    await h.complete()
    assert.equal(h.loading(), false)
    assert.ok(
        h.jobs[0].request.x < 0 ||
            h.jobs[0].request.z < 0 ||
            h.jobs[0].request.x >= 2048 ||
            h.jobs[0].request.z >= 1024,
    )
    h.layer.dispose()
    await h.drain()
})

test('aligns tiles across negative coordinates without gaps at zero', async () => {
    const h = harness()
    const next = view({ bounds: { minX: -1024, minZ: -1024, maxX: 0, maxZ: 0 } })
    h.layer.refresh(next)
    assert.equal(h.jobs[0].request.x, -1024)
    assert.equal(h.jobs[0].request.z, -1024)
    await h.complete()
    assert.deepEqual(h.coverage, [1])
    h.layer.dispose()
    await h.drain()
})

test('coverage accepts a mix of preview and detail and a preview failure does not hide valid detail', async () => {
    const h = harness()
    const next = view({ bounds: { minX: 0, minZ: 0, maxX: 2048, maxZ: 1024 } })
    h.layer.refresh(next)
    h.jobs.shift()!.reject(new Error('preview failed'))
    await new Promise((resolve) => setImmediate(resolve))
    await h.complete()
    assert.deepEqual(h.coverage, [])
    await h.complete()
    assert.deepEqual(h.coverage, [1])
    assert.deepEqual(h.errors, [])
    h.layer.dispose()
    await h.drain()
})

test('LoD changes retain old bitmaps, draw new target last and reuse them on reverse zoom', async () => {
    const h = harness()
    const fine = view()
    h.layer.refresh(fine)
    await h.drain()
    const old = h.layer.drawable(fine).at(-1)!
    const coarse = view({ epoch: 2, level: { scale: 16, pixels: 256 } })
    h.layer.invalidate(coarse)
    assert.ok(h.layer.drawable(coarse).some((tile) => tile.bitmap === old.bitmap))
    assert.ok(h.bitmaps.every((bitmap) => !bitmap.closed))
    h.layer.refresh(coarse)
    await h.complete()
    assert.ok(h.layer.drawable(coarse).some((tile) => tile.bitmap === old.bitmap))
    await h.complete()
    assert.equal(h.layer.drawable(coarse).at(-1)!.scale, 16)
    const reverse = { ...fine, epoch: 3 }
    h.layer.refresh(reverse)
    assert.equal(h.layer.drawable(reverse).at(-1)!.bitmap, old.bitmap)
    assert.ok(h.bitmaps.every((bitmap) => !bitmap.closed))
    h.layer.dispose()
    await h.drain()
    assert.ok(h.bitmaps.every((bitmap) => bitmap.closed))
})

test('panning across epochs reuses in-flight tiles and reports current coverage once', async () => {
    const h = harness()
    h.layer.refresh(view())
    const next = view({ epoch: 2, center: { x: 520, z: 512 } })
    h.layer.invalidate(next)
    h.layer.refresh(next)
    assert.equal(h.jobs.length, 1)
    await h.complete()
    assert.deepEqual(h.coverage, [2])
    await h.drain()
    assert.deepEqual(h.coverage, [2])
    const count = h.bitmaps.length
    h.layer.refresh(view({ epoch: 3 }))
    assert.equal(h.jobs.length, 0)
    assert.equal(h.bitmaps.length, count)
    assert.deepEqual(h.coverage, [2, 3])
    h.layer.dispose()
})

test('seed changes discard stale results while all epochs share one concurrency cap', async () => {
    const h = harness()
    h.layer.refresh(view({ concurrency: 8 }))
    assert.equal(h.jobs.length, 4)
    const next = view({ epoch: 2, concurrency: 8, context: { ...view().context, seed: '456' } })
    h.layer.refresh(next)
    assert.equal(h.jobs.length, 4)
    const stale = await h.complete()
    assert.equal(stale.closed, true)
    assert.equal(h.jobs.length, 4)
    assert.equal(h.jobs.at(-1)!.request.seed, '456')
    assert.deepEqual(h.layer.drawable(next), [])
    await h.drain()
    assert.ok(h.layer.drawable(next).length > 0)
    h.layer.dispose()
})

test('invalidation cancels queued work and pauses coverage callbacks until refresh', async () => {
    const h = harness()
    h.layer.refresh(view())
    const next = view({ epoch: 2 })
    h.layer.invalidate(next)
    await h.complete()
    assert.equal(h.jobs.length, 0)
    assert.deepEqual(h.coverage, [])
    assert.equal(h.loading(), false)
    h.layer.refresh(next)
    assert.deepEqual(h.coverage, [2])
    assert.equal(h.jobs[0].request.width, 256)
    h.layer.dispose()
    await h.drain()
})

test('failed tiles can be retried without closing already visible tiles', async () => {
    const h = harness()
    h.layer.refresh(view())
    const preview = await h.complete()
    const failure = h.jobs.shift()!
    failure.reject(new Error('native renderer unavailable'))
    await new Promise((resolve) => setImmediate(resolve))
    h.layer.invalidate(view({ epoch: 2 }))
    await h.drain()
    assert.deepEqual(h.errors, ['native renderer unavailable'])
    assert.equal(preview.closed, false)
    h.layer.refresh(view({ epoch: 2 }))
    assert.equal(h.jobs[0].request.width, 256)
    assert.equal(h.jobs[0].request.x, 0)
    await h.drain()
    h.layer.dispose()
})

test('bitmap cache stays within 32 MiB and frees evicted and disposed resources', async () => {
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
    assert.ok(
        live.reduce((sum, bitmap) => sum + bitmap.width * bitmap.height * 4, 0) <= 32 * 1024 * 1024,
    )
    assert.ok(h.bitmaps.some((bitmap) => bitmap.closed))
    assert.ok(h.layer.drawable(next).length > 0)
    h.layer.dispose()
    assert.ok(h.bitmaps.every((bitmap) => bitmap.closed))
})

test('large viewports retain every visible preview and detail tile instead of evicting coverage', async () => {
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
    assert.equal(
        drawn.filter((tile) => tile.scale === 1 && tile.pixels === 256).length,
        visibleCount,
    )
    assert.equal(
        drawn.filter((tile) => tile.scale === 4 && tile.pixels === 64).length,
        visibleCount,
    )
    assert.equal(h.loading(), false)
    assert.deepEqual(h.coverage, [1])
    h.layer.dispose()
})
