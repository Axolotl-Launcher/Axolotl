import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { getServerInstallStrategy } from '@/composables/server-install'
import { resumeModpackInstall } from '@/composables/useServerInstalls'
import { useServerLifecycle } from '@/composables/useServerLifecycle'
import type { ServerView } from '@/composables/useServers'
import type { ServerInfoData } from '@/helpers/servers'
import { mountThemed } from '@/test/visual-harness'

import {
    createCreateServerFlowContext,
    type CreateServerFlowContextValue,
    provideCreateServerFlow,
} from './create-server-flow'
import {
    createModpackServerFlowContext,
    resolveModpackLoader,
} from './modpack/create-modpack-server-flow'
import TypeStage from './stages/TypeStage.vue'

const api = vi.hoisted(() => ({
    loaders: vi.fn(),
    install: vi.fn().mockResolvedValue(undefined),
    create: vi.fn().mockResolvedValue({ id: 'neo-server' }),
    write: vi.fn().mockResolvedValue(undefined),
    refresh: vi.fn().mockResolvedValue(undefined),
    packInstall: vi.fn().mockResolvedValue(undefined),
    version: vi.fn(),
}))
vi.mock('@/helpers/metadata', () => ({
    get_loader_versions: api.loaders,
    get_game_versions: async () => ({
        latest: { release: '1.21.1' },
        versions: [
            { id: '1.21.1', type: 'release' },
            { id: '1.20.1', type: 'release' },
        ],
    }),
}))
vi.mock('@/helpers/jre', () => ({
    find_filtered_jres: async () => [],
    get_java_default_versions: async () => [],
    get_max_memory: async () => 8192 * 1024,
}))
vi.mock('@/helpers/servers', () => ({
    servers: {
        installNeoForge: api.install,
        create: api.create,
        writeFile: api.write,
        installModpack: api.packInstall,
        list: async () => [],
    },
    serverEventListener: async () => () => {},
}))
vi.mock('@/composables/useServers', () => ({
    refresh: api.refresh,
    useServers: () => ({ startServer: vi.fn() }),
    setServerExitReasonHandler: () => () => {},
}))
vi.mock('@/helpers/cache.js', () => ({ get_version: api.version }))
vi.mock('@/providers/download-manager', () => ({ injectDownloadManager: () => null }))
vi.mock('./stages/InstallStage.vue', () => ({ default: { render: () => null } }))
vi.mock('./stages/SetupStage.vue', () => ({ default: { render: () => null } }))
vi.mock('./modpack/stages/ModpackSetupStage.vue', () => ({ default: { render: () => null } }))
vi.mock('./modpack/stages/ModpackInstallStage.vue', () => ({ default: { render: () => null } }))

let wrapper: Awaited<ReturnType<typeof mountThemed>> | undefined
let ctx: CreateServerFlowContextValue
afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    document.body.replaceChildren()
    vi.clearAllMocks()
    api.loaders.mockReset()
})

async function wizard() {
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    wrapper = await mountThemed(
        defineComponent({
            setup() {
                ctx = createCreateServerFlowContext(ref(null))
                ctx.selectedGameVersion.value = '1.21.1'
                ctx.availableGameVersions.value = ['1.21.1', '1.20.1']
                provideCreateServerFlow(ctx)
                return () => h(TypeStage)
            },
        }),
        {},
        'dark',
        {
            global: {
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref('en-US'),
                        t: (key: string) => key,
                        setLocale: () => {},
                    },
                },
            },
        },
    )
    return wrapper
}

function manifest(game: string, loaders: { id: string; stable: boolean }[]) {
    return { gameVersions: [{ id: game, loaders }] }
}

it('offers NeoForge, loads its game-scoped catalog and installs the selected version', async () => {
    api.loaders.mockResolvedValue(
        manifest('1.21.1', [
            { id: '21.1.173-beta', stable: false },
            { id: '21.1.172', stable: true },
        ]),
    )
    const view = await wizard()
    await view
        .findAll('button')
        .find((button) => button.text() === 'NeoForge')!
        .trigger('click')
    await flushPromises()
    expect(api.loaders).toHaveBeenCalledWith('neo', '1.21.1')
    expect(ctx.selectedLoaderVersion.value).toBe('21.1.172')
    expect(ctx.canContinueFromType.value).toBe(true)
    ctx.name.value = 'NeoForge server'
    ctx.selectedJava.value = { path: 'java', version: '21' }
    await ctx.beginInstall()
    expect(api.create).toHaveBeenCalledWith(
        expect.objectContaining({
            serverType: 'neoforge',
            loaderVersion: '21.1.172',
            gameVersion: '1.21.1',
        }),
    )
    expect(api.install).toHaveBeenCalledWith('neo-server', '1.21.1', '21.1.172', 'java')
    expect(ctx.installPhase.value).toBe('done')
})

it('ignores older catalog responses when the game version changes', async () => {
    let finishOld!: (value: ReturnType<typeof manifest>) => void
    api.loaders.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finishOld = resolve
            }),
    )
    api.loaders.mockResolvedValueOnce(manifest('1.20.1', [{ id: '47.1.106', stable: true }]))
    await wizard()
    ctx.serverType.value = 'neoforge'
    const old = ctx.loadLoaderVersions()
    ctx.selectedGameVersion.value = '1.20.1'
    await ctx.loadLoaderVersions()
    finishOld(manifest('1.21.1', [{ id: '21.1.172', stable: true }]))
    await old
    expect(ctx.selectedLoaderVersion.value).toBe('47.1.106')
    expect(ctx.loaderVersions.value.map((loader) => loader.id)).toEqual(['47.1.106'])
})

it('blocks missing catalogs and preserves error recovery in the type stage', async () => {
    api.loaders.mockResolvedValueOnce(manifest('1.21.1', []))
    const view = await wizard()
    ctx.serverType.value = 'neoforge'
    await ctx.loadLoaderVersions()
    await flushPromises()
    expect(ctx.canContinueFromType.value).toBe(false)
    expect(view.text()).toContain('No loader versions are available')
    api.loaders.mockRejectedValueOnce(new Error('Catalog unavailable'))
    await ctx.loadLoaderVersions()
    await flushPromises()
    expect(view.find('[role="alert"]').text()).toContain('Catalog unavailable')
    api.loaders.mockResolvedValueOnce(manifest('1.21.1', [{ id: '21.1.172', stable: true }]))
    await view.find('[role="alert"] button').trigger('click')
    await flushPromises()
    expect(ctx.canContinueFromType.value).toBe(true)
    expect(view.find('[role="alert"]').exists()).toBe(false)
})

it('switching to vanilla clears pending NeoForge versions without getting stuck loading', async () => {
    let finish!: (value: ReturnType<typeof manifest>) => void
    api.loaders.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    await wizard()
    ctx.serverType.value = 'neoforge'
    const request = ctx.loadLoaderVersions()
    ctx.serverType.value = 'vanilla'
    await ctx.loadLoaderVersions()
    finish(manifest('1.21.1', [{ id: '21.1.172', stable: true }]))
    await request
    expect(ctx.loaderVersions.value).toEqual([])
    expect(ctx.isVersionsLoading.value).toBe(false)
    expect(ctx.canContinueFromType.value).toBe(true)
})

it('uses an installer strategy for NeoForge and rejects an absent selection', async () => {
    const strategy = getServerInstallStrategy('neoforge')
    await expect(strategy.install('neo-server', { gameVersion: '1.21.1' })).rejects.toThrow(
        'Select a NeoForge version',
    )
    expect(api.install).not.toHaveBeenCalled()
    expect(resolveModpackLoader(['neoforge'])).toEqual({ type: 'neoforge', label: 'NeoForge' })
})

it('dispatches NeoForge modpacks and retries without resolving a latest loader', async () => {
    api.loaders.mockRejectedValue(new Error('Catalog unavailable'))
    const packVersion = {
        id: 'pack-version',
        version_number: '1.0',
        game_versions: ['1.21.1'],
        files: [{ primary: true, url: 'https://example.com/pack.mrpack', hashes: { sha1: 'abc' } }],
    }
    api.version.mockResolvedValue(packVersion)
    let pack!: ReturnType<typeof createModpackServerFlowContext>
    wrapper = await mountThemed(
        defineComponent({
            setup() {
                pack = createModpackServerFlowContext(ref(null))
                return () => null
            },
        }),
        {},
        'dark',
        {
            global: {
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref('en-US'),
                        t: (key: string) => key,
                        setLocale: () => {},
                    },
                },
            },
        },
    )
    pack.setPack(
        {
            id: 'project',
            title: 'Neo pack',
            loaders: ['neoforge'],
            game_versions: ['1.21.1'],
        } as Parameters<typeof pack.setPack>[0],
        packVersion as Parameters<typeof pack.setPack>[1],
    )
    pack.selectedJava.value = { path: 'java', version: '21' }
    await pack.beginInstall()
    expect(pack.installPhase.value).toBe('done')
    expect(api.packInstall).toHaveBeenCalledWith(
        'neo-server',
        expect.objectContaining({
            mrpackUrl: 'https://example.com/pack.mrpack',
            jarUrl: undefined,
            jarFilename: undefined,
        }),
    )
    await resumeModpackInstall({
        id: 'neo-server',
        serverType: 'neoforge',
        gameVersion: '1.21.1',
        loaderVersion: '21.1.172',
        modpack: { projectId: 'project', versionId: 'pack-version', title: 'Neo pack' },
    } as ServerInfoData)
    expect(api.packInstall).toHaveBeenCalledTimes(2)
    expect(api.loaders).not.toHaveBeenCalled()
})

it('retries an interrupted standalone NeoForge install using its recorded loader and Java', async () => {
    let lifecycle!: ReturnType<typeof useServerLifecycle>
    const error = vi.fn()
    wrapper = await mountThemed(
        defineComponent({
            setup() {
                lifecycle = useServerLifecycle()
                return () => null
            },
        }),
        {},
        'dark',
        {
            global: {
                provide: {
                    [Symbol.for('modrinth:notificationManager')]: {
                        handleError: error,
                        addNotification: vi.fn(),
                    },
                },
            },
        },
    )
    await lifecycle.resumeInstall({
        id: 'neo-retry',
        name: 'NeoForge',
        serverType: 'neoforge',
        gameVersion: '1.21.1',
        loaderVersion: '21.1.172',
        javaPath: 'saved-java',
        memoryMb: 4096,
    } as ServerView)
    expect(api.install).toHaveBeenCalledWith('neo-retry', '1.21.1', '21.1.172', 'saved-java')
    expect(api.packInstall).not.toHaveBeenCalled()
    expect(error).not.toHaveBeenCalled()
})
