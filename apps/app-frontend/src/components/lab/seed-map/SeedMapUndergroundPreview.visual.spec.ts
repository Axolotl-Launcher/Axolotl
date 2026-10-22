import '@/test/visual-styles.scss'

import { mount, type VueWrapper } from '@vue/test-utils'
import { CanvasTexture, type WebGLRenderer } from 'three'
import { afterEach, expect, it, vi } from 'vitest'
import { page, userEvent } from 'vitest/browser'

import { SchematicPreviewScene } from '@/lab/schematic-preview/scene'
import { buildUndergroundMesh } from '@/lab/seed-map/underground-mesh'

import SeedMapUndergroundPreview from './SeedMapUndergroundPreview.vue'

const backend = vi.hoisted(() => ({ invoke: vi.fn(), maxChunks: 100 }))
vi.mock('@/lab/seed-map/underground', async (original) => {
    const module = await original<typeof import('@/lab/seed-map/underground')>()
    return {
        ...module,
        undergroundChunkQueue: (...args: Parameters<typeof module.undergroundChunkQueue>) =>
            module.undergroundChunkQueue(...args).slice(0, backend.maxChunks),
    }
})
vi.mock('@tauri-apps/api/core', () => ({ invoke: backend.invoke, isTauri: () => false }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values: Record<string, unknown> = {},
            ) =>
                message.defaultMessage.replace(/\{(\w+)\}/g, (_, key) =>
                    String(values[key] ?? key),
                ),
        }),
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        Toggle: defineComponent({
            props: ['modelValue'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        type: 'checkbox',
                        checked: props.modelValue,
                        onChange: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).checked),
                    }),
        }),
    }
})

let wrapper: VueWrapper | undefined
let host: HTMLDivElement | undefined
afterEach(() => {
    wrapper?.unmount()
    host?.remove()
    wrapper = undefined
    vi.clearAllMocks()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    backend.maxChunks = 100
})

it('does not start native work or create a scene when closed during initialization', async () => {
    const regions = vi.spyOn(SchematicPreviewScene.prototype, 'setRegions')
    backend.invoke.mockResolvedValue(new ArrayBuffer(0))
    host = document.createElement('div')
    document.body.append(host)
    wrapper = mount(SeedMapUndergroundPreview, {
        attachTo: host,
        props: {
            query: {
                seed: '12345',
                edition: 'java',
                version: '26.3',
                dimension: 'overworld',
                x: 0,
                y: 16,
                z: 0,
            },
        },
    })
    wrapper.unmount()
    wrapper = undefined
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(backend.invoke.mock.calls.length).toBe(0)
    expect(regions.mock.calls.length).toBe(0)
})

it('shows the running and completed chunk count only in the upper-right status', async () => {
    backend.maxChunks = 2
    let resolveFirst!: (buffer: ArrayBuffer) => void
    const response = () => {
        const buffer = new ArrayBuffer(16 + 18 * 18 * 384 + 18 * 18 * 2)
        const view = new DataView(buffer)
        ;[3, 0, 18, 384].forEach((value, index) => view.setUint32(index * 4, value, true))
        new Uint8Array(buffer, 16, 18 * 18 * 384).fill(1)
        return buffer
    }
    let calls = 0
    backend.invoke.mockImplementation((command: string) => {
        if (command.endsWith('seed_map_cancel_underground')) return Promise.resolve()
        return ++calls === 1
            ? new Promise((resolve) => {
                  resolveFirst = resolve
              })
            : Promise.resolve(response())
    })
    host = document.createElement('div')
    host.style.cssText = 'position:relative;width:900px;height:650px'
    document.body.append(host)
    wrapper = mount(SeedMapUndergroundPreview, {
        attachTo: host,
        props: {
            query: {
                seed: '1',
                edition: 'java',
                version: '26.3',
                dimension: 'overworld',
                x: 0,
                y: 16,
                z: 0,
            },
        },
    })
    await expect.poll(() => resolveFirst).toBeDefined()
    const status = wrapper.get('[data-testid=underground-generation-status]')
    expect(status.text()).toBe('0 chunks')
    expect(status.attributes('data-generating')).toBe('true')
    expect(status.find('svg.animate-spin').exists()).toBe(true)
    const settings = wrapper.get('[data-testid=underground-settings]')
    expect(settings.text()).not.toMatch(/chunks|Computing|Building/)
    resolveFirst(response())
    await expect.poll(() => status.attributes('data-generating')).toBe('false')
    expect(status.text()).toBe('2 chunks')
    expect(status.find('svg.animate-spin').exists()).toBe(false)
    expect(status.find('svg').exists()).toBe(true)
    expect(wrapper.findAll('button').map((button) => button.text())).toEqual(['Back to map'])
})

it('terminates an in-flight mesh worker when the preview is closed before its result arrives', async () => {
    const NativeWorker = window.Worker
    const workers = new Set<Worker>()
    let posted!: () => void
    const meshPosted = new Promise<void>((resolve) => {
        posted = resolve
    })
    vi.stubGlobal(
        'Worker',
        class extends NativeWorker {
            constructor(url: string | URL, options?: WorkerOptions) {
                super(url, options)
                workers.add(this)
            }
            postMessage(message: unknown, transfer: Transferable[]) {
                super.postMessage(message, transfer)
                if ((message as { type?: string }).type === 'mesh') posted()
            }
            terminate() {
                workers.delete(this)
                super.terminate()
            }
        },
    )
    const result = new ArrayBuffer(16 + 18 * 18 * 384 + 18 * 18 * 2)
    const header = new DataView(result)
    ;[3, 0, 18, 384].forEach((value, index) => header.setUint32(index * 4, value, true))
    new Uint8Array(result, 16, 18 * 18 * 384).fill(1)
    backend.invoke.mockResolvedValue(result)
    host = document.createElement('div')
    host.style.cssText = 'position:relative;width:800px;height:600px'
    document.body.append(host)
    wrapper = mount(SeedMapUndergroundPreview, {
        attachTo: host,
        props: {
            query: {
                seed: '12345',
                edition: 'java',
                version: '26.3',
                dimension: 'overworld',
                x: 0,
                y: 16,
                z: 0,
            },
        },
    })
    await meshPosted
    expect(workers.size).toBe(1)
    wrapper.unmount()
    wrapper = undefined
    expect(workers.size).toBe(0)
    const count = backend.invoke.mock.calls.length
    await new Promise((resolve) => setTimeout(resolve, 350))
    expect(backend.invoke.mock.calls.length).toBe(count)
})

it('reuses one worker and releases graphics, timers and late native results across repeated open and close cycles', async () => {
    const NativeWorker = window.Worker
    const workers = new Set<Worker>()
    let created = 0
    vi.stubGlobal(
        'Worker',
        class extends NativeWorker {
            constructor(url: string | URL, options?: WorkerOptions) {
                super(url, options)
                created++
                workers.add(this)
            }
            terminate() {
                workers.delete(this)
                super.terminate()
            }
        },
    )
    const render = vi.spyOn(
        SchematicPreviewScene.prototype as unknown as { render: () => void },
        'render',
    )
    const textureDispose = vi.spyOn(CanvasTexture.prototype, 'dispose')
    const dispose = vi.spyOn(SchematicPreviewScene.prototype, 'dispose')
    const response = () => {
        const buffer = new ArrayBuffer(16 + 18 * 18 * 384 + 18 * 18 * 2)
        const header = new DataView(buffer)
        ;[3, 0, 18, 384].forEach((value, index) => header.setUint32(index * 4, value, true))
        const blocks = new Uint8Array(buffer, 16, 18 * 18 * 384).fill(1)
        for (let x = 1; x < 17; x++)
            for (let z = 1; z < 17; z++)
                for (let y = 70; y < 100; y++) blocks[(x * 18 + z) * 384 + y] = 0
        for (let column = 0; column < 18 * 18; column++)
            header.setInt16(16 + blocks.length + column * 2, 100, true)
        return buffer
    }
    for (let cycle = 0; cycle < 3; cycle++) {
        let resolveLate: ((value: ArrayBuffer) => void) | undefined
        let calls = 0
        backend.invoke.mockImplementation((command: string) => {
            if (command.endsWith('seed_map_cancel_underground')) return Promise.resolve()
            calls++
            return calls > 3
                ? new Promise((resolve) => {
                      resolveLate = resolve
                  })
                : Promise.resolve(response())
        })
        host = document.createElement('div')
        host.style.cssText = 'position:relative;width:800px;height:600px'
        document.body.append(host)
        const workerCountBefore = created
        wrapper = mount(SeedMapUndergroundPreview, {
            attachTo: host,
            props: {
                query: {
                    seed: '12345',
                    edition: 'java',
                    version: '26.3',
                    dimension: 'overworld',
                    x: 0,
                    y: 16,
                    z: 0,
                },
            },
        })
        await expect.poll(() => resolveLate, { timeout: 30000 }).toBeDefined()
        expect(created - workerCountBefore).toBe(1)
        expect(workers.size).toBe(1)
        const canvas = wrapper.get('canvas').element as HTMLCanvasElement
        const context = canvas.getContext('webgl2')!
        expect(context.isContextLost()).toBe(false)
        const beforeDispose = dispose.mock.calls.length
        wrapper.unmount()
        wrapper = undefined
        expect(dispose.mock.calls.length).toBe(beforeDispose + 1)
        expect(workers.size).toBe(0)
        const scene = dispose.mock.contexts.at(-1) as unknown as {
            chunks: Map<string, unknown>
            regions: Map<string, unknown>
            scene: { children: unknown[] }
            renderer: WebGLRenderer
        }
        expect(scene.chunks.size).toBe(0)
        expect(scene.regions.size).toBe(0)
        expect(scene.scene.children.length).toBe(0)
        await expect.poll(() => context.isContextLost()).toBe(true)
        expect(canvas.width).toBe(1)
        expect(canvas.height).toBe(1)
        expect(scene.renderer.info.memory.geometries).toBe(0)
        expect(scene.renderer.info.memory.textures).toBe(0)
        expect(textureDispose.mock.calls.length).toBe(cycle + 1)
        const requestsAfterClose = calls
        const rendersAfterClose = render.mock.calls.length
        resolveLate!(response())
        await new Promise((resolve) => setTimeout(resolve, 350))
        expect(calls).toBe(requestsAfterClose)
        expect(render.mock.calls.length).toBe(rendersAfterClose)
        expect(workers.size).toBe(0)
        host.remove()
    }
})

it('targets visible block coordinates after chunk translation and ignores hidden or removed chunks', () => {
    host = document.createElement('div')
    host.style.cssText = 'position:relative;width:600px;height:500px'
    document.body.append(host)
    const canvas = document.createElement('canvas')
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
        doubleSided: true,
    })
    try {
        const blocks = new Uint8Array(125)
        blocks[(2 * 5 + 2) * 5 + 2] = 1
        const mesh = buildUndergroundMesh({
            blocks,
            width: 5,
            height: 5,
            truncated: false,
            textureUvs: { 'minecraft:block/stone': [0, 0, 1, 1] },
        })
        scene.setRegions([{ id: 'terrain', min: [0, 0, 0], max: [5, 5, 5] }])
        scene.setChunk('terrain', [0, 0, 0], mesh.layers.terrain, mesh.layers.water)
        scene.setChunkOffset('terrain', [0, 0, 0], [10, 20, 30])
        scene.setCameraPosition([15, 25, 35])
        expect(scene.crosshairBlock()).toEqual([12, 22, 32])
        scene.setRegionVisible('terrain', false)
        expect(scene.crosshairBlock()).toBeUndefined()
        scene.setRegionVisible('terrain', true)
        scene.removeChunk('terrain', [0, 0, 0])
        expect(scene.crosshairBlock()).toBeUndefined()
    } finally {
        scene.dispose()
    }
})

it('loads textured cavity geometry, switches layers, keeps the view on failure and cancels pending generation on exit', async () => {
    backend.maxChunks = 2
    const volume = () => {
        const result = new ArrayBuffer(16 + 18 * 18 * 384 + 18 * 18 * 2)
        const header = new DataView(result)
        ;[3, 0, 18, 384].forEach((value, index) => header.setUint32(index * 4, value, true))
        const blocks = new Uint8Array(result, 16, 18 * 18 * 384).fill(1)
        for (let x = 1; x < 17; x++)
            for (let z = 1; z < 17; z++)
                for (let y = 64; y < 112; y++) blocks[(x * 18 + z) * 384 + y] = 0
        for (let column = 0; column < 18 * 18; column++)
            header.setInt16(16 + blocks.length + column * 2, 100, true)
        blocks[(8 * 18 + 8) * 384 + 64] = 5
        blocks[(10 * 18 + 8) * 384 + 64] = 4
        blocks[(6 * 18 + 8) * 384 + 63] = 16
        return result
    }
    let fail = false
    let pending = false
    backend.invoke.mockImplementation((command: string) => {
        if (command.endsWith('seed_map_cancel_underground')) return Promise.resolve()
        if (pending) return new Promise(() => undefined)
        if (fail) return Promise.reject(new Error('Generation fixture failed'))
        return Promise.resolve(volume())
    })
    host = document.createElement('div')
    host.style.cssText = 'position:relative;width:900px;height:650px'
    document.body.append(host)
    wrapper = mount(SeedMapUndergroundPreview, {
        attachTo: host,
        props: {
            query: {
                seed: '12345',
                edition: 'java',
                version: '26.3',
                dimension: 'overworld',
                x: 0,
                y: 16,
                z: 0,
            },
        },
    })
    await expect
        .poll(() => wrapper?.text(), { timeout: 30000 })
        .toContain('Click the view to explore')
    expect(wrapper.get('[data-testid=underground-generation-status]').text()).toMatch(
        /[1-9]\d* chunks/,
    )
    expect(wrapper.text()).toContain('Water (estimated)')
    expect(wrapper.text()).toContain('Lava (estimated)')
    expect(wrapper.text()).toMatch(/X -?\d+ · Y -?\d+ · Z -?\d+ · 8 blocks\/s/)
    await userEvent.click(page.getByRole('checkbox', { name: 'Bedrock' }))
    expect(wrapper.get('[data-testid=underground-settings]').text()).not.toContain('chunks')
    expect(
        wrapper
            .findAll('button')
            .some((button) => /Generate around camera|Cancel generation/.test(button.text())),
    ).toBe(false)
    await userEvent.click(page.getByRole('checkbox', { name: 'Surface layer' }))
    expect((wrapper.findAll('input')[1].element as HTMLInputElement).checked).toBe(false)
    await expect
        .poll(
            () =>
                backend.invoke.mock.calls.filter(([command]) =>
                    command.endsWith('seed_map_underground_volume'),
                ).length,
        )
        .toBeGreaterThan(1)
    const requests = backend.invoke.mock.calls
        .filter(([command]) => command.endsWith('seed_map_underground_volume'))
        .map(([, args]) => args.request)
    expect(requests.every((request) => request.chunk)).toBe(true)
    expect(new Set(requests.map((request) => `${request.x}:${request.y}:${request.z}`)).size).toBe(
        requests.length,
    )
    await expect
        .poll(() =>
            wrapper
                ?.get('[data-testid=underground-generation-status]')
                .attributes('data-generating'),
        )
        .toBe('false')
    const canvas = wrapper.get('canvas').element as HTMLCanvasElement
    expect(canvas.width).toBeGreaterThan(1)
    await userEvent.click(page.elementLocator(canvas), { position: { x: 800, y: 500 } })
    await expect.poll(() => document.pointerLockElement).toBe(canvas)
    const generationCalls = () =>
        backend.invoke.mock.calls.filter(([command]) =>
            command.endsWith('seed_map_underground_volume'),
        ).length
    const callsBeforeWalking = generationCalls()
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW' }))
    await expect.poll(generationCalls, { timeout: 6000 }).toBeGreaterThan(callsBeforeWalking)
    window.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyW' }))
    expect(wrapper.get('canvas').element).toBe(canvas)
    expect(wrapper.text()).not.toContain('Could not generate')
    const beforeMove = wrapper.text().match(/X -?\d+ · Y -?\d+ · Z -?\d+ · \d+ blocks\/s/)?.[0]
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Space' }))
    await expect
        .poll(() => wrapper?.text().match(/X -?\d+ · Y -?\d+ · Z -?\d+ · \d+ blocks\/s/)?.[0])
        .not.toBe(beforeMove)
    document.exitPointerLock()
    await expect.poll(() => document.pointerLockElement).toBe(null)
    await new Promise((resolve) => setTimeout(resolve, 180))
    const afterUnlock = wrapper.text().match(/X -?\d+ · Y -?\d+ · Z -?\d+ · \d+ blocks\/s/)?.[0]
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(wrapper.text().match(/X -?\d+ · Y -?\d+ · Z -?\d+ · \d+ blocks\/s/)?.[0]).toBe(
        afterUnlock,
    )
    await userEvent.click(page.getByRole('checkbox', { name: 'Cave boundaries' }))
    expect((wrapper.findAll('input')[0].element as HTMLInputElement).checked).toBe(false)
    fail = true
    backend.maxChunks += 2
    await expect.poll(() => wrapper?.text()).toContain('Your previous view is kept')
    expect(wrapper.get('canvas').element).toBe(canvas)
    expect(wrapper.text()).toMatch(/\d+ chunks/)
    pending = true
    await userEvent.click(page.getByRole('button', { name: 'Retry region' }))
    await expect
        .poll(() =>
            wrapper
                ?.get('[data-testid=underground-generation-status]')
                .attributes('data-generating'),
        )
        .toBe('true')
    wrapper.unmount()
    wrapper = undefined
    expect(
        backend.invoke.mock.calls.some(([command]) =>
            command.endsWith('seed_map_cancel_underground'),
        ),
    ).toBe(true)
})
