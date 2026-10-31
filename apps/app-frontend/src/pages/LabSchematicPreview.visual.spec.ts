import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import type { SchematicEditResult, SchematicPreviewManifest } from '@/lab/schematic-preview/backend'

import LabSchematicPreview from './LabSchematicPreview.vue'

const fixture = vi.hoisted(() => ({
    open: vi.fn(),
    save: vi.fn(),
    read: vi.fn(),
    edit: vi.fn(),
    transform: vi.fn(),
    resources: vi.fn(),
    error: vi.fn(),
    notify: vi.fn(),
    write: vi.fn(),
    close: vi.fn(),
    cancel: vi.fn(),
    select: undefined as
        undefined | ((selection: { regionId: string; position: [number, number, number] }) => void),
    scene: {} as Record<string, ReturnType<typeof vi.fn>>,
}))

vi.mock('@/components/lab/schematic-preview/SchematicBlockPickerModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/schematic-preview/SchematicInfoModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/schematic-preview/SchematicInstancePickerModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/schematic-preview/SchematicMaterialSwatch.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/helpers/instance', () => ({ list: async () => [] }))
vi.mock('vue-router', () => ({ useRoute: () => ({ query: {} }) }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: fixture.open, save: fixture.save }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: fixture.write, writeTextFile: fixture.write }))
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => 'windows' }))
vi.mock('@tauri-apps/api/webview', () => ({
    getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }),
}))
vi.mock('@/lab/schematic-preview/backend', () => ({
    openSchematicPreview: fixture.open,
    readSchematicChunk: fixture.read,
    applySchematicEdits: fixture.edit,
    transformSchematic: fixture.transform,
    closeSchematicPreview: fixture.close,
    cancelSchematicPreview: fixture.cancel,
    exportSchematicSponge: async () => new ArrayBuffer(0),
    exportSchematicLitematic: async () => new ArrayBuffer(0),
}))
vi.mock('@/lab/schematic-preview/resources', () => ({
    createSchematicResources: fixture.resources,
    minecraftVersionFromDataVersion: () => 'latest',
    resolveSchematicBlockName: (name: string) => name,
    resolveSchematicMaterialTexture: () => undefined,
}))
vi.mock('@/lab/schematic-preview/scene', () => ({
    SchematicPreviewScene: function (options: { onSelect: typeof fixture.select }) {
        fixture.select = options.onSelect
        return fixture.scene
    },
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', [slots.default?.(), slots.actions?.()]),
    })
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        EmptyState: control,
        Slider: control,
        StyledInput: control,
        Tabs: control,
        TagItem: control,
        OverflowMenu: defineComponent({
            props: ['options'],
            setup: (props) => () =>
                h(
                    'div',
                    props.options.map((option: { id?: string; action?: () => void }) =>
                        h(
                            'button',
                            { 'data-menu-id': option.id, onClick: option.action },
                            option.id,
                        ),
                    ),
                ),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            locale: ref('en-US'),
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
        injectNotificationManager: () => ({
            handleError: fixture.error,
            addNotification: fixture.notify,
        }),
    }
})

function manifest(id: string): SchematicPreviewManifest {
    return {
        sessionId: id,
        fileName: `${id}.schem`,
        sourcePath: `${id}.schem`,
        format: 'schem_v3',
        formatVersion: 3,
        min: [0, 0, 0],
        max: [0, 0, 0],
        size: [1, 1, 1],
        blockCount: 1,
        entityCount: 0,
        blockEntityCount: 0,
        palette: [
            { name: 'minecraft:air', properties: {} },
            { name: 'minecraft:stone', properties: {} },
        ],
        materials: [{ name: 'minecraft:stone', count: 1 }],
        warnings: [],
        regions: [
            {
                id: 'region',
                name: 'region',
                origin: [0, 0, 0],
                min: [0, 0, 0],
                max: [0, 0, 0],
                size: [1, 1, 1],
                blockCount: 1,
                chunks: [{ position: [0, 0, 0], nonAirBlocks: 1 }],
            },
        ],
    }
}

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: Error) => void
    const promise = new Promise<T>((done, fail) => {
        resolve = done
        reject = fail
    })
    return { promise, resolve, reject }
}

function resources() {
    return {
        texture: { dispose: vi.fn() },
        workerResources: {
            blockDefinitions: {},
            blockModels: {},
            textureUvs: {},
            bubbleUv: [0, 0.52, 0.004, 0.522],
        },
        blockNames: { en_us: {}, zh_cn: {} },
        availableBlockStates: [],
        atlas: document.createElement('canvas'),
    }
}

class MeshWorker {
    onmessage?: (event: { data: unknown }) => void
    onerror?: (event: { message: string }) => void
    terminate() {}
    postMessage(message: {
        type: string
        epoch: number
        regionId: string
        chunkPosition: number[]
    }) {
        const empty = {
            positions: new Float32Array(),
            normals: new Float32Array(),
            uvs: new Float32Array(),
            colors: new Float32Array(),
            blockPositions: new Float32Array(),
        }
        queueMicrotask(() =>
            this.onmessage?.({
                data:
                    message.type === 'init'
                        ? { type: 'ready', epoch: message.epoch, warnings: [] }
                        : {
                              ...message,
                              type: 'mesh',
                              opaque: empty,
                              translucent: empty,
                              bubble: empty,
                              water: empty,
                              lava: empty,
                              missing: [],
                          },
            }),
        )
    }
}

let wrapper: ReturnType<typeof mount>
beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    fixture.scene = Object.fromEntries(
        [
            'setRegions',
            'fitView',
            'setTextures',
            'clearChunks',
            'setChunk',
            'setSelectionBlocks',
            'setMeasurement',
            'setSelection',
            'setLayerRange',
            'setExplosion',
            'dispose',
        ].map((name) => [name, vi.fn()]),
    )
    fixture.open.mockImplementation(async (source: { path: string }) => manifest(source.path))
    fixture.resources.mockImplementation(async () => resources())
    fixture.read.mockImplementation(async () => {
        const blocks = new Uint32Array(4096)
        blocks[0] = 1
        return blocks
    })
    fixture.transform.mockImplementation(async (id: string) => manifest(id))
    vi.stubGlobal('Worker', MeshWorker)
    wrapper = mount(LabSchematicPreview, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} } },
    })
})
afterEach(() => {
    wrapper.unmount()
    vi.unstubAllGlobals()
})

async function openFile(id: string) {
    fixture.open.mockImplementationOnce(async () => id)
    const button = wrapper.find('[data-menu-id="open-file"]')
    if (button.exists()) await button.trigger('click')
    else
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Open file')!
            .trigger('click')
    await flushPromises()
}

function button(label: string) {
    return wrapper
        .findAll('button')
        .find((button) => button.text() === label || button.attributes('aria-label') === label)!
}

it('old edit completion cannot overwrite B or clear B busy state', async () => {
    await openFile('A')
    const pending = deferred<SchematicEditResult>()
    fixture.edit.mockReturnValueOnce(pending.promise)
    fixture.select?.({ regionId: 'region', position: [0, 0, 0] })
    await nextTick()
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }))
    await nextTick()
    expect(fixture.edit.mock.calls[0][0]).toBe('A')
    await openFile('B')
    const transform = deferred<SchematicPreviewManifest>()
    fixture.transform.mockReturnValueOnce(transform.promise)
    await button('Rotate clockwise').trigger('click')
    pending.resolve({ manifest: manifest('A'), changedChunks: [], appliedPaletteIndices: [0] })
    await flushPromises()
    expect(wrapper.get('h1').text()).toBe('B.schem')
    expect(button('Rotate clockwise').attributes('disabled')).toBeDefined()
    expect(button('Undo').attributes('disabled')).toBeDefined()
    expect(fixture.error).not.toHaveBeenCalled()
    transform.resolve(manifest('B'))
    await flushPromises()
    expect(button('Rotate clockwise').attributes('disabled')).toBeUndefined()
    expect(button('Undo').attributes('disabled')).toBeUndefined()
})

it('old transform failure is silent and cannot restore A history after opening B', async () => {
    await openFile('A')
    await button('Rotate clockwise').trigger('click')
    await flushPromises()
    const pending = deferred<SchematicPreviewManifest>()
    fixture.transform.mockReturnValueOnce(pending.promise)
    await button('Undo').trigger('click')
    await openFile('B')
    pending.reject(new Error('A operation failed'))
    await flushPromises()
    expect(wrapper.get('h1').text()).toBe('B.schem')
    expect(button('Undo').attributes('disabled')).toBeDefined()
    expect(button('Redo').attributes('disabled')).toBeDefined()
    expect(fixture.error).not.toHaveBeenCalled()
})

it('stale resources are disposed and an export picker cannot export B under A filename', async () => {
    const pending = deferred<ReturnType<typeof resources>>()
    fixture.resources.mockReturnValueOnce(pending.promise)
    await openFile('A')
    const signal = fixture.resources.mock.calls[0][2] as AbortSignal
    await openFile('B')
    expect(signal.aborted).toBe(true)
    const stale = resources()
    pending.resolve(stale)
    await flushPromises()
    expect(stale.texture.dispose).toHaveBeenCalledOnce()
    expect(wrapper.get('h1').text()).toBe('B.schem')
    const save = deferred<string>()
    fixture.save.mockReturnValueOnce(save.promise)
    await wrapper.get('[data-menu-id="materials-csv"]').trigger('click')
    await openFile('C')
    save.resolve('B-materials.csv')
    await flushPromises()
    expect(fixture.write).not.toHaveBeenCalled()
    expect(fixture.notify).not.toHaveBeenCalled()
})

it.each(['transform', 'undo', 'redo'])(
    'stale %s success preserves B selection and history',
    async (operation) => {
        await openFile('A')
        if (operation !== 'transform') {
            await button('Rotate clockwise').trigger('click')
            await flushPromises()
        }
        if (operation === 'redo') {
            await button('Undo').trigger('click')
            await flushPromises()
        }
        const pending = deferred<SchematicPreviewManifest>()
        fixture.transform.mockReturnValueOnce(pending.promise)
        await button(
            operation === 'transform' ? 'Rotate clockwise' : operation === 'undo' ? 'Undo' : 'Redo',
        ).trigger('click')
        await openFile('B')
        fixture.select?.({ regionId: 'region', position: [0, 0, 0] })
        await nextTick()
        pending.resolve(manifest('A'))
        await flushPromises()
        expect(wrapper.get('h1').text()).toBe('B.schem')
        expect(button('Clear selection').attributes('disabled')).toBeUndefined()
        expect(button('Undo').attributes('disabled')).toBeDefined()
        expect(button('Redo').attributes('disabled')).toBeDefined()
        expect(fixture.error).not.toHaveBeenCalled()
    },
)

it('a chunk from A cannot update the scene while B is still opening', async () => {
    const read = deferred<Uint32Array>()
    fixture.read.mockReturnValueOnce(read.promise)
    await openFile('A')
    const opened = deferred<SchematicPreviewManifest>()
    fixture.open.mockImplementation((source: { path: string }) =>
        source.path === 'B' ? opened.promise : Promise.resolve(manifest(source.path)),
    )
    await openFile('B')
    fixture.scene.setChunk.mockClear()
    read.resolve(new Uint32Array(4096).fill(1))
    await flushPromises()
    expect(fixture.scene.setChunk).not.toHaveBeenCalled()
    expect(fixture.read.mock.calls.every(([id]) => id === 'A')).toBe(true)
    opened.resolve(manifest('B'))
    await flushPromises()
    expect(wrapper.get('h1').text()).toBe('B.schem')
})

it('unmounting invalidates pending transform results and errors', async () => {
    await openFile('A')
    const pending = deferred<SchematicPreviewManifest>()
    fixture.transform.mockReturnValueOnce(pending.promise)
    await button('Rotate clockwise').trigger('click')
    wrapper.unmount()
    fixture.scene.setRegions.mockClear()
    pending.reject(new Error('late error'))
    await flushPromises()
    expect(fixture.error).not.toHaveBeenCalled()
    expect(fixture.scene.setRegions).not.toHaveBeenCalled()
})

it('changing glass rendering while B loads resources preserves B resource ownership', async () => {
    await openFile('A')
    const pending = deferred<ReturnType<typeof resources>>()
    fixture.resources.mockReturnValueOnce(pending.promise)
    await openFile('B')
    fixture.scene.setTextures.mockClear()
    await wrapper.get('[data-menu-id="seamless-glass"]').trigger('click')
    const loaded = resources()
    pending.resolve(loaded)
    await flushPromises()
    expect(loaded.texture.dispose).not.toHaveBeenCalled()
    expect(fixture.scene.setTextures).toHaveBeenLastCalledWith({ atlas: loaded.texture })
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false)
    expect(button('Rotate clockwise').attributes('disabled')).toBeUndefined()
})

it('every pass the worker meshes reaches the scene, bubbles included', async () => {
    await openFile('A')
    const calls = fixture.scene.setChunk.mock.calls
    const [, , meshes] = calls[calls.length - 1] as [
        string,
        [number, number, number],
        Record<string, unknown>,
    ]
    // A bubble column has no block model, so dropping its pass leaves nothing but water on screen.
    expect(meshes.bubble).toBeDefined()
    expect(meshes.water).toBeDefined()
})
