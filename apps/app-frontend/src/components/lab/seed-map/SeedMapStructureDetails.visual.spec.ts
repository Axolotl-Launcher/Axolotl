import '@/test/visual-styles.scss'

import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { page, userEvent } from 'vitest/browser'

import type { SeedMapStructureDetails as StructureDetails } from '@/lab/seed-map/advanced'
import type { SeedMapFeatureKind } from '@/lab/seed-map/features'
import { buildStructureBlockMeshes } from '@/lab/seed-map/structure-block-mesh'
import type {
    StructureBlocksResponse,
    StructureTemplatePack,
} from '@/lab/seed-map/structure-blocks'
import { seedMapTemplatePieces } from '@/lab/seed-map/structure-blocks'

import SeedMapLayoutPreview from './SeedMapLayoutPreview.vue'
import SeedMapStructureDetails from './SeedMapStructureDetails.vue'

const calls = vi.hoisted(() => ({ details: vi.fn(), loot: vi.fn() }))
const language = vi.hoisted(() => ({ current: 'en-US' }))
vi.mock('@/lab/seed-map/advanced', () => ({
    getSeedMapStructureDetails: calls.details,
    getSeedMapContainerLoot: calls.loot,
}))
vi.mock('@/lab/shared/minecraft/item-images', () => ({ loadMinecraftItemImage: vi.fn() }))
vi.mock('@modrinth/ui', async () => {
    const { ref } = await import('vue')
    const { IntlMessageFormat } = await import('intl-messageformat')
    const en = (await import('@/locales/en-US/index.json')).default as Record<
        string,
        { message: string }
    >
    const zh = (await import('@/locales/zh-CN/index.json')).default as Record<
        string,
        { message: string }
    >
    return {
        Button: (
            await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue')
        ).default,
        Combobox: (await import('../../../../../../packages/ui/src/components/base/Combobox.vue'))
            .default,
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            locale: ref(language.current),
            formatMessage: (
                message: { id: string; defaultMessage: string },
                values?: Record<string, string | number>,
            ) =>
                String(
                    new IntlMessageFormat(
                        (language.current === 'zh-CN' ? zh : en)[message.id]?.message ??
                            message.defaultMessage,
                        language.current,
                    ).format(values),
                ),
        }),
    }
})

const query = {
    seed: '1',
    edition: 'java' as const,
    version: '26.3',
    dimension: 'overworld' as const,
    x: 0,
    z: 0,
    featureMask: 1,
}
const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.resetAllMocks()
    language.current = 'en-US'
})

function render(actions?: string) {
    if (!document.getElementById('teleports')) {
        const target = document.createElement('div')
        target.id = 'teleports'
        document.body.append(target)
    }
    const wrapper = mount(SeedMapStructureDetails, {
        props: { query, kind: 'village', title: 'Village' },
        slots: actions ? { actions } : {},
        attachTo: document.body,
    })
    wrappers.push(wrapper)
    ;(wrapper.element as HTMLElement).style.cssText =
        'width: 430px; height: 720px; background: var(--surface-2); color: var(--color-text-primary)'
    return wrapper
}

it.each([
    ['1.13.2', 'end_city/tower_top'],
    ['1.14.4', 'pillager_outpost/watchtower'],
    ['1.19.2', 'end_city/tower_top'],
    ['1.20', 'pillager_outpost/watchtower'],
    ['1.13.2', 'igloo/bottom'],
    ['1.14.4', 'igloo/bottom'],
    ['1.13.2', 'shipwreck/with_mast'],
    ['26.2', 'end_city/tower_top'],
])('meshes historical %s %s assets through the structure worker', async (version, templateId) => {
    const worker = new Worker(
        new URL('../../../lab/seed-map/structure-block-worker.ts', import.meta.url),
        { type: 'module' },
    )
    try {
        const response = await new Promise<StructureBlocksResponse>((resolve, reject) => {
            worker.onmessage = (event) => resolve(event.data)
            worker.onerror = (event) => reject(new Error(event.message))
            worker.postMessage({
                version,
                pieces: [
                    {
                        id: 0,
                        name: templateId,
                        templateId,
                        positionKnown: true,
                        position: { x: 0, y: 0, z: 0 },
                        rotation: 0,
                    },
                ],
            })
        })
        expect(response.type, response.type === 'error' ? response.message : 'ready').toBe('ready')
        if (response.type === 'ready') {
            expect(response.result.blockCount).toBeGreaterThan(100)
            expect(response.result.meshes.some((mesh) => mesh.positions.length > 0)).toBe(true)
            expect(response.result.missingModels).not.toContain('minecraft:oak_slab')
            expect(response.result.missingModels).not.toContain('minecraft:spruce_slab')
        }
    } finally {
        worker.terminate()
    }
})

it('loads original End city and ship geometry on demand and switches modes without rebuilding the canvas', async () => {
    const pieces = [
        {
            id: 0,
            name: 'base_floor',
            rotation: 0,
            positionKnown: true,
            position: { x: 0, y: 0, z: 0 },
            min: { x: 0, y: 0, z: 0 },
            max: { x: 9, y: 3, z: 9 },
        },
        {
            id: 1,
            name: 'second_floor_1',
            rotation: 0,
            positionKnown: true,
            position: { x: -1, y: 0, z: -1 },
            min: { x: -1, y: 0, z: -1 },
            max: { x: 10, y: 7, z: 10 },
        },
        {
            id: 2,
            name: 'third_floor_1',
            rotation: 0,
            positionKnown: true,
            position: { x: -2, y: 4, z: -2 },
            min: { x: -2, y: 4, z: -2 },
            max: { x: 11, y: 11, z: 11 },
        },
        {
            id: 3,
            name: 'third_roof',
            rotation: 0,
            positionKnown: true,
            position: { x: -3, y: 12, z: -3 },
            min: { x: -3, y: 12, z: -3 },
            max: { x: 12, y: 13, z: 12 },
        },
        {
            id: 4,
            name: 'ship',
            rotation: 90,
            positionKnown: true,
            position: { x: 34, y: 8, z: 0 },
            min: { x: 6, y: 8, z: 0 },
            max: { x: 34, y: 31, z: 12 },
        },
    ]
    const probe = new Worker(
        new URL('../../../lab/seed-map/structure-block-worker.ts', import.meta.url),
        { type: 'module' },
    )
    try {
        const result = await new Promise<StructureBlocksResponse>((resolve, reject) => {
            probe.onmessage = (event) => resolve(event.data)
            probe.onerror = (event) => reject(new Error(event.message))
            probe.postMessage({ version: '26.3', pieces })
        })
        expect(result.type, JSON.stringify(result.type === 'error' ? result : 'ready')).toBe(
            'ready',
        )
        if (result.type === 'ready') {
            expect(result.result.blockCount).toBeGreaterThan(1000)
            expect(result.result.missingModels).toEqual(['minecraft:dragon_wall_head'])
            expect(
                result.result.meshes.every(
                    (mesh) => mesh.positions.length > 0 && mesh.positions.every(Number.isFinite),
                ),
            ).toBe(true)
            const triples = result.result.meshes.flatMap((mesh) => {
                expect(mesh.colors.length).toBe(mesh.positions.length)
                expect(
                    mesh.colors.every(
                        (value) => Number.isFinite(value) && value >= 0 && value <= 1,
                    ),
                ).toBe(true)
                return Array.from({ length: mesh.colors.length / 3 }, (_, index) =>
                    Array.from(mesh.colors.subarray(index * 3, index * 3 + 3)),
                )
            })
            expect(triples.some(([r, g, b]) => r > g * 1.5 && b > g * 1.5)).toBe(true)
            expect(triples.some(([r, g, b]) => g > b * 1.5 && r > b * 1.5)).toBe(true)
        }
    } finally {
        probe.terminate()
    }
    const wrapper = mount(SeedMapLayoutPreview, {
        props: { pieces, kind: 'end-city', version: '26.3', label: (piece) => piece.name },
        attachTo: document.body,
    })
    wrappers.push(wrapper)
    ;(wrapper.element as HTMLElement).style.cssText =
        'width: 430px; background: var(--surface-2); color: var(--color-text-primary); padding: 12px'
    const canvas = wrapper.get('canvas').element as HTMLCanvasElement
    // The first preview also imports Three.js and initializes WebGL on the CI runner.
    await vi.waitFor(() => expect(canvas.dataset.ready).toBe('true'), { timeout: 10000 })
    await vi.waitFor(() => expect(Number(canvas.dataset.chestCount)).toBeGreaterThan(0), {
        timeout: 10000,
    })
    expect(canvas.dataset.renderMode).toBe('frames')
    const gl = canvas.getContext('webgl2')!
    const pixels = () => {
        const result = new Uint8Array(canvas.width * canvas.height * 4)
        gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, result)
        return result
    }
    await vi.waitFor(() => expect(pixels().some((value) => value > 0)).toBe(true), {
        timeout: 10000,
    })
    const initialFrame = pixels()
    const rect = canvas.getBoundingClientRect()
    canvas.dispatchEvent(
        new WheelEvent('wheel', {
            deltaY: -150,
            clientX: rect.x + rect.width / 2,
            clientY: rect.y + rect.height / 2,
            bubbles: true,
            cancelable: true,
        }),
    )
    await vi.waitFor(() => expect(pixels()).not.toEqual(initialFrame), { timeout: 10000 })
    const zoomedFrame = pixels()
    await wrapper.get('[data-testid="seed-map-blocks-mode"]').trigger('click')
    await vi.waitFor(() => expect(canvas.dataset.renderMode).toBe('blocks'), { timeout: 20000 })
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    await vi.waitFor(() => expect(pixels()).not.toEqual(zoomedFrame), { timeout: 10000 })
    await wrapper.setProps({ selected: 4 })
    const selected = pixels()
    await wrapper.setProps({ selected: 0 })
    await vi.waitFor(() => expect(pixels()).not.toEqual(selected), { timeout: 10000 })
    await wrapper.setProps({ selected: undefined })
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE) {
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/seed-map-end-city-blocks.png`,
        })
    }
    const blocksImage = pixels()
    await wrapper.get('[data-testid="seed-map-frames-mode"]').trigger('click')
    expect(canvas.dataset.renderMode).toBe('frames')
    await vi.waitFor(() => expect(pixels()).not.toEqual(blocksImage), { timeout: 10000 })
    const frames = pixels()
    expect(frames).toEqual(zoomedFrame)
    await wrapper.get('[data-testid="seed-map-blocks-mode"]').trigger('click')
    expect(canvas.dataset.renderMode).toBe('blocks')
    expect(wrapper.get('canvas').element).toBe(canvas)
    await vi.waitFor(() => expect(pixels()).not.toEqual(frames), { timeout: 10000 })
    await wrapper.get('[data-testid="seed-map-frames-mode"]').trigger('click')
    await vi.waitFor(() => expect(pixels()).toEqual(frames), { timeout: 10000 })
    await wrapper.setProps({ version: '26.2' })
    expect(wrapper.find('[data-testid="seed-map-blocks-mode"]').exists()).toBe(true)
    expect(canvas.dataset.renderMode).toBe('frames')
})

it.each([
    'shipwreck',
    'igloo',
    'outpost',
    'ruined-portal',
    'nether-fossil',
] as SeedMapFeatureKind[])(
    'opens texture-colored templates for %s from the real details entry',
    async (kind) => {
        const source: StructureDetails = {
            supported: true,
            truncated: false,
            containers: [],
            limits:
                kind === 'outpost' || kind === 'ruined-portal'
                    ? ['partial_layout', 'height_placeholder']
                    : kind === 'igloo' || kind === 'shipwreck'
                      ? ['height_placeholder']
                      : ['layout_unavailable'],
            variant: {
                rotation: 90,
                mirror: 1,
                start: kind === 'nether-fossil' ? 13 : 1,
                biome: 1,
                abandoned: false,
                giant: false,
                underground: false,
                airPocket: false,
                basement: true,
                cracked: false,
                size: 4,
                offset: { x: 3, y: 60, z: 5 },
                size3d: { x: 7, y: 5, z: 8 },
            },
            pieces:
                kind === 'shipwreck'
                    ? [
                          {
                              id: 0,
                              name: 'shipwreck/rightsideup_full',
                              rotation: 90,
                              positionKnown: true,
                              position: { x: 19, y: 64, z: 11 },
                          },
                      ]
                    : kind === 'outpost'
                      ? [
                            {
                                id: 0,
                                name: 'pillager_outpost/watchtower',
                                rotation: 0,
                                positionKnown: true,
                                position: { x: 0, y: 64, z: 0 },
                            },
                        ]
                      : kind === 'igloo'
                        ? [
                              {
                                  id: 0,
                                  name: 'igloo/top',
                                  rotation: 90,
                                  positionKnown: true,
                                  position: { x: 0, y: 90, z: 0 },
                              },
                          ]
                        : [],
        }
        expect(seedMapTemplatePieces(source, query, kind).length).toBeGreaterThan(0)
        const probe = new Worker(
            new URL('../../../lab/seed-map/structure-block-worker.ts', import.meta.url),
            { type: 'module' },
        )
        try {
            const response = await new Promise<StructureBlocksResponse>((resolve, reject) => {
                probe.onmessage = (event) => resolve(event.data)
                probe.onerror = (event) => reject(new Error(event.message))
                probe.postMessage({
                    version: '26.3',
                    pieces: seedMapTemplatePieces(source, query, kind),
                })
            })
            expect(response.type, response.type === 'error' ? response.message : 'ready').toBe(
                'ready',
            )
        } finally {
            probe.terminate()
        }
        calls.details.mockResolvedValue(source)
        const wrapper = render()
        await wrapper.setProps({ kind, title: kind, query: { ...query, featureMask: 1 << 22 } })
        await flushPromises()
        await wrapper.findAll('[role="tab"]')[1].trigger('click')
        const canvas = wrapper.get('canvas').element as HTMLCanvasElement
        await vi.waitFor(() => expect(canvas.dataset.ready).toBe('true'), { timeout: 10000 })
        await wrapper.get('[data-testid="seed-map-blocks-mode"]').trigger('click')
        await vi.waitFor(() => expect(canvas.dataset.renderMode).toBe('blocks'), { timeout: 20000 })
        expect(wrapper.find('[role="alert"]').exists()).toBe(false)
        const gl = canvas.getContext('webgl2')!
        await vi.waitFor(
            () => {
                const pixels = new Uint8Array(canvas.width * canvas.height * 4)
                gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels)
                expect(pixels.some((value, index) => index % 4 === 3 && value > 0)).toBe(true)
            },
            { timeout: 10000 },
        )
        if (source.limits.includes('height_placeholder'))
            expect(wrapper.text()).toContain('Some Y coordinates are placeholders')
        if (source.limits.includes('partial_layout'))
            expect(wrapper.text()).toContain('The engine simulates only part of this structure')
        if (kind === 'nether-fossil') expect(wrapper.text()).toContain('Nether fossil 14')
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/seed-map-${kind}-blocks.png`,
            })
        await wrapper.get('[data-testid="seed-map-frames-mode"]').trigger('click')
        expect(canvas.dataset.renderMode).toBe('frames')
    },
)

it('assigns texture-derived colors to individual model faces and preserves them through rotation', () => {
    const pack: StructureTemplatePack = {
        schemaVersion: 2,
        version: '26.3',
        dataVersion: 5023,
        templates: {
            'end_city/base_floor': {
                size: [1, 1, 1],
                overwriteAir: true,
                palette: [{ name: 'minecraft:purpur_block', properties: {} }],
                blocks: [[0, 0, 0, 0]],
            },
        },
        textureColors: { 'minecraft:block/top': [1, 0, 0], 'minecraft:block/side': [0, 1, 0] },
        resources: {
            defaultBlockProperties: {},
            blockDefinitions: {
                'minecraft:purpur_block': { variants: { '': { model: 'minecraft:block/test' } } },
            },
            blockModels: {
                'minecraft:block/test': {
                    textures: { top: 'minecraft:block/top', side: 'minecraft:block/side' },
                    elements: [
                        {
                            from: [0, 0, 0],
                            to: [16, 16, 16],
                            faces: { up: { texture: '#top' }, north: { texture: '#side' } },
                        },
                    ],
                },
            },
        },
    }
    for (const rotation of [0, 90, 180, 270]) {
        const result = buildStructureBlockMeshes(
            [
                {
                    id: 0,
                    name: 'base_floor',
                    rotation,
                    positionKnown: true,
                    position: { x: 0, y: 0, z: 0 },
                },
            ],
            pack,
        )
        const colors = Array.from(result.meshes[0].colors)
        const red = colors.filter((value, index) => index % 3 === 0 && value === 1).length
        const green = colors.filter((value, index) => index % 3 === 1 && value === 1).length
        expect(red).toBe(6)
        expect(green).toBe(6)
        expect(colors.filter((value, index) => index % 3 === 2).every((value) => value === 0)).toBe(
            true,
        )
    }
})

it('meshes every enabled template variant with original resources and bounded worker output', async () => {
    const catalog = (await import('@/lab/seed-map/assets/catalog.json')).default
    const names = Object.keys(catalog.versions['26.3'].templates).filter(
        (id) => !id.startsWith('pillager_outpost/') || id === 'pillager_outpost/watchtower',
    )
    const worker = new Worker(
        new URL('../../../lab/seed-map/structure-block-worker.ts', import.meta.url),
        { type: 'module' },
    )
    try {
        const response = await new Promise<StructureBlocksResponse>((resolve, reject) => {
            worker.onmessage = (event) => resolve(event.data)
            worker.onerror = (event) => reject(new Error(event.message))
            worker.postMessage({
                version: '26.3',
                pieces: names.map((name, id) => ({
                    id,
                    name,
                    templateId: name,
                    rotation: (id % 4) * 90,
                    mirror: name.startsWith('ruined_portal/') && id % 2 === 0,
                    positionKnown: true,
                    position: { x: id * 128, y: 0, z: 0 },
                })),
            })
        })
        expect(response.type, response.type === 'error' ? response.message : 'ready').toBe('ready')
        if (response.type === 'ready') {
            expect(response.result.meshes.length).toBe(names.length)
            expect(response.result.missingModels).toEqual([
                'minecraft:deepslate',
                'minecraft:deepslate_tile_slab',
                'minecraft:dragon_wall_head',
            ])
            expect(
                response.result.meshes.every(
                    (mesh) =>
                        mesh.positions.length > 0 &&
                        mesh.colors.length === mesh.positions.length &&
                        mesh.positions.every(Number.isFinite) &&
                        mesh.colors.every(Number.isFinite),
                ),
            ).toBe(true)
        }
    } finally {
        worker.terminate()
    }
})

it('cancels pending block work when switching back to frames or leaving the preview', async () => {
    const terminate = vi.fn()
    const postMessage = vi.fn()
    vi.stubGlobal(
        'Worker',
        class {
            terminate = terminate
            postMessage = postMessage
        },
    )
    try {
        const wrapper = mount(SeedMapLayoutPreview, {
            props: {
                pieces: [
                    {
                        id: 0,
                        name: 'ship',
                        rotation: 0,
                        positionKnown: true,
                        position: { x: 0, y: 0, z: 0 },
                        min: { x: 0, y: 0, z: 0 },
                        max: { x: 12, y: 23, z: 28 },
                    },
                ],
                kind: 'end-city',
                version: '26.3',
                label: (piece) => piece.name,
            },
            attachTo: document.body,
        })
        wrappers.push(wrapper)
        await vi.waitFor(
            () => expect(wrapper.get('canvas').attributes('data-ready')).toBe('true'),
            { timeout: 10000 },
        )
        await wrapper.get('[data-testid="seed-map-blocks-mode"]').trigger('click')
        expect(
            postMessage.mock.calls.filter(([message]) => message.mode !== 'chests'),
        ).toHaveLength(1)
        await wrapper.get('[data-testid="seed-map-frames-mode"]').trigger('click')
        expect(terminate).toHaveBeenCalledTimes(2)
        await wrapper.get('[data-testid="seed-map-blocks-mode"]').trigger('click')
        wrapper.unmount()
        wrappers.splice(wrappers.indexOf(wrapper), 1)
        expect(terminate).toHaveBeenCalledTimes(4)
    } finally {
        vi.unstubAllGlobals()
    }
})

it('shows camera-relative positive axes and toggles depth-independent chest highlights', async () => {
    const wrapper = mount(SeedMapLayoutPreview, {
        props: {
            pieces: [
                {
                    id: 0,
                    name: 'fortress_room',
                    rotation: 0,
                    positionKnown: true,
                    position: { x: -8, y: 60, z: -8 },
                    min: { x: -8, y: 60, z: -8 },
                    max: { x: 8, y: 68, z: 8 },
                },
            ],
            kind: 'fortress',
            version: '26.3',
            selected: 0,
            label: (piece) => piece.name,
            chests: [
                {
                    pieceId: 0,
                    position: [0, 64, 0],
                    name: 'minecraft:chest',
                    containerId: 4,
                },
            ],
            hoverLoot: {
                containerId: 4,
                result: {
                    supported: true,
                    truncated: false,
                    items: [{ id: 'minecraft:diamond', count: 3, enchantments: [] }],
                    limits: [],
                },
            },
            unknownChestHeights: 2,
        },
        attachTo: document.body,
    })
    wrappers.push(wrapper)
    ;(wrapper.element as HTMLElement).style.cssText =
        'width: 430px; background: var(--surface-2); color: var(--color-text-primary); padding: 12px'
    const canvas = wrapper.get('canvas').element as HTMLCanvasElement
    await vi.waitFor(() => expect(canvas.dataset.ready).toBe('true'), { timeout: 10000 })
    const canvasRect = canvas.getBoundingClientRect()
    canvas.dispatchEvent(
        new PointerEvent('pointermove', {
            clientX: canvasRect.left + canvasRect.width / 2,
            clientY: canvasRect.top + canvasRect.height / 2,
            bubbles: true,
        }),
    )
    await vi.waitFor(() => expect(wrapper.emitted('chestHover')?.[0]).toEqual([4]), {
        timeout: 10000,
    })
    expect(wrapper.find('[data-testid="seed-map-chest-loot-preview"]').exists()).toBe(true)
    expect(wrapper.get('[data-testid="seed-map-chest-loot-preview"]').text()).toContain('3')
    const axis = wrapper.get('[data-testid="seed-map-axis-indicator"]')
    expect(axis.attributes('aria-label')).toContain('+X east')
    for (const name of ['X', 'Y', 'Z'])
        expect(axis.find(`[data-testid="seed-map-axis-${name}"]`).exists()).toBe(true)
    expect(wrapper.get('[data-testid="seed-map-chest-xray"]').text()).toContain('(1)')
    expect(wrapper.text()).toContain('2 containers have no known Y coordinates')
    const gl = canvas.getContext('webgl2')!
    const pixels = () => {
        const value = new Uint8Array(canvas.width * canvas.height * 4)
        gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, value)
        return value
    }
    await vi.waitFor(() => expect(pixels().some((value) => value > 0)).toBe(true), {
        timeout: 10000,
    })
    const visible = pixels()
    await wrapper.get('[data-testid="seed-map-chest-xray"]').trigger('click')
    expect(canvas.dataset.chestXray).toBe('false')
    await vi.waitFor(() => expect(pixels()).not.toEqual(visible), { timeout: 10000 })
    const axisX = axis.get('[data-testid="seed-map-axis-X"] line').attributes('x2')
    const rect = canvas.getBoundingClientRect()
    canvas.dispatchEvent(
        new WheelEvent('wheel', {
            deltaY: -140,
            clientX: rect.x + rect.width / 2,
            clientY: rect.y + rect.height / 2,
            bubbles: true,
            cancelable: true,
        }),
    )
    await vi.waitFor(
        () =>
            expect(axis.get('[data-testid="seed-map-axis-X"] line').attributes('x2')).not.toBe(
                axisX,
            ),
        { timeout: 10000 },
    )
    await wrapper.get('[data-testid="seed-map-chest-xray"]').trigger('click')
    expect(canvas.dataset.chestXray).toBe('true')
})

it('recovers from a block worker failure without losing frames or the selection', async () => {
    let reply: ((event: MessageEvent) => void) | undefined
    vi.stubGlobal(
        'Worker',
        class {
            terminate = vi.fn()
            postMessage() {
                queueMicrotask(() =>
                    reply?.({ data: { type: 'error', message: 'Unavailable' } } as MessageEvent),
                )
            }
            set onmessage(handler: (event: MessageEvent) => void) {
                reply = handler
            }
        },
    )
    try {
        language.current = 'zh-CN'
        const wrapper = mount(SeedMapLayoutPreview, {
            props: {
                pieces: [
                    {
                        id: 0,
                        name: 'ship',
                        rotation: 0,
                        positionKnown: true,
                        position: { x: 0, y: 0, z: 0 },
                        min: { x: 0, y: 0, z: 0 },
                        max: { x: 12, y: 23, z: 28 },
                    },
                ],
                kind: 'end-city',
                version: '26.3',
                selected: 0,
                label: (piece) => piece.name,
            },
            attachTo: document.body,
        })
        wrappers.push(wrapper)
        await vi.waitFor(
            () => expect(wrapper.get('canvas').attributes('data-ready')).toBe('true'),
            { timeout: 10000 },
        )
        expect(wrapper.text()).toContain('真实结构')
        await wrapper.get('[data-testid="seed-map-blocks-mode"]').trigger('click')
        await vi.waitFor(() =>
            expect(wrapper.get('[role="alert"]').text()).toContain('无法加载真实结构'),
        )
        expect(wrapper.get('canvas').attributes('data-render-mode')).toBe('frames')
        expect(wrapper.props('selected')).toBe(0)
        expect(wrapper.get('[role="alert"]').text()).toContain('重试加载结构')
    } finally {
        vi.unstubAllGlobals()
    }
})

it('keeps a newer structure selection when an older calculation finishes later', async () => {
    let resolveOld!: (value: unknown) => void
    calls.details
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    resolveOld = resolve
                }),
        )
        .mockResolvedValueOnce({
            supported: true,
            pieces: [],
            containers: [],
            truncated: false,
            limits: [],
            variant: { rotation: 90, mirror: 0, start: 3 },
        })
    const wrapper = render()
    await wrapper.setProps({ query: { ...query, x: 128 } })
    await vi.waitFor(() => expect(wrapper.text()).toContain('90°'), { timeout: 10000 })
    resolveOld({
        supported: true,
        pieces: [],
        containers: [],
        truncated: false,
        limits: [],
        variant: { rotation: 180, mirror: 0, start: 1 },
    })
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(wrapper.text()).toContain('90°')
    expect(wrapper.text()).not.toContain('180°')
})

it('renders interactive 3D bounds and keeps map selection synchronized without inventing heights', async () => {
    calls.details.mockResolvedValue({
        supported: true,
        truncated: false,
        limits: ['relative_height'],
        containers: [],
        pieces: [
            {
                id: 0,
                name: 'SHPR',
                positionKnown: true,
                rotation: 0,
                position: { x: -20, y: 64, z: 0 },
                min: { x: -20, y: 64, z: 0 },
                max: { x: -10, y: 72, z: 15 },
            },
            {
                id: 1,
                name: 'SHLi',
                positionKnown: true,
                rotation: 90,
                position: { x: -10, y: 70, z: 0 },
                min: { x: -10, y: 70, z: 0 },
                max: { x: 5, y: 82, z: 10 },
            },
        ],
    })
    const wrapper = render()
    await wrapper.setProps({ kind: 'stronghold', title: 'Stronghold', selected: 1 })
    await flushPromises()
    const card = wrapper.get('[data-testid="seed-map-structure-capabilities"]')
    expect(card.get('button').attributes('aria-expanded')).toBe('false')
    expect(card.text()).toContain('3 features supported')
    expect(card.text()).toContain('4 features not supported')
    expect(card.find('[data-testid="seed-map-capability-list"]').exists()).toBe(false)
    expect(card.get('button').findAll('svg')).toHaveLength(3)
    await card.get('button').trigger('click')
    expect(card.get('button').attributes('aria-expanded')).toBe('true')
    expect(card.text()).toContain('Supported')
    expect(card.text()).toContain('Not supported')
    expect(card.get('[data-testid="seed-map-capability-list"]').text()).toContain(
        'Verified world heights',
    )
    await card.get('button').trigger('click')
    expect(card.find('[data-testid="seed-map-capability-list"]').exists()).toBe(false)
    await wrapper.findAll('[role="tab"]')[1].trigger('click')
    const canvas = wrapper.get('canvas').element as HTMLCanvasElement
    await vi.waitFor(() => expect(wrapper.get('canvas').attributes('data-ready')).toBe('true'), {
        timeout: 10000,
    })
    const gl = canvas.getContext('webgl2')!
    const pixels = () => {
        const data = new Uint8Array(canvas.width * canvas.height * 4)
        gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, data)
        return data
    }
    await vi.waitFor(() => expect(pixels().some((value) => value > 0)).toBe(true), {
        timeout: 10000,
    })
    expect(wrapper.get('[data-piece-id="1"]').attributes('aria-pressed')).toBe('true')
    const before = pixels()
    canvas.dispatchEvent(
        new WheelEvent('wheel', {
            deltaY: -200,
            clientX: 200,
            clientY: 400,
            bubbles: true,
            cancelable: true,
        }),
    )
    await vi.waitFor(() => expect(pixels()).not.toEqual(before), { timeout: 10000 })
    const zoomed = pixels()
    await userEvent.dragAndDrop(
        canvas,
        wrapper.get('[data-testid="seed-map-layout-preview"] strong').element,
    )
    await vi.waitFor(() => expect(pixels()).not.toEqual(zoomed), { timeout: 10000 })
    await wrapper.get('[data-piece-id="0"]').trigger('click')
    expect(wrapper.emitted('focus')?.at(-1)?.[0]).toMatchObject({ id: 0 })
    expect(wrapper.text()).toContain('Vertical relationships are relative')
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/seed-map-layout-3d.png`,
        })
})

it('retains a selectable horizontal fallback after a WebGL context loss', async () => {
    calls.details.mockResolvedValue({
        supported: true,
        truncated: false,
        limits: ['height_placeholder'],
        containers: [],
        pieces: [
            {
                id: 0,
                name: 'SHPR',
                positionKnown: true,
                rotation: 0,
                position: { x: 0, y: 64, z: 0 },
                min: { x: 0, y: 64, z: 0 },
                max: { x: 10, y: 72, z: 15 },
            },
        ],
    })
    const wrapper = render()
    await wrapper.setProps({ kind: 'stronghold' })
    await flushPromises()
    await wrapper.findAll('[role="tab"]')[1].trigger('click')
    await vi.waitFor(() => expect(wrapper.get('canvas').attributes('data-ready')).toBe('true'), {
        timeout: 10000,
    })
    await wrapper.get('canvas').trigger('webglcontextlost')
    expect(wrapper.text()).toContain('3D rendering is unavailable')
    await wrapper.get('svg[aria-label] rect').trigger('click')
    expect(wrapper.emitted('focus')?.at(-1)?.[0]).toMatchObject({ id: 0 })
    expect(wrapper.text()).toContain('Retry 3D preview')
})

it.each(['en-US', 'zh-CN'])(
    'summarizes unavailable capabilities for a supported detail response in %s and collapses for a new structure',
    async (locale) => {
        language.current = locale
        calls.details.mockResolvedValue({
            supported: true,
            truncated: false,
            pieces: [],
            containers: [],
            limits: ['layout_unavailable'],
        })
        const wrapper = render()
        await flushPromises()
        const card = () => wrapper.get('[data-testid="seed-map-structure-capabilities"]')
        expect(card().get('button').text()).toContain(
            locale === 'zh-CN' ? '支持 0 个功能' : '0 features supported',
        )
        expect(card().get('button').text()).toContain(
            locale === 'zh-CN' ? '不支持 7 个功能' : '7 features not supported',
        )
        await card().get('button').trigger('click')
        expect(card().find('[data-testid="seed-map-capability-list"]').exists()).toBe(true)
        await wrapper.setProps({ query: { ...query, x: 256 } })
        await flushPromises()
        expect(card().get('button').attributes('aria-expanded')).toBe('false')
        expect(card().find('[data-testid="seed-map-capability-list"]').exists()).toBe(false)
    },
)

it.each(['en-US', 'zh-CN'])(
    'shows a compact unavailable-details state in %s while preserving map actions',
    async (locale) => {
        language.current = locale
        calls.details.mockResolvedValue({
            supported: false,
            truncated: false,
            pieces: [],
            containers: [],
            limits: ['layout_unavailable'],
        })
        const wrapper = render('<button data-testid="map-action">Map action</button>')
        await flushPromises()
        expect(wrapper.get('[data-testid="seed-map-details-unavailable"]').text()).toBe(
            locale === 'zh-CN'
                ? '引擎尚未实现此版本中该结构的更多详情。'
                : 'The engine has no additional details for this structure in this version.',
        )
        expect(wrapper.find('[role="tablist"]').exists()).toBe(false)
        expect(wrapper.find('[data-testid="seed-map-structure-capabilities"]').exists()).toBe(false)
        expect(wrapper.find('[data-onboarding-id="seed-map-container-loot"]').exists()).toBe(false)
        expect(wrapper.find('details').exists()).toBe(false)
        expect(wrapper.find('canvas').exists()).toBe(false)
        expect(wrapper.text()).not.toContain('0 containers')
        expect(wrapper.get('[data-testid="map-action"]').exists()).toBe(true)
        expect(wrapper.text()).toContain('26.3 · 0, 0')
        expect(calls.loot).not.toHaveBeenCalled()
        await wrapper.get('button[aria-label]').trigger('click')
        expect(wrapper.emitted('close')).toHaveLength(1)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: wrapper.element,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/seed-map-unavailable-${locale}.png`,
            })
    },
)

it('clears a pending loot view when selecting an unsupported structure and restores overview for supported details', async () => {
    const supported = {
        supported: true,
        truncated: false,
        pieces: [],
        containers: [{ id: 0, piece: 0, x: 0, z: 0, table: 'chests/stronghold_library' }],
        limits: [],
    }
    calls.details.mockResolvedValueOnce(supported)
    let resolveLoot!: (value: unknown) => void
    calls.loot.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                resolveLoot = resolve
            }),
    )
    const wrapper = render()
    await flushPromises()
    await wrapper.findAll('[role="tab"]')[2].trigger('click')
    await vi.waitFor(() => expect(calls.loot).toHaveBeenCalledOnce())
    calls.details.mockResolvedValueOnce({
        supported: false,
        truncated: false,
        pieces: [],
        containers: [],
        limits: ['layout_unavailable'],
    })
    await wrapper.setProps({ query: { ...query, x: 128 } })
    await flushPromises()
    expect(wrapper.find('[role="tablist"]').exists()).toBe(false)
    expect(
        wrapper.get('[data-onboarding-id="seed-map-structure-details"]').attributes('aria-busy'),
    ).toBe('false')
    resolveLoot({
        supported: true,
        items: [{ id: 'minecraft:diamond', count: 64, enchantments: [] }],
        limits: [],
    })
    await flushPromises()
    expect(wrapper.text()).not.toContain('Diamond')
    calls.details.mockResolvedValueOnce(supported)
    await wrapper.setProps({ query: { ...query, x: 256 } })
    await flushPromises()
    expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toBe('Overview')
    expect(calls.loot).toHaveBeenCalledOnce()
    expect(wrapper.find('[data-testid="seed-map-details-unavailable"]').exists()).toBe(false)
})

it('loads loot only after opening its tab and reports unavailable piece positions', async () => {
    calls.details.mockResolvedValue({
        supported: true,
        pieces: [
            {
                id: 0,
                name: 'tent',
                positionKnown: false,
                position: { x: 0, y: 0, z: 0 },
                rotation: 0,
            },
        ],
        containers: [{ id: 0, piece: 0, x: 232, z: -392, table: 'barrels/abandoned_camp_barrel' }],
        truncated: false,
        limits: ['piece_positions_unavailable'],
    })
    calls.loot.mockResolvedValue({
        supported: true,
        items: [],
        unresolvedTables: ['missing/table'],
        limits: [],
    })
    const wrapper = render()
    await vi.waitFor(() => expect(wrapper.text()).toContain('no piece positions'))
    expect(calls.loot).not.toHaveBeenCalled()
    await wrapper
        .findAll('[role="tab"]')
        .find((tab) => tab.text() === 'Layout')!
        .trigger('click')
    expect(wrapper.find('svg[aria-label]').exists()).toBe(false)
    await wrapper
        .findAll('[role="tab"]')
        .find((tab) => tab.text() === 'Loot')!
        .trigger('click')
    await vi.waitFor(() => expect(calls.loot).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(wrapper.text()).toContain('Unrecognized loot table'))
    expect(wrapper.text()).not.toContain('missing/table')
    await wrapper.get('details summary').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('missing/table'))
})

it('shows all containers by default and merges identical items within each container', async () => {
    calls.details.mockResolvedValue({
        supported: true,
        truncated: false,
        limits: [],
        pieces: [],
        containers: [
            { id: 2, piece: 0, x: 10, z: 20, table: 'chests/end_city_treasure' },
            { id: 5, piece: 1, x: 30, z: 40, table: 'chests/end_city_treasure' },
        ],
    })
    calls.loot.mockImplementation((_query, container: number) =>
        Promise.resolve({
            supported: true,
            truncated: false,
            limits: [],
            items:
                container === 2
                    ? [
                          { id: 'minecraft:diamond', count: 2, enchantments: [] },
                          { id: 'minecraft:diamond', count: 3, enchantments: [] },
                          {
                              id: 'minecraft:diamond',
                              count: 1,
                              enchantments: [{ id: 'minecraft:fortune', level: 1 }],
                          },
                      ]
                    : [{ id: 'minecraft:emerald', count: 4, enchantments: [] }],
        }),
    )
    const wrapper = render()
    await flushPromises()
    await wrapper.findAll('[role="tab"]')[2].trigger('click')
    await vi.waitFor(() => expect(calls.loot).toHaveBeenCalledTimes(2))
    await vi.waitFor(() => expect(wrapper.text()).toContain('Emerald ×4'))
    expect(wrapper.get('[data-combobox]').text()).toContain('All loot')
    expect(wrapper.text()).toContain('Diamond ×5')
    expect(wrapper.text()).toContain('Diamond ×1')
    expect(wrapper.text()).toContain('From End city chest 1 · 10, 20')
    expect(wrapper.text()).toContain('From End city chest 2 · 30, 40')
    expect(wrapper.text()).not.toContain('>')
})

it.each(['en-US', 'zh-CN'])(
    'uses translated names for layouts, loot and effects in %s while retaining raw identifiers on demand',
    async (locale) => {
        language.current = locale
        calls.details.mockResolvedValue({
            supported: true,
            truncated: false,
            limits: [],
            variant: { start: 255, rotation: 90, mirror: 0 },
            pieces: [
                {
                    id: 0,
                    name: 'SHPR',
                    positionKnown: true,
                    position: { x: 12, y: 64, z: 34 },
                    rotation: 90,
                },
            ],
            containers: [{ id: 0, piece: 0, x: 13, z: 35, table: 'chests/stronghold_library' }],
        })
        calls.loot.mockResolvedValue({
            supported: true,
            truncated: false,
            limits: [],
            items: [
                {
                    id: 'minecraft:diamond',
                    count: 2,
                    enchantments: [{ id: 'minecraft:fortune', level: 3 }],
                    effect: 'minecraft:regeneration',
                    duration: 160,
                },
            ],
        })
        const wrapper = render()
        await wrapper.setProps({ kind: 'stronghold' })
        await flushPromises()
        expect(wrapper.text()).not.toContain('255')
        await wrapper.findAll('[role="tab"]')[1].trigger('click')
        expect(wrapper.text()).toContain(locale === 'zh-CN' ? '末地传送门房间' : 'End portal room')
        expect(wrapper.text()).not.toContain('SHPR')
        expect(wrapper.get('[data-piece-id="0"]').text()).toContain(
            locale === 'zh-CN' ? '末地传送门房间' : 'End portal room',
        )
        await wrapper.get('[data-piece-id="0"]').trigger('click')
        expect(wrapper.emitted('focus')?.[0]?.[0]).toMatchObject({
            name: 'SHPR',
            position: { x: 12, y: 64, z: 34 },
        })
        await wrapper.findAll('[role="tab"]')[2].trigger('click')
        await vi.waitFor(() => expect(calls.loot).toHaveBeenCalled())
        await flushPromises()
        expect(wrapper.text()).toContain(
            locale === 'zh-CN' ? '要塞图书馆宝箱' : 'Stronghold library chest',
        )
        expect(wrapper.text()).toContain(locale === 'zh-CN' ? '钻石 ×2' : 'Diamond ×2')
        expect(wrapper.text()).toContain(locale === 'zh-CN' ? '时运 III' : 'Fortune III')
        expect(wrapper.text()).toContain(
            locale === 'zh-CN' ? '生命恢复 · 0:08' : 'Regeneration · 0:08',
        )
        expect(wrapper.text()).not.toContain('minecraft:fortune')
        await wrapper.get('details summary').trigger('click')
        await vi.waitFor(() => expect(wrapper.text()).toContain('SHPR'))
        expect(wrapper.text()).toContain('minecraft:fortune')
    },
)

it('opens the matching loot container when a highlighted chest is clicked', async () => {
    calls.details.mockResolvedValue({
        supported: true,
        truncated: false,
        limits: [],
        pieces: [
            {
                id: 0,
                name: 'NeSCLT',
                positionKnown: true,
                rotation: 0,
                position: { x: 12, y: 50, z: 20 },
                min: { x: 12, y: 50, z: 20 },
                max: { x: 18, y: 55, z: 26 },
            },
        ],
        containers: [{ id: 7, piece: 0, x: 14, z: 22, table: 'chests/nether_bridge' }],
    })
    calls.loot.mockResolvedValue({
        supported: true,
        truncated: false,
        items: [{ id: 'minecraft:diamond', count: 3, enchantments: [] }],
        limits: [],
    })
    const wrapper = render()
    await wrapper.setProps({ kind: 'fortress' })
    await flushPromises()
    await wrapper.findAll('[role="tab"]')[1].trigger('click')
    const preview = wrapper.findComponent(SeedMapLayoutPreview)
    expect(preview.exists()).toBe(true)
    preview.vm.$emit('chestClick', 7)
    await vi.waitFor(() =>
        expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toBe('Loot'),
    )
    await vi.waitFor(() => expect(wrapper.text()).toContain('Diamond ×3'))
    expect(wrapper.get('[data-combobox]').text()).toContain('Nether fortress chest 1 · 14, 22')
    expect(calls.loot).toHaveBeenCalledWith(expect.objectContaining({ x: 0, z: 0 }), 7)
})

it('names numbered portal variants and mirror state and resolves camp placeholders', async () => {
    language.current = 'zh-CN'
    calls.details.mockResolvedValue({
        supported: true,
        truncated: false,
        limits: [],
        pieces: [],
        containers: [],
        variant: { start: 3, giant: true, rotation: 90, mirror: 1 },
    })
    const wrapper = render()
    await wrapper.setProps({ kind: 'ruined-portal' })
    await flushPromises()
    expect(wrapper.text()).toContain('巨型废弃传送门 3')
    expect(wrapper.text()).toContain('giant_ruined_portal_3')
    expect(wrapper.text()).toContain('已镜像')
    calls.details.mockResolvedValue({
        supported: true,
        truncated: false,
        limits: ['piece_positions_unavailable'],
        containers: [],
        variant: { start: 7, biome: 188, rotation: 0, mirror: 0 },
        pieces: [
            {
                id: 0,
                name: 'abandoned_camp/tent/%s/tent_%s_7',
                positionKnown: false,
                position: { x: 0, y: 0, z: 0 },
                rotation: 0,
            },
        ],
    })
    await wrapper.setProps({ kind: 'abandoned-camp', query: { ...query, x: 64 } })
    await flushPromises()
    await wrapper.findAll('[role="tab"]')[1].trigger('click')
    expect(wrapper.text()).toContain('帐篷 7')
    expect(wrapper.text()).not.toContain('%s')
    expect(wrapper.text()).not.toContain('无镜像')
})
