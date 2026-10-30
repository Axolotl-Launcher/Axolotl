import '@modrinth/ui/directives/tooltip-styles.ts'

import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { ref } from 'vue'

import type { ServerView } from '@/composables/useServers'
import chinese from '@/locales/zh-CN/index.json'
import { applyTheme, mountThemed, waitFor } from '@/test/visual-harness'

import ServerCard from './ServerCard.vue'
import ServerDetail from './ServerDetail.vue'
import ServersOverview from './ServersOverview.vue'

const api = vi.hoisted(() => ({
    stop: vi.fn(),
    kill: vi.fn().mockResolvedValue(true),
    start: vi.fn(),
    resume: vi.fn(),
    open: vi.fn(),
    list: [] as ServerView[],
    loaded: true,
    refreshing: false,
    error: null as Error | null,
}))
vi.mock('@/composables/useServers', async () => {
    const { ref } = await import('vue')
    return {
        useServers: () => ({
            servers: ref(api.list),
            isRefreshing: ref(api.refreshing),
            hasLoaded: ref(api.loaded),
            loadError: ref(api.error),
            refresh: vi.fn(),
            stopServer: api.stop,
            killServer: api.kill,
        }),
    }
})
vi.mock('@/composables/useServerLifecycle', async () => {
    const { ref } = await import('vue')
    return {
        useServerLifecycle: () => ({
            eulaModal: ref(null),
            eulaText: ref(''),
            tryStartServer: api.start,
            acceptEula: vi.fn(),
            declineEula: vi.fn(),
            resumeInstall: api.resume,
        }),
    }
})
vi.mock('@/composables/useServerInstalls', () => ({
    serverSetupStatus: (server: ServerView) =>
        server.id === 'installing'
            ? 'installing'
            : server.installState === 'failed'
              ? 'failed'
              : null,
}))
vi.mock('@/composables/useMultiplayerSession', async () => {
    const { ref } = await import('vue')
    return {
        useMultiplayerSession: () => ({
            isActionPending: ref(false),
            isStopping: ref(false),
            activeProvider: ref(null),
        }),
    }
})
vi.mock('vue-router', async (original) => ({
    ...(await original<typeof import('vue-router')>()),
    useRoute: () => ({ params: { id: 'running' }, query: {} }),
    useRouter: () => ({ push: api.open, replace: api.open }),
}))
vi.mock('@/helpers/servers', () => ({ servers: { portProcess: async () => null } }))
vi.mock('@/helpers/utils', () => ({ openPath: vi.fn() }))
vi.mock('@/components/multiplayer/servers/CreateServerModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/multiplayer/servers/EulaModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/multiplayer/servers/ServerConsole.vue', async () => {
    const { h } = await import('vue')
    return {
        default: {
            render: () =>
                h(
                    'div',
                    {
                        style: 'height:100%;min-height:100px;background:var(--surface-2);border-radius:12px;padding:16px;font-family:monospace',
                    },
                    '[19:53:22] [Server thread/INFO]: Done! For help, type "help"',
                ),
        },
    }
})
vi.mock('@/components/multiplayer/servers/ServerFilesPanel.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/multiplayer/servers/ServerSettingsPanel.vue', () => ({
    default: { render: () => null },
}))

const wrappers: VueWrapper[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    document.body.replaceChildren()
    vi.clearAllMocks()
    api.loaded = true
    api.refreshing = false
    api.error = null
})
function data(): ServerView[] {
    return [
        {
            id: 'running',
            name: '朋友们的生存世界',
            serverType: 'fabric',
            gameVersion: '26.1.2',
            loaderVersion: '0.19.5',
            status: 'running',
            running: true,
            port: 25565,
        },
        {
            id: 'ready',
            name: '周末建筑 · Creative',
            serverType: 'paper',
            gameVersion: '1.21.11',
            status: 'ready',
            running: false,
            port: 25566,
        },
        {
            id: 'failed',
            name: '自动化工厂与冒险整合包服务器名称很长用于检查标题与状态是否拥挤',
            serverType: 'forge',
            gameVersion: '1.20.1',
            status: 'crashed',
            running: false,
            installState: 'failed',
        },
        {
            id: 'starting',
            name: '新建 Fabric 服务器',
            serverType: 'fabric',
            gameVersion: '26.1.2',
            status: 'starting',
            starting: true,
            running: false,
            port: 25567,
        },
    ].map((server) => ({
        path: 'E:/Servers/example',
        eulaAccepted: true,
        eulaExists: true,
        ...server,
    })) as ServerView[]
}
async function render(
    component: typeof ServersOverview | typeof ServerDetail | typeof ServerCard,
    props: Record<string, unknown> = {},
) {
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    const host = document.createElement('div')
    host.style.cssText = 'padding:24px;min-height:100vh;background:var(--surface-1)'
    document.body.append(host)
    const wrapper = await mountThemed(component, props, 'dark', {
        attachTo: host,
        global: {
            directives: { tooltip: tooltipDirective },
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('zh-CN'),
                    setLocale: () => {},
                    t: (key: string, values?: Record<string, unknown>) => {
                        const text = (chinese as Record<string, { message: string }>)[key]?.message
                        return text
                            ? String(new IntlMessageFormat(text, 'zh-CN').format(values))
                            : key
                    },
                },
                [Symbol.for('modrinth:notificationManager')]: { addNotification: vi.fn() },
                [Symbol.for('modrinth:FilePickerContext')]: {},
            },
        },
    })
    wrappers.push(wrapper)
    await flushPromises()
    return { wrapper, host }
}

it('keeps server identities and actions inside the management viewport', async () => {
    api.list = data()
    localStorage.setItem('axolotl-library-display-mode', 'standard')
    const { wrapper, host } = await render(ServersOverview)
    for (const [width, theme] of [
        [1100, 'dark'],
        [720, 'dark'],
        [420, 'light'],
    ] as const) {
        await page.viewport(width, 780)
        applyTheme(theme)
        await flushPromises()
        await waitFor(() => wrapper.element.getAnimations({ subtree: true }).length === 0)
        expect(wrapper.element.scrollWidth).toBeLessThanOrEqual(wrapper.element.clientWidth + 1)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: host,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/servers-${width}-${theme}.png`,
            })
    }
})

it('keeps open and lifecycle actions separate and exposes them without hovering in both views', async () => {
    for (const variant of ['standard', 'library'] as const) {
        const server = data()[0]
        const { wrapper } = await render(ServerCard, { server, variant })
        const action = wrapper.findAll('button').find((button) => button.text() === '停止')!
        expect(getComputedStyle(action.element).opacity).toBe('1')
        await action.trigger('click')
        expect(wrapper.emitted('start-stop')).toHaveLength(1)
        expect(wrapper.emitted('open')).toBeUndefined()
        await wrapper.get('button[aria-label="管理 朋友们的生存世界"]').trigger('click')
        expect(wrapper.emitted('open')).toHaveLength(1)
        await wrapper.setProps({ server: { ...server, stopping: true } })
        const stopping = wrapper.findAll('button').find((button) => button.text() === '正在停止…')!
        expect(stopping.attributes('disabled')).toBeDefined()
        await stopping.trigger('click')
        expect(wrapper.emitted('start-stop')).toHaveLength(1)
        const terminate = wrapper.findAll('button').find((button) => button.text() === '终止')!
        await terminate.trigger('click')
        expect(api.kill).toHaveBeenCalledWith(server.id)
        expect(wrapper.emitted('open')).toHaveLength(1)
        await wrapper.setProps({ server: data()[2] })
        const retry = wrapper.findAll('button').find((button) => button.text() === '重试下载')!
        await retry.trigger('click')
        expect(wrapper.emitted('resume')).toHaveLength(1)
        expect(wrapper.find('button button').exists()).toBe(false)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE && variant === 'library')
            await page.screenshot({
                element: wrapper.element,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/server-library-card.png`,
            })
    }
})

it('filters by name, loader and version, and recovers from an empty search', async () => {
    api.list = data()
    const { wrapper } = await render(ServersOverview)
    const search = wrapper.get('input')
    await search.setValue('paper')
    expect(wrapper.findAllComponents(ServerCard)).toHaveLength(1)
    expect(wrapper.text()).toContain('周末建筑')
    await search.setValue('26.1.2')
    expect(wrapper.findAllComponents(ServerCard)).toHaveLength(2)
    await search.setValue('不存在的名字')
    expect(wrapper.findAllComponents(ServerCard)).toHaveLength(0)
    expect(wrapper.text()).toContain('没有匹配的服务器')
    const clear = wrapper.findAll('button').find((button) => button.text() === '清除搜索')!
    await clear.trigger('click')
    expect(wrapper.findAllComponents(ServerCard)).toHaveLength(4)
})

it('keeps creation available in an empty library and renders loading and failure separately', async () => {
    api.list = []
    const { wrapper } = await render(ServersOverview)
    expect(wrapper.text()).toContain('还没有服务器')
    expect(
        wrapper.findAll('button').filter((button) => button.text() === '创建服务器'),
    ).toHaveLength(1)
    api.loaded = false
    api.refreshing = true
    const { wrapper: loading } = await render(ServersOverview)
    expect(loading.findAllComponents(ServerCard)).toHaveLength(0)
    expect(loading.text()).not.toContain('还没有服务器')
    api.refreshing = false
    api.error = new Error('offline')
    const { wrapper: failed } = await render(ServersOverview)
    expect(failed.text()).toContain('无法读取服务器')
    expect(failed.text()).not.toContain('加载中')
    expect(failed.text()).not.toContain('还没有服务器')
})

it('shows an active download as status without an inert duplicate action', async () => {
    const server = { ...data()[1], id: 'installing' }
    const { wrapper } = await render(ServerCard, { server, variant: 'standard' })
    expect(wrapper.text()).toContain('下载中')
    expect(wrapper.findAll('button')).toHaveLength(1)
    expect(wrapper.get('button').attributes('aria-label')).toContain('管理')
})

it('keeps the detail heading and primary controls visible with a long server name', async () => {
    api.list = data()
    api.list[0].name += ' · 长名称测试服务器'
    const { wrapper, host } = await render(ServerDetail)
    for (const width of [1100, 720]) {
        await page.viewport(width, 780)
        await flushPromises()
        expect(wrapper.element.scrollWidth).toBeLessThanOrEqual(wrapper.element.clientWidth + 1)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: host,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/server-detail-${width}.png`,
            })
    }
})
