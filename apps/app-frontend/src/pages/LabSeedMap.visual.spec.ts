import '@/test/visual-styles.scss'

import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { page, userEvent } from 'vitest/browser'
import { defineComponent, h, Suspense, Transition, type VNode } from 'vue'
import { createMemoryHistory, createRouter, RouterView } from 'vue-router'

import type {
    SeedMapLoot,
    SeedMapStructureDetails,
    SeedMapStructurePiece,
} from '@/lab/seed-map/advanced'
import type { SeedMapArea } from '@/lab/seed-map/area-analysis'
import type { SeedMapOreScanChunk, SeedMapTile, SeedMapTileRequest } from '@/lab/seed-map/backend'
import type { SeedMapFeature } from '@/lab/seed-map/features'
import { layoutSeedMapMarkers } from '@/lab/seed-map/marker-rendering'
import { createDefaultSeedMapWorkspace, type SeedMapWorkspace } from '@/lab/seed-map/workspace'

import LabSeedMap from './LabSeedMap.vue'

const backend = vi.hoisted(() => ({
    tiles: [] as { request: SeedMapTileRequest; resolve: (tile: SeedMapTile) => void }[],
    spawns: [] as {
        seed: string
        resolve: (spawn: { x: number; z: number; approximate: boolean }) => void
    }[],
    features: vi.fn(async (): Promise<SeedMapFeature[]> => []),
    loot: vi.fn(async (): Promise<SeedMapLoot> => ({
        supported: true,
        truncated: false,
        items: [],
        limits: [],
    })),
    ores: vi.fn(
        async (_request: {
            ores: string[]
            chunks: number[]
        }): Promise<SeedMapOreScanChunk[]> => [],
    ),
    details: vi.fn(),
}))

vi.mock('@/lab/seed-map/advanced', () => ({
    getSeedMapStructureDetails: backend.details,
    getSeedMapContainerLoot: backend.loot,
}))

vi.mock('vue-router', async (importOriginal) => ({
    ...(await importOriginal<typeof import('vue-router')>()),
    useRoute: () => ({ query: {} }),
}))
vi.mock('@/lab/recipe-generator/item-images', () => ({
    loadMinecraftItemImage: () => Promise.resolve({ src: '' }),
}))
vi.mock('@/components/lab/seed-map/SeedMapBiomePicker.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/seed-map/SeedMapCopyrightModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/seed-map/SeedMapWorldImportModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/seed-map/SeedMapUndergroundPreview.vue', () => ({
    default: {
        props: ['query'],
        emits: ['close'],
        setup:
            (_, { emit }) =>
            () =>
                h('div', { 'data-testid': 'direct-underground' }, [
                    h('button', { onClick: () => emit('close') }, 'Back to map'),
                ]),
    },
}))
vi.mock('@/lab/seed-map/backend.ts', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@/lab/seed-map/backend')>()),
    getSeedMapProfiles: async () => [
        {
            edition: 'java',
            version: '26.3',
            available: true,
            dimensions: ['overworld', 'nether', 'end'],
            ores: true,
            undergroundMasks: [7, 3, 1],
        },
    ],
    renderSeedMapTile: (request: SeedMapTileRequest) =>
        new Promise((resolve) => backend.tiles.push({ request, resolve })),
    getSeedMapSpawn: (seed: string) =>
        new Promise((resolve) => backend.spawns.push({ seed, resolve })),
    findSeedMapFeatures: backend.features,
    scanSeedMapOres: backend.ores,
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Admonition: control,
        Checkbox: control,
        Combobox: control,
        PopoutMenu: control,
        Slider: control,
        Toggle: control,
        Button: (await import('../../../../packages/ui/src/components/base/buttons/Button.vue'))
            .default,
        StyledInput: defineComponent({
            props: ['modelValue'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn(), addNotification: vi.fn() }),
        useRelativeTime: () => () => '',
        useVIntl: () => ({
            locale: ref('en-US'),
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

type MapVm = {
    workspace: SeedMapWorkspace
    resizeCanvas: () => void
    scheduleRefresh: (delay?: number) => void
    drawMap: () => void
    features: SeedMapFeature[]
    selection: { x: number; z: number; feature?: SeedMapFeature } | null
    focusedPiece: SeedMapStructurePiece | null
    selectAt: (x: number, y: number) => void
    worldToScreen: (x: number, z: number) => { x: number; y: number }
    addMarker: () => void
    markerName: string
    dimensionMarkers: SeedMapWorkspace['markers']
    selectedArea: SeedMapArea | null
    markerDraftOpen: boolean
}
let wrapper: VueWrapper | undefined
let closed: ReturnType<typeof vi.spyOn>[] = []

it('validates numeric and cleared heights and cancels a pending batch when area analysis closes', async () => {
    const { vm } = await openMap()
    vm.selectedArea = { minX: 0, minZ: 0, maxX: 15, maxZ: 15 }
    await flushPromises()
    const panel = wrapper!.getComponent({ name: 'SeedMapAreaAnalysis' })
    const state = panel.vm as unknown as { minY: number | undefined }
    state.minY = undefined
    await flushPromises()
    expect(
        panel
            .findAll('button')
            .find((button) => button.text() === 'Count mineral blocks')!
            .attributes('disabled'),
    ).toBeDefined()
    state.minY = -64
    await flushPromises()
    let finish: ((value: SeedMapOreScanChunk[]) => void) | undefined
    backend.ores.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    await userEvent.click(page.getByRole('button', { name: 'Count mineral blocks', exact: true }))
    await vi.waitFor(() => expect(panel.find('progress').exists()).toBe(true))
    expect(Number(panel.get('progress').attributes('max'))).toBeGreaterThan(0)
    const calls = backend.ores.mock.calls.length
    await userEvent.click(page.getByRole('button', { name: 'Close area analysis', exact: true }))
    finish!([])
    await flushPromises()
    expect(backend.ores.mock.calls).toHaveLength(calls)
    expect(wrapper!.find('[data-testid="seed-map-area-analysis"]').exists()).toBe(false)
})

it('searches supported loot outside visible layers and locates a chest while retaining area results', async () => {
    const { vm } = await openMap()
    backend.features.mockResolvedValue([{ kind: 'outpost', x: 100, z: 100, approximate: false }])
    backend.details.mockResolvedValue({
        supported: true,
        truncated: false,
        pieces: [],
        containers: [{ id: 0, piece: 0, x: 5, z: 7 }],
        limits: ['container_positions_approximate'],
    })
    backend.loot.mockResolvedValue({
        supported: true,
        truncated: false,
        items: [{ id: 'minecraft:diamond', count: 3, enchantments: [] }],
        limits: [],
    })
    vm.workspace.visibleFeatures = []
    vm.selectedArea = { minX: 0, minZ: 0, maxX: 15, maxZ: 15 }
    await flushPromises()
    await userEvent.click(page.getByRole('tab', { name: 'Find loot', exact: true }))
    const panel = wrapper!.get('[data-testid="seed-map-area-analysis"]')
    await panel.get('input').setValue('diamond')
    await userEvent.click(page.getByRole('button', { name: 'Search container loot', exact: true }))
    await vi.waitFor(() => expect(panel.text()).toContain('Diamond ×3'))
    expect(panel.text()).toContain('Chest coordinates and inclusion in this area are approximate')
    await userEvent.click(page.getByRole('button', { name: 'Locate chest', exact: true }))
    expect(vm.workspace.center).toEqual({ x: 5, z: 7 })
    expect(vm.selectedArea).not.toBeNull()
    expect(panel.text()).toContain('Diamond ×3')
})

it('draws a live deep-purple right-drag rectangle without panning or opening a native context menu', async () => {
    const { vm, canvas } = await openMap()
    const capture = vi.spyOn(canvas, 'setPointerCapture').mockImplementation(() => {})
    const release = vi.spyOn(canvas, 'releasePointerCapture').mockImplementation(() => {})
    try {
        const center = { ...vm.workspace.center }
        const rect = canvas.getBoundingClientRect()
        const pointer = (type: string, x: number, y: number) =>
            canvas.dispatchEvent(
                new PointerEvent(type, {
                    button: 2,
                    buttons: type === 'pointerup' ? 0 : 2,
                    pointerId: 22,
                    clientX: rect.left + x,
                    clientY: rect.top + y,
                    bubbles: true,
                    cancelable: true,
                }),
            )
        pointer('pointerdown', 120, 80)
        pointer('pointermove', 180, 140)
        await flushPromises()
        const box = () => wrapper!.get('[data-testid="seed-map-area-selection"]')
        expect(box().classes()).toContain('border-purple-800')
        expect(getComputedStyle(box().element).borderTopWidth).toBe('2px')
        const width = (box().element as HTMLElement).style.width
        pointer('pointermove', 240, 180)
        await flushPromises()
        expect((box().element as HTMLElement).style.width).not.toBe(width)
        expect(vm.workspace.center).toEqual(center)
        pointer('pointerup', 240, 180)
        await flushPromises()
        expect(wrapper!.find('[data-testid="seed-map-area-analysis"]').exists()).toBe(true)
        const menu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
        wrapper!.get('[data-testid="seed-map-area-analysis"]').element.dispatchEvent(menu)
        expect(menu.defaultPrevented).toBe(true)
        expect(vm.markerDraftOpen).toBe(false)
        expect(vm.selectedArea).not.toBeNull()
        const inputMenu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
        wrapper!
            .get('[data-testid="seed-map-area-analysis"] input')
            .element.dispatchEvent(inputMenu)
        expect(inputMenu.defaultPrevented).toBe(false)
        pointer('pointerdown', 100, 100)
        pointer('pointerup', 100, 100)
        const markerMenu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
        canvas.dispatchEvent(markerMenu)
        expect(markerMenu.defaultPrevented).toBe(true)
        expect(vm.markerDraftOpen).toBe(true)
        vm.workspace.dimension = 'nether'
        await flushPromises()
        expect(wrapper!.find('[data-testid="seed-map-area-selection"]').exists()).toBe(false)
        expect(wrapper!.find('[data-testid="seed-map-area-analysis"]').exists()).toBe(false)
    } finally {
        capture.mockRestore()
        release.mockRestore()
    }
})

it('counts the entire selected area and omits zero mineral rows independently of map filters', async () => {
    const { vm } = await openMap()
    backend.ores.mockImplementation(async (request) => [
        {
            cx: 0,
            cz: 0,
            truncated: false,
            hits:
                request.ores[0] === 'diamond'
                    ? [
                          {
                              ore: 'diamond',
                              x: 0,
                              y: -40,
                              z: 0,
                              yMin: -40,
                              yMax: -40,
                              status: 'terrain_unverified',
                          },
                          {
                              ore: 'diamond',
                              x: 0,
                              y: 4,
                              z: 0,
                              yMin: 4,
                              yMax: 4,
                              status: 'terrain_unverified',
                          },
                      ]
                    : [],
        },
    ])
    vm.workspace.selectedOres = ['iron']
    vm.selectedArea = { minX: 0, minZ: 0, maxX: 15, maxZ: 15 }
    await flushPromises()
    await userEvent.click(page.getByRole('button', { name: 'Count mineral blocks', exact: true }))
    const panel = wrapper!.get('[data-testid="seed-map-area-analysis"]')
    await vi.waitFor(() => expect(panel.attributes('aria-busy')).toBe('false'))
    expect(panel.findAll('tbody tr')).toHaveLength(1)
    expect(panel.get('tbody tr').text()).toContain('Diamond')
    expect(panel.get('tbody tr').text()).toContain('2')
    expect(backend.ores.mock.calls.some(([request]) => request.ores.includes('diamond'))).toBe(true)
})

it('anchors a plain location selection with a crosshair that follows the map and closes with its card', async () => {
    const { vm } = await openMap()
    vm.workspace.showSpawn = false
    vm.selectAt(180, 140)
    await flushPromises()
    const crosshair = () => wrapper!.find('[data-testid="seed-map-selection-point"]')
    expect(crosshair().findAll('svg')).toHaveLength(2)
    const position = vm.worldToScreen(vm.selection!.x, vm.selection!.z)
    expect((crosshair().element as HTMLElement).style.left).toBe(`${position.x}px`)
    const before = (crosshair().element as HTMLElement).style.left
    vm.workspace.center.x += 64
    await flushPromises()
    expect((crosshair().element as HTMLElement).style.left).not.toBe(before)
    vm.selection = null
    await flushPromises()
    expect(crosshair().exists()).toBe(false)
})

it('enlarges POI images and their hit targets only when closer than four blocks per pixel', async () => {
    const { vm, canvas } = await openMap()
    vm.workspace.showSpawn = false
    vm.workspace.visibleFeatures = ['village']
    vm.features = [{ kind: 'village', x: 0, z: 0, approximate: true }]
    const drawImage = vi.spyOn(canvas.getContext('2d')!, 'drawImage')
    try {
        for (const [zoom, expectedSize, hit] of [
            [1, 28, false],
            [0.5, 42, true],
            [1.5, 28, false],
        ] as const) {
            vm.selection = null
            vm.workspace.zoom = zoom
            await flushPromises()
            await vi.waitFor(() => {
                drawImage.mockClear()
                vm.drawMap()
                expect(
                    drawImage.mock.calls.some(
                        (call) =>
                            call[0] instanceof HTMLImageElement &&
                            call[3] === expectedSize &&
                            call[4] === expectedSize,
                    ),
                ).toBe(true)
            })
            const point = vm.worldToScreen(0, 0)
            vm.selectAt(point.x + 18, point.y)
            expect(!!vm.selection?.feature).toBe(hit)
        }
    } finally {
        drawImage.mockRestore()
    }
})

it('isolates saved marker rendering and selection by dimension and preserves their creation dimension', async () => {
    const { vm } = await openMap()
    vm.workspace.showSpawn = false
    vm.selectAt(180, 140)
    vm.markerName = 'Overworld camp'
    vm.addMarker()
    const overworld = vm.workspace.markers[0]
    expect(overworld.dimension).toBe('overworld')
    vm.workspace.dimension = 'nether'
    await flushPromises()
    expect(vm.dimensionMarkers).toEqual([])
    const point = vm.worldToScreen(overworld.x, overworld.z)
    vm.selectAt(point.x, point.y)
    expect(vm.selection).not.toHaveProperty('markerId')
    vm.markerName = 'Nether camp'
    vm.addMarker()
    expect(vm.workspace.markers[1].dimension).toBe('nether')
    expect(vm.dimensionMarkers.map((marker) => marker.name)).toEqual(['Nether camp'])
    vm.workspace.dimension = 'end'
    await flushPromises()
    expect(vm.dimensionMarkers).toEqual([])
    vm.workspace.dimension = 'overworld'
    await flushPromises()
    expect(vm.dimensionMarkers).toEqual([overworld])
    vm.selectAt(point.x, point.y)
    expect(vm.selection).toHaveProperty('markerId', overworld.id)
})

it('draws saved marker names above the pins by default and selects their visible names at both icon scales', async () => {
    const { vm, canvas } = await openMap()
    vm.workspace.showSpawn = false
    vm.workspace.visibleFeatures = []
    const context = canvas.getContext('2d')!
    const text = vi.spyOn(context, 'fillText')
    const curve = vi.spyOn(context, 'bezierCurveTo')
    vm.workspace.markers = [
        { id: 'home', name: '基地 Home', x: 0, z: 0, color: '#22C55E', dimension: 'overworld' },
    ]
    try {
        for (const zoom of [1, 0.5]) {
            vm.workspace.zoom = zoom
            vm.selection = null
            await flushPromises()
            text.mockClear()
            curve.mockClear()
            vm.drawMap()
            const point = vm.worldToScreen(0, 0)
            const name = text.mock.calls.find(([value]) => value === '基地 Home')
            expect(name).toBeDefined()
            expect(name![1]).toBe(point.x)
            expect(name![2]).toBeLessThan(point.y - 30 * (zoom === 1 ? 1 : 1.5))
            expect(curve.mock.calls.length).toBe(4)
            vm.selectAt(name![1], name![2])
            expect(vm.selection).toHaveProperty('markerId', 'home')
            vm.selection = null
            vm.selectAt(point.x, point.y - 25 * (zoom === 1 ? 1 : 1.5))
            expect(vm.selection).toHaveProperty('markerId', 'home')
        }
        vm.workspace.dimension = 'nether'
        await flushPromises()
        text.mockClear()
        vm.drawMap()
        expect(text.mock.calls.some(([value]) => value === '基地 Home')).toBe(false)
    } finally {
        text.mockRestore()
        curve.mockRestore()
    }
})

it('keeps selected marker names above overlapping markers and clips long names without changing saved data', async () => {
    const { vm, canvas } = await openMap()
    vm.workspace.showSpawn = false
    vm.workspace.visibleFeatures = []
    vm.workspace.zoom = 1
    vm.workspace.markers = [
        { id: 'home', name: '基地 Home', x: 0, z: 0, color: '#22C55E', dimension: 'overworld' },
        { id: 'mine', name: '矿场 Mine', x: 0, z: 0, color: '#EF4444', dimension: 'overworld' },
    ]
    vm.selection = { x: 0, z: 0, markerId: 'home' }
    await flushPromises()
    const context = canvas.getContext('2d')!
    const text = vi.spyOn(context, 'fillText')
    try {
        vm.drawMap()
        expect(text.mock.calls.some(([value]) => value === '基地 Home')).toBe(true)
        expect(text.mock.calls.some(([value]) => value === '矿场 Mine')).toBe(false)
        const point = vm.worldToScreen(0, 0)
        vm.selectAt(point.x, point.y - 15)
        expect(vm.selection).toHaveProperty('markerId', 'home')
        const longName = '基地😀'.repeat(40)
        vm.workspace.markers = [{ ...vm.workspace.markers[0], name: longName }]
        await flushPromises()
        text.mockClear()
        vm.drawMap()
        expect(text.mock.calls.some(([value]) => value.endsWith('…'))).toBe(true)
        expect(vm.workspace.markers[0].name).toBe(longName)
        const placements = layoutSeedMapMarkers(
            context,
            vm.dimensionMarkers,
            vm.worldToScreen,
            { width: 512, height: 256 },
            1,
            'home',
        )
        expect(placements[0].label!.width).toBeLessThanOrEqual(168)
    } finally {
        text.mockRestore()
    }
})

beforeEach(() => {
    localStorage.removeItem('axolotl.lab.seed-map.v1')
    localStorage.removeItem('axolotl.lab.seed-map.history.v1')
    backend.tiles = []
    backend.spawns = []
    backend.features.mockReset().mockResolvedValue([])
    backend.loot
        .mockReset()
        .mockResolvedValue({ supported: true, truncated: false, items: [], limits: [] })
    backend.ores.mockReset().mockResolvedValue([])
    backend.details.mockReset().mockResolvedValue({
        supported: false,
        truncated: false,
        pieces: [],
        containers: [],
        limits: ['layout_unavailable'],
    })
})

it.each([-2, 0, 0.5])(
    'opens a saved workspace at zoom %s without reading uninitialized layout dependencies',
    async (zoom) => {
        localStorage.setItem(
            'axolotl.lab.seed-map.v1',
            JSON.stringify({ ...createDefaultSeedMapWorkspace(), zoom }),
        )
        const { vm } = await openMap()
        expect(vm.workspace.zoom).toBe(zoom)
        expect(wrapper!.get('canvas').exists()).toBe(true)
    },
)

it('can leave a close-zoom map with pending work and navigate back through the app transition shell', async () => {
    localStorage.setItem(
        'axolotl.lab.seed-map.v1',
        JSON.stringify({ ...createDefaultSeedMapWorkspace(), zoom: 0 }),
    )
    const errors: unknown[] = []
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/lab/seed-map', component: LabSeedMap },
            {
                path: '/other',
                component: defineComponent({
                    render: () => h('div', { 'data-testid': 'other-page' }, 'Other page'),
                }),
            },
        ],
    })
    router.onError((error) => errors.push(error))
    await router.push('/lab/seed-map')
    await router.isReady()
    const shell = defineComponent({
        render: () =>
            h(
                RouterView,
                {},
                {
                    default: ({
                        Component,
                        route,
                    }: {
                        Component: VNode
                        route: { path: string }
                    }) =>
                        h(
                            Transition,
                            { css: false, appear: true },
                            {
                                default: () =>
                                    h('div', { key: route.path }, [
                                        h(Suspense, {}, { default: () => Component }),
                                    ]),
                            },
                        ),
                },
            ),
    })
    wrapper = mount(shell, {
        attachTo: document.body,
        global: {
            plugins: [router],
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    await vi.waitFor(() => expect(wrapper!.find('canvas').exists()).toBe(true))
    await vi.waitFor(() => expect(backend.tiles.length).toBeGreaterThan(0))
    await router.push('/other')
    await flushPromises()
    expect(wrapper.get('[data-testid="other-page"]').text()).toBe('Other page')
    for (const job of backend.tiles.splice(0)) await complete(job, '#00ff00')
    await router.push('/lab/seed-map')
    await flushPromises()
    expect(wrapper.find('canvas').exists()).toBe(true)
    await router.push('/other')
    await flushPromises()
    expect(wrapper.find('[data-testid="other-page"]').exists()).toBe(true)
    expect(errors).toEqual([])
})

it('automatically paints visible layouts without a selection, shares requests with details and resizes the split', async () => {
    await page.viewport(1280, 1100)
    document.documentElement.classList.add('dark-mode')
    const result: SeedMapStructureDetails = {
        supported: true,
        truncated: false,
        containers: [],
        limits: ['relative_height'],
        pieces: [
            {
                id: 0,
                name: 'SHPR',
                rotation: 0,
                positionKnown: true,
                position: { x: -40, y: 64, z: -20 },
                min: { x: -40, y: 64, z: -20 },
                max: { x: 40, y: 75, z: 20 },
            },
        ],
    }
    backend.details.mockResolvedValue(result)
    const { vm, canvas } = await openMap()
    ;(wrapper!.element as HTMLElement).style.cssText =
        'width: 1200px; height: 1000px; background: var(--surface-1); color: var(--color-text-primary)'
    canvas.style.width = ''
    canvas.style.height = ''
    vm.resizeCanvas()
    vm.workspace.zoom = 0
    vm.workspace.showGrid = false
    vm.workspace.showSpawn = false
    vm.workspace.visibleFeatures = ['stronghold']
    vm.features = [{ kind: 'stronghold', x: 0, z: 0, approximate: true }]
    await flushPromises()
    await vi.waitFor(() => expect(backend.details).toHaveBeenCalledOnce())
    expect(vm.selection).toBeNull()
    const badge = wrapper!.get('[data-testid="seed-map-structure-badge"]')
    expect(badge.text()).toBe('Stronghold')
    expect(badge.element.getBoundingClientRect().height).toBeGreaterThanOrEqual(32)
    await badge.trigger('click')
    await flushPromises()
    expect(vm.selection?.feature?.kind).toBe('stronghold')
    expect(vm.focusedPiece).toBeNull()
    await wrapper!.get('aside button[aria-label="Close structure details"]').trigger('click')
    await flushPromises()
    vm.drawMap()
    const sample = () => {
        const ratio = window.devicePixelRatio || 1
        return [
            ...canvas
                .getContext('2d')!
                .getImageData(
                    Math.floor((canvas.clientWidth / 2 + 25) * ratio),
                    Math.floor((canvas.clientHeight / 2) * ratio),
                    1,
                    1,
                ).data,
        ]
    }
    const overlay = sample()
    vm.workspace.zoom = 1
    await flushPromises()
    expect(wrapper!.find('[data-testid="seed-map-structure-badge"]').exists()).toBe(false)
    vm.drawMap()
    expect(sample()).not.toEqual(overlay)
    vm.workspace.zoom = 0
    await flushPromises()
    vm.selectAt(canvas.clientWidth / 2 + 25, canvas.clientHeight / 2)
    await flushPromises()
    await vi.waitFor(() => expect(wrapper!.find('aside').exists()).toBe(true))
    expect(backend.details).toHaveBeenCalledOnce()
    expect(vm.focusedPiece?.id).toBe(0)
    const divider = wrapper!.get('[role="separator"]')
    const mapRegion = wrapper!.get('.seed-map-canvas-shell').element as HTMLElement
    const initialWidth = mapRegion.clientWidth
    await divider.trigger('keydown', { key: 'ArrowLeft' })
    await vi.waitFor(() => expect(mapRegion.clientWidth).toBeLessThan(initialWidth))
    const keyboardWidth = mapRegion.clientWidth
    await userEvent.dragAndDrop(divider.element, mapRegion)
    await vi.waitFor(() => expect(mapRegion.clientWidth).toBeLessThan(keyboardWidth))
    await divider.trigger('dblclick')
    await vi.waitFor(() => expect(Number(divider.attributes('aria-valuenow'))).toBe(62))
    await wrapper!.findAll('[role="tab"]')[1].trigger('click')
    await vi.waitFor(() =>
        expect(wrapper!.get('aside canvas').attributes('data-ready')).toBe('true'),
    )
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper!.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/seed-map-split-desktop.png`,
        })
    ;(wrapper!.element as HTMLElement).style.width = '650px'
    await vi.waitFor(() => expect(wrapper!.find('[role="separator"]').exists()).toBe(false))
    expect(wrapper!.element.scrollWidth).toBeLessThanOrEqual(wrapper!.element.clientWidth + 1)
    expect(mapRegion.getBoundingClientRect().bottom).toBeLessThanOrEqual(
        wrapper!.get('aside').element.getBoundingClientRect().top + 1,
    )
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper!.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/seed-map-split-narrow.png`,
        })
    await wrapper!.get('aside button[aria-label="Close structure details"]').trigger('click')
    expect(vm.selection).toBeNull()
    expect(wrapper!.find('aside').exists()).toBe(false)
})

it('paints unselected Ancient city piece projections on the map canvas', async () => {
    const details: SeedMapStructureDetails = {
        supported: true,
        truncated: false,
        containers: [],
        limits: ['terrain_unverified', 'relative_height'],
        pieces: [
            {
                id: 0,
                name: 'minecraft:ancient_city/city_center/city_center_1',
                templateId: 'minecraft:ancient_city/city_center/city_center_1',
                rotation: 0,
                positionKnown: true,
                position: { x: 79_987, y: -27, z: -120_020 },
                min: { x: 79_987, y: -27, z: -120_020 },
                max: { x: 80_004, y: 3, z: -119_980 },
            },
            {
                id: 1,
                name: 'minecraft:ancient_city/city_center/walls/bottom_1',
                templateId: 'minecraft:ancient_city/city_center/walls/bottom_1',
                rotation: 0,
                positionKnown: true,
                position: { x: 79_973, y: -26, z: -120_020 },
                min: { x: 79_973, y: -26, z: -120_020 },
                max: { x: 79_986, y: -3, z: -119_980 },
            },
        ],
    }
    backend.details.mockResolvedValue(details)
    const { vm, canvas } = await openMap()
    vm.workspace.showSpawn = false
    vm.workspace.center = { x: 80_000, z: -120_000 }
    vm.workspace.zoom = 0
    vm.workspace.displayMode = 'structures'
    vm.workspace.dimension = 'overworld'
    vm.workspace.visibleFeatures = ['ancient-city']
    vm.features = [{ kind: 'ancient-city', x: 80_000, z: -120_000, approximate: false }]
    await flushPromises()
    await vi.waitFor(() => expect(backend.details).toHaveBeenCalledOnce())
    for (const job of backend.tiles.splice(0)) await complete(job, '#00ff00')
    await vi.waitFor(() => {
        vm.drawMap()
        const ratio = window.devicePixelRatio || 1
        const x = Math.floor((canvas.clientWidth / 2 - 20) * ratio)
        const y = Math.floor((canvas.clientHeight / 2) * ratio)
        expect([...canvas.getContext('2d')!.getImageData(x, y, 1, 1).data]).not.toEqual([
            0, 255, 0, 255,
        ])
    })
    expect(backend.details.mock.calls[0][0]).toMatchObject({
        featureMask: 1 << 5,
        x: 80_000,
        z: -120_000,
    })
})

it('keeps screen coordinates and painted canvas intact when opening, dragging and closing details', async () => {
    await page.viewport(1280, 1100)
    const { vm, canvas } = await openMap()
    ;(wrapper!.element as HTMLElement).style.cssText = 'width: 1200px; height: 1000px'
    canvas.style.width = ''
    canvas.style.height = ''
    await vi.waitFor(() => expect(canvas.clientWidth).toBeGreaterThan(1000))
    vm.resizeCanvas()
    vm.workspace.showSpawn = false
    vm.workspace.visibleFeatures = []
    await flushPromises()
    vm.drawMap()
    const center = { ...vm.workspace.center }
    const screenPoint = vm.worldToScreen(center.x - 200, center.z - 100)
    const dimensions = { width: canvas.width, height: canvas.height }
    const origin = canvas.getBoundingClientRect()
    const resets: MutationRecord[] = []
    const observer = new MutationObserver((records) => resets.push(...records))
    observer.observe(canvas, { attributes: true, attributeFilter: ['width', 'height'] })
    const pixel = () => [...canvas.getContext('2d')!.getImageData(40, 40, 1, 1).data]
    const painted = pixel()
    const frames: number[][] = []
    let recording = true
    const sample = () => {
        if (recording) {
            frames.push(pixel())
            requestAnimationFrame(sample)
        }
    }
    requestAnimationFrame(sample)
    try {
        vm.selection = {
            x: 0,
            z: 0,
            feature: { kind: 'stronghold', x: 0, z: 0, approximate: true },
        }
        await vi.waitFor(() => expect(wrapper!.find('[role="separator"]').exists()).toBe(true))
        const divider = wrapper!.get('[role="separator"]')
        for (const key of ['ArrowLeft', 'ArrowLeft', 'ArrowRight', 'End', 'Home']) {
            await divider.trigger('keydown', { key })
            await new Promise((resolve) => requestAnimationFrame(resolve))
            expect(vm.worldToScreen(center.x - 200, center.z - 100)).toEqual(screenPoint)
            expect(canvas.width).toBe(dimensions.width)
            expect(canvas.height).toBe(dimensions.height)
        }
        await userEvent.dragAndDrop(divider.element, wrapper!.get('.seed-map-canvas-shell').element)
        await wrapper!.get('aside button[aria-label="Close structure details"]').trigger('click')
        await new Promise((resolve) => requestAnimationFrame(resolve))
        expect(vm.workspace.center).toEqual(center)
        expect(vm.worldToScreen(center.x - 200, center.z - 100)).toEqual(screenPoint)
        expect(canvas.getBoundingClientRect().left).toBe(origin.left)
        expect(canvas.getBoundingClientRect().top).toBe(origin.top)
        expect(canvas.width).toBe(dimensions.width)
        expect(canvas.height).toBe(dimensions.height)
        expect(resets).toHaveLength(0)
        expect(frames.length).toBeGreaterThan(0)
        expect(
            frames.every((frame) => frame.every((value, index) => value === painted[index])),
        ).toBe(true)
    } finally {
        recording = false
        observer.disconnect()
    }
})

it('keeps an End city label clickable when its origin is offscreen but its layout remains visible', async () => {
    backend.details.mockResolvedValue({
        supported: true,
        truncated: false,
        containers: [],
        limits: ['relative_height'],
        pieces: [
            {
                id: 0,
                name: 'base_floor',
                rotation: 0,
                positionKnown: true,
                position: { x: -400, y: 0, z: 0 },
                min: { x: -400, y: 0, z: 0 },
                max: { x: 20, y: 20, z: 40 },
            },
        ],
    })
    const { vm, canvas } = await openMap()
    vm.workspace.dimension = 'end'
    vm.workspace.zoom = 0
    vm.workspace.showSpawn = false
    vm.workspace.visibleFeatures = ['end-city']
    await flushPromises()
    vm.features = [{ kind: 'end-city', x: -400, z: 0, approximate: false, endShip: true }]
    await vi.waitFor(() =>
        expect(wrapper!.find('[data-testid="seed-map-structure-badge"]').exists()).toBe(true),
    )
    const badge = wrapper!.get('[data-testid="seed-map-structure-badge"]')
    expect(badge.get('img').attributes('src')).toContain('end_city_ship.webp')
    const region = canvas.parentElement!.getBoundingClientRect()
    const position = badge.element.getBoundingClientRect()
    expect(position.left).toBeGreaterThanOrEqual(region.left)
    expect(position.right).toBeLessThanOrEqual(region.right)
    await badge.trigger('click')
    await flushPromises()
    expect(vm.selection?.feature).toMatchObject({ kind: 'end-city', x: -400, endShip: true })
    expect(vm.focusedPiece).toBeNull()
})

afterEach(async () => {
    wrapper?.unmount()
    wrapper = undefined
    for (const job of backend.tiles.splice(0)) await complete(job, '#00ff00')
    for (const spy of closed) spy.mockRestore()
    closed = []
})

async function openMap() {
    wrapper = mount(LabSeedMap, {
        attachTo: document.body,
        global: {
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    await flushPromises()
    const canvas = wrapper.get('canvas').element as HTMLCanvasElement
    canvas.style.width = '512px'
    canvas.style.height = '256px'
    const vm = wrapper.vm as unknown as MapVm
    vm.resizeCanvas()
    vm.scheduleRefresh(0)
    await expect.poll(() => backend.tiles.length).toBeGreaterThan(0)
    return { vm, canvas }
}

it('opens 3D directly from Underground and suspends 2D tile generation until returning to the map', async () => {
    const { canvas } = await openMap()
    await userEvent.click(page.getByRole('button', { name: 'Underground', exact: true }))
    await expect.poll(() => wrapper?.find('[data-testid=direct-underground]').exists()).toBe(true)
    expect(wrapper!.find('[data-onboarding-id=seed-map-underground-layers]').exists()).toBe(false)
    expect(wrapper!.text()).not.toContain('Explore in 3D')
    const count = backend.tiles.length
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(backend.tiles.length).toBe(count)
    expect(canvas.width).toBe(1)
    await userEvent.click(page.getByRole('button', { name: 'Back to map', exact: true }))
    await expect.poll(() => wrapper?.find('[data-testid=direct-underground]').exists()).toBe(false)
    expect(canvas.width).toBeGreaterThan(1)
    wrapper!.unmount()
    wrapper = undefined
    expect(canvas.width).toBe(1)
    expect(canvas.height).toBe(1)
})

it('leaves underground mode when the selected dimension no longer supports 3D generation', async () => {
    const { vm } = await openMap()
    vm.workspace.displayMode = 'underground'
    await expect.poll(() => wrapper?.find('[data-testid=direct-underground]').exists()).toBe(true)
    vm.workspace.dimension = 'end'
    await flushPromises()
    expect(vm.workspace.displayMode).toBe('structures')
    expect(wrapper!.find('[data-testid=direct-underground]').exists()).toBe(false)
    vm.workspace.displayMode = 'underground'
    await flushPromises()
    expect(vm.workspace.displayMode).toBe('structures')
})

it('restores a saved underground workspace directly into 3D after profiles load without generating 2D tiles', async () => {
    localStorage.setItem(
        'axolotl.lab.seed-map.v1',
        JSON.stringify({ ...createDefaultSeedMapWorkspace(), displayMode: 'underground' }),
    )
    wrapper = mount(LabSeedMap, {
        attachTo: document.body,
        global: {
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    await expect.poll(() => wrapper?.find('[data-testid=direct-underground]').exists()).toBe(true)
    expect((wrapper.vm as unknown as MapVm).workspace.displayMode).toBe('underground')
    await new Promise((resolve) => setTimeout(resolve, 150))
    expect(backend.tiles.length).toBe(0)
})

async function complete(job: (typeof backend.tiles)[number], color: string) {
    const source = new OffscreenCanvas(job.request.width, job.request.height)
    const context = source.getContext('2d')!
    context.fillStyle = color
    context.fillRect(0, 0, source.width, source.height)
    const bitmap = source.transferToImageBitmap()
    closed.push(vi.spyOn(bitmap, 'close'))
    job.resolve({
        epoch: job.request.epoch,
        width: source.width,
        height: source.height,
        bitmap,
        approximate: true,
    })
    await flushPromises()
}

it('renders seed changes while spawn is pending and deduplicates spawn work across map refreshes', async () => {
    const { vm } = await openMap()
    expect(backend.spawns).toHaveLength(1)
    expect(backend.features).not.toHaveBeenCalled()
    vm.workspace.seed = '456'
    await flushPromises()
    vm.scheduleRefresh(0)
    await expect.poll(() => backend.spawns.length).toBe(2)
    vm.workspace.center.x += 1
    vm.scheduleRefresh(0)
    await new Promise((resolve) => setTimeout(resolve, 30))
    expect(backend.spawns).toHaveLength(2)
    for (const job of backend.tiles.splice(0)) await complete(job, '#ff0000')
    await expect.poll(() => backend.tiles.some((job) => job.request.seed === '456')).toBe(true)
    backend.spawns[0].resolve({ x: 999, z: 999, approximate: false })
    await flushPromises()
    expect(vm.workspace.center.x).not.toBe(999)
    backend.spawns[1].resolve({ x: 128, z: 64, approximate: false })
    await flushPromises()
    expect(vm.workspace.center).toEqual({ x: 128, z: 64 })
})

it('keeps real canvas pixels across LoD changes and progressively paints new detail over old tiles', async () => {
    const { vm, canvas } = await openMap()
    const initial = backend.tiles.splice(0)
    for (const job of initial) await complete(job, '#ff0000')
    for (let batch = 0; batch < 4 && !backend.features.mock.calls.length; batch++) {
        await new Promise((resolve) => setTimeout(resolve, 30))
        for (const job of backend.tiles.splice(0)) await complete(job, '#ff0000')
    }
    vm.drawMap()
    const sample = () => [
        ...canvas
            .getContext('2d')!
            .getImageData(
                Math.round(300 * devicePixelRatio),
                Math.round(150 * devicePixelRatio),
                1,
                1,
            ).data,
    ]
    expect(sample()).toEqual([255, 0, 0, 255])
    expect(backend.features).toHaveBeenCalledTimes(1)
    vm.workspace.zoom = 0
    await flushPromises()
    vm.scheduleRefresh(0)
    vm.drawMap()
    expect(sample()).toEqual([255, 0, 0, 255])
    expect(closed.every((spy) => spy.mock.calls.length === 0)).toBe(true)
    const isTarget = (job: (typeof backend.tiles)[number]) =>
        job.request.scale === 1 &&
        job.request.width === 256 &&
        job.request.x === 0 &&
        job.request.z === 0
    await expect
        .poll(async () => {
            for (const job of [...backend.tiles]) {
                if (isTarget(job)) continue
                backend.tiles.splice(backend.tiles.indexOf(job), 1)
                await complete(job, '#ff0000')
            }
            return backend.tiles.find(isTarget)
        })
        .toBeDefined()
    const fine = backend.tiles.find(isTarget)!
    backend.tiles.splice(backend.tiles.indexOf(fine), 1)
    await complete(fine, '#00ff00')
    vm.drawMap()
    expect(sample()).toEqual([0, 255, 0, 255])
    expect(closed.every((spy) => spy.mock.calls.length === 0)).toBe(true)
})

it('defers ore scanning until base coverage and invalidates pending work on a seed change', async () => {
    const { vm } = await openMap()
    vm.workspace.displayMode = 'ores'
    vm.workspace.zoom = -1
    await flushPromises()
    vm.scheduleRefresh(0)
    await new Promise((resolve) => setTimeout(resolve, 220))
    expect(backend.ores).not.toHaveBeenCalled()
    for (let batch = 0; batch < 10 && !backend.ores.mock.calls.length; batch++) {
        for (const job of backend.tiles.splice(0)) await complete(job, '#ff0000')
        await new Promise((resolve) => setTimeout(resolve, 60))
    }
    await expect.poll(() => backend.ores.mock.calls.length).toBeGreaterThan(0)
    backend.ores.mockClear()
    vm.workspace.seed = '789'
    await flushPromises()
    vm.scheduleRefresh(0)
    await new Promise((resolve) => setTimeout(resolve, 220))
    expect(backend.ores).not.toHaveBeenCalled()
})

it.each([
    { mode: 'structures', dimension: 'overworld' },
    { mode: 'structures', dimension: 'nether' },
    { mode: 'structures', dimension: 'end' },
    { mode: 'ores', dimension: 'overworld' },
    { mode: 'ores', dimension: 'nether' },
] as const)(
    'toggles every $mode layer button in the $dimension, including its visible selected state',
    async ({ mode, dimension }) => {
        const { vm } = await openMap()
        vm.workspace.dimension = dimension
        vm.workspace.displayMode = mode
        await flushPromises()
        const buttons = wrapper!.findAll('.layer-options > button')
        expect(buttons.length).toBeGreaterThan(0)
        for (const button of buttons) {
            const wasSelected = button.attributes('aria-pressed') === 'true'
            await userEvent.click(button.element)
            await flushPromises()
            expect(button.attributes('aria-pressed')).toBe(String(!wasSelected))
            expect(button.classes()).toContain(
                wasSelected ? 'button-frame--base' : 'button-frame--colored',
            )
            await userEvent.click(button.element)
            await flushPromises()
            expect(button.attributes('aria-pressed')).toBe(String(wasSelected))
            expect(button.classes()).toContain(
                wasSelected ? 'button-frame--colored' : 'button-frame--base',
            )
        }
    },
)

it('loads the provided camp and fossil PNGs and omits the removed end-island layer', async () => {
    const { vm } = await openMap()
    const camp = wrapper!.get('.layer-options img[src$="/abandoned_camp.png"]')
        .element as HTMLImageElement
    await camp.decode()
    expect(camp.naturalWidth).toBe(26)
    vm.workspace.dimension = 'nether'
    await flushPromises()
    const fossil = wrapper!.get('.layer-options img[src$="/nether_fossil.png"]')
        .element as HTMLImageElement
    await fossil.decode()
    expect(fossil.naturalWidth).toBe(26)
    vm.workspace.dimension = 'end'
    await flushPromises()
    expect(wrapper!.findAll('.layer-options > button')).toHaveLength(3)
})
