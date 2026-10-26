import assert from 'node:assert/strict'
import { after, before, test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { BlockDefinition, BlockModel, Cull, Identifier } from 'deepslate'
import { createServer, type ViteDevServer } from 'vite'

import type { SchematicBlockState } from './backend.ts'
import { getSchematicMeshOcclusionFaces, isSchematicOccluding } from './meshing.ts'

type BuiltinResourcesModule = typeof import('./builtin-resources.ts')

let server: ViteDevServer
let builtin: BuiltinResourcesModule

before(async () => {
    const appRoot = fileURLToPath(new URL('../../../', import.meta.url))
    server = await createServer({
        root: appRoot,
        configFile: `${appRoot}/vite.config.ts`,
        // This SSR fixture must not replace an active dev server's optimized dependencies.
        cacheDir: `${appRoot}/node_modules/.vite-schematic-resources`,
        server: { middlewareMode: true, hmr: false },
        appType: 'custom',
    })
    builtin = (await server.ssrLoadModule(
        '/src/lab/schematic-preview/builtin-resources.ts',
    )) as BuiltinResourcesModule
})

after(async () => {
    await server.close()
})

test('built-in assets render stained glass and sugar cane across supported versions', () => {
    const palette: SchematicBlockState[] = [
        { name: 'minecraft:air', properties: {} },
        { name: 'minecraft:white_stained_glass', properties: {} },
        { name: 'minecraft:sugar_cane', properties: {} },
    ]
    for (const version of ['1.21', '26.2', '26.3']) {
        const resources = builtin.loadBuiltinBlockResources(version, palette)
        assert.equal(resources.defaultBlockProperties['minecraft:sugar_cane']?.age, '0')
        const models = Object.fromEntries(
            Object.entries(resources.blockModels).map(([id, model]) => [
                id,
                BlockModel.fromJson(model),
            ]),
        )
        const modelProvider = {
            getBlockModel: (id: Identifier) => models[id.toString()] ?? null,
        }
        for (const model of Object.values(models)) model.flatten(modelProvider)

        const textureUvs = builtin.builtinTextureUvs()
        for (const blockName of ['white_stained_glass', 'sugar_cane']) {
            const state = palette.find((entry) => entry.name === `minecraft:${blockName}`)
            assert.ok(state)
            assert.ok(textureUvs[`minecraft:block/${blockName}`])
            const definition = BlockDefinition.fromJson(resources.blockDefinitions[state.name])
            const mesh = definition.getMesh(
                Identifier.parse(state.name),
                state.properties,
                {
                    getTextureAtlas: () => ({}) as ImageData,
                    getTextureUV: (id) => textureUvs[id.toString()] ?? [0, 0, 0, 0],
                },
                modelProvider,
                Cull.none(),
            )
            assert.ok(mesh.quads.length > 0, `${version} generated no mesh for ${state.name}`)
        }
    }
})

test('built-in assets expose the complete renderer resource set', () => {
    const textureUvs = builtin.builtinTextureUvs()
    assert.ok(Object.keys(textureUvs).length >= 2600)
    assert.ok(textureUvs['minecraft:block/white_stained_glass'])
    assert.ok(textureUvs['minecraft:block/sugar_cane'])
    assert.deepEqual(builtin.builtinTextureRegion('minecraft:block/white_stained_glass'), {
        x: 1568,
        y: 1968,
        width: 16,
        height: 16,
    })
})

test('all built-in block resources remain compatible with Deepslate', () => {
    const palette = builtin.listBuiltinBlockStates()
    const resources = builtin.loadBuiltinBlockResources('26.3', palette)
    const definitions = Object.fromEntries(
        Object.entries(resources.blockDefinitions).map(([id, definition]) => [
            id,
            BlockDefinition.fromJson(definition),
        ]),
    )
    const models = Object.fromEntries(
        Object.entries(resources.blockModels).map(([id, model]) => [
            id,
            BlockModel.fromJson(model),
        ]),
    )
    const modelProvider = {
        getBlockModel: (id: Identifier) => models[id.toString()] ?? null,
    }
    for (const model of Object.values(models)) model.flatten(modelProvider)

    const textureUvs = builtin.builtinTextureUvs()
    const atlasProvider = {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: (id: Identifier) => textureUvs[id.toString()] ?? [0, 0, 0, 0],
    }
    for (const state of palette) {
        try {
            definitions[state.name]?.getMesh(
                Identifier.parse(state.name),
                state.properties,
                atlasProvider,
                modelProvider,
                Cull.none(),
            )
        } catch (error) {
            throw new Error(`Unable to render built-in block ${state.name}`, { cause: error })
        }
    }
})

test('partial redstone models do not hide every face of adjacent blocks', () => {
    const reportedNames = [
        'minecraft:observer',
        'minecraft:redstone_wire',
        'minecraft:lantern',
        'minecraft:hopper',
        'minecraft:repeater',
        'minecraft:piston',
    ]
    const availableStates = builtin.listBuiltinBlockStates()
    const palette = [
        { name: 'minecraft:air', properties: {} },
        ...['minecraft:stone', ...reportedNames].map((name) => {
            const state = availableStates.find((entry) => entry.name === name)
            assert.ok(state)
            return name === 'minecraft:piston'
                ? { ...state, properties: { ...state.properties, extended: 'true' } }
                : state
        }),
    ]
    const resources = builtin.loadBuiltinBlockResources('26.3', palette)
    const definitions = Object.fromEntries(
        Object.entries(resources.blockDefinitions).map(([id, definition]) => [
            id,
            BlockDefinition.fromJson(definition),
        ]),
    )
    const models = Object.fromEntries(
        Object.entries(resources.blockModels).map(([id, model]) => [
            id,
            BlockModel.fromJson(model),
        ]),
    )
    const modelProvider = {
        getBlockModel: (id: Identifier) => models[id.toString()] ?? null,
    }
    for (const model of Object.values(models)) model.flatten(modelProvider)
    const atlasProvider = {
        getTextureAtlas: () => ({}) as ImageData,
        getTextureUV: () => [0, 0, 1, 1] as [number, number, number, number],
    }
    const occlusionFaces = (state: SchematicBlockState) => {
        const paletteIndex = palette.indexOf(state)
        if (!isSchematicOccluding(paletteIndex, palette)) return {}
        const properties = {
            ...(resources.defaultBlockProperties[state.name] ?? {}),
            ...state.properties,
        }
        const mesh = definitions[state.name].getMesh(
            Identifier.parse(state.name),
            properties,
            atlasProvider,
            modelProvider,
            Cull.none(),
        )
        return getSchematicMeshOcclusionFaces(mesh)
    }

    assert.equal(Object.keys(occlusionFaces(palette[1])).length, 6)
    for (const state of palette.slice(2)) {
        assert.ok(
            Object.keys(occlusionFaces(state)).length < 6,
            `${state.name} was treated as a full occluding cube`,
        )
    }
})

test('external models inherit missing vanilla parents from built-in assets', () => {
    const blockDefinitions: Record<string, unknown> = {
        'example:machine': { variants: { '': { model: 'example:block/machine' } } },
    }
    const blockModels: Record<string, unknown> = {
        'example:block/machine': {
            parent: 'minecraft:block/cube_all',
            textures: { all: 'minecraft:block/white_stained_glass' },
        },
    }
    builtin.completeBuiltinModelDependencies('26.2', blockDefinitions, blockModels)
    assert.ok(blockModels['minecraft:block/cube_all'])
})

test('unqualified model parents use the vanilla namespace', () => {
    const blockDefinitions: Record<string, unknown> = {
        'example:machine': { variants: { '': { model: 'example:block/machine' } } },
    }
    const blockModels: Record<string, unknown> = {
        'example:block/machine': { parent: 'block/cube_all' },
    }
    builtin.completeBuiltinModelDependencies('26.2', blockDefinitions, blockModels)
    assert.ok(blockModels['minecraft:block/cube_all'])
})

test('canceling an atlas decode releases the bitmap and rejects the old resource task', async () => {
    const controller = new AbortController()
    const originalFetch = globalThis.fetch
    const bitmapDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'createImageBitmap')
    let closed = false
    globalThis.fetch = async (_, options) => {
        assert.equal(options?.signal, controller.signal)
        return new Response(new Blob())
    }
    Object.defineProperty(globalThis, 'createImageBitmap', {
        configurable: true,
        value: async () => {
            controller.abort()
            return {
                close: () => {
                    closed = true
                },
            }
        },
    })
    try {
        await assert.rejects(builtin.loadBuiltinAtlas(controller.signal), { name: 'AbortError' })
        assert.equal(closed, true)
    } finally {
        globalThis.fetch = originalFetch
        if (bitmapDescriptor)
            Object.defineProperty(globalThis, 'createImageBitmap', bitmapDescriptor)
        else Reflect.deleteProperty(globalThis, 'createImageBitmap')
    }
})
