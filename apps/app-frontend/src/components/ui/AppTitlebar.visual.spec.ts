import {
    provideI18n,
    provideNotificationManager,
    providePopupNotificationManager,
} from '@modrinth/ui'
import { createPinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'
import { page, userEvent } from 'vitest/browser'
import { computed, defineComponent, h, nextTick, ref, Suspense } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import { useMcpPreview } from '@/features/mcp/preview'
import { mountThemed, waitFor } from '@/test/visual-harness'

import AppActionBar from './AppActionBar.vue'
import AppTitlebar from './AppTitlebar.vue'
import Breadcrumbs from './Breadcrumbs.vue'

const fixture = vi.hoisted(() => ({
    instanceName: '我的整合包实例名称非常长'.repeat(20),
    openAgentWindow: vi.fn(async () => {}),
    handleError: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@tauri-apps/api/core')>()),
    invoke: (command: string, args: unknown) =>
        command === 'plugin:agent-window|open_agent_window'
            ? fixture.openAgentWindow(args)
            : Promise.resolve(),
}))
vi.mock('@/helpers/process', () => ({
    get_all: async () => [
        { uuid: 'process', instance_id: 'demo' },
        { uuid: 'process-2', instance_id: 'demo' },
    ],
    kill: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({
    get_many: async () => [{ id: 'demo', name: fixture.instanceName }],
}))
vi.mock('@/helpers/events', () => ({
    loading_listener: async () => vi.fn(),
    process_listener: async () => vi.fn(),
}))
vi.mock('@/helpers/instance-backup', () => ({
    listBackupOperations: async () => [
        {
            id: 'backup',
            operation_type: 'create',
            instance_id: 'demo',
            state: 'hashing',
            processed_bytes: 0,
            total_bytes: 100,
        },
    ],
    listenBackupProgress: async () => vi.fn(),
}))
vi.mock('@/helpers/state', () => ({ progress_bars_list: async () => ({}) }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/remote-announcements', () => ({
    OPEN_REMOTE_ANNOUNCEMENT_CENTER_EVENT: 'open-announcements',
    REMOTE_ANNOUNCEMENTS_UPDATED_EVENT: 'announcements-updated',
}))
vi.mock('@/composables/useNetworkStatus', () => ({
    useNetworkStatus: () => ({ offline: ref(true) }),
}))
vi.mock('@/providers/download-manager', () => ({
    injectDownloadManager: () => ({
        activeJobs: ref([
            {
                job_id: 'download',
                status: 'running',
                phase: 'downloading_content',
                display: { title: '一个非常长的下载任务名称'.repeat(20) },
                progress: { current: 1, total: 10, unit: 'bytes' },
                summary: {
                    bytes_downloaded: 1,
                    bytes_total: 10,
                    speed_bytes_per_second: 123456789,
                    files_completed: 0,
                    files_total: 1,
                },
            },
        ]),
        legacyDownloads: ref([]),
        cancel: vi.fn(),
    }),
}))
vi.mock(import('@/composables/browse/install-job-notifications'), () => ({
    useInstallJobNotifications: async () => ({
        active: computed(() => false),
        hasItems: computed(() => false),
        title: computed(() => 'Installs'),
        progressItems: computed(() => []),
        stateSignature: computed(() => ''),
        buttons: computed(() => undefined),
        refresh: vi.fn(async () => {}),
        dispose: vi.fn(),
    }),
}))
vi.mock('@/providers/app-update', () => ({
    appUpdateState: {
        downloading: ref(false),
        downloadPercent: ref(100),
        downloadProgress: ref(1),
        finishedDownloading: ref(true),
        isVisible: ref(true),
        metered: ref(false),
        restarting: ref(false),
    },
    downloadAvailableAppUpdate: vi.fn(),
    installAvailableAppUpdate: vi.fn(),
}))

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((dispose) => dispose()))

it('keeps the connected AI control and download actions accessible together', async () => {
    const { wrapper, width } = await mountTitlebar(true)
    const preview = useMcpPreview()
    cleanup.push(preview.disconnect)
    preview.requestConnection('Codex', true)
    for (const windowWidth of [1100, 1280]) {
        width.value = windowWidth
        await page.viewport(windowWidth, 720)
        await nextTick()
        const header = wrapper.getComponent(AppTitlebar).element as HTMLElement
        expect(header.scrollWidth).toBeLessThanOrEqual(header.clientWidth)
        const connection = wrapper.get('[data-testid="mcp-connection-status"]').element
        const rect = connection.getBoundingClientRect()
        expect(
            connection.contains(
                document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2),
            ),
        ).toBe(true)
        expect(wrapper.get('button[aria-label="Downloads"]').isVisible()).toBe(true)
    }
    await wrapper.get('[data-testid="mcp-connection-status"]').trigger('click')
    expect(preview.connectedClient.value).toBeNull()
})

it('uses the same resting icon color for agent, downloads and quiet titlebar actions', async () => {
    const { wrapper } = await mountTitlebar(true)
    await waitFor(() => wrapper.find('button[aria-label="Announcements"]').exists())
    const announcement = wrapper.get('button[aria-label="Announcements"]').element
    const quietColor = getComputedStyle(announcement).color
    const agent = wrapper.get('[data-testid="agent-window-button"]').element
    expect(agent.getBoundingClientRect().width).toBe(announcement.getBoundingClientRect().width)
    expect(agent.getBoundingClientRect().height).toBe(announcement.getBoundingClientRect().height)
    for (const selector of [
        '[data-testid="agent-window-button"]',
        'button[aria-label="Downloads"]',
    ]) {
        expect(getComputedStyle(wrapper.get(selector).element).color).toBe(quietColor)
    }
})

async function mountTitlebar(withActionBar = false) {
    await page.viewport(1100, 720)
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/instance/demo', component: { render: () => null } },
            {
                path: '/browse',
                component: { render: () => null },
                meta: { breadcrumb: [{ name: 'Discover content' }] },
            },
            {
                path: '/project/jei',
                component: { render: () => null },
                meta: {
                    breadcrumb: [
                        { name: 'Discover content', link: '/browse', query: { i: 'demo' } },
                        { name: 'Just Enough Items (JEI) / 一个很长的模组名称'.repeat(8) },
                    ],
                },
            },
        ],
    })
    await router.push('/browse?i=demo')
    await router.push('/project/jei?i=demo')
    const width = ref(1100)
    const Harness = defineComponent({
        setup() {
            provideI18n({ locale: ref('en-US'), t: (key) => key, setLocale: () => {} })
            provideNotificationManager({
                getNotifications: () => [
                    { id: 'notification', title: 'Notification', read: false },
                ],
                handleError: fixture.handleError,
            } as unknown as Parameters<typeof provideNotificationManager>[0])
            providePopupNotificationManager({
                getNotifications: () => [],
            } as unknown as Parameters<typeof providePopupNotificationManager>[0])
            return () =>
                h(
                    AppTitlebar,
                    {
                        isBetaBuild: true,
                        style: { width: `${width.value}px`, height: '3rem', paddingLeft: '5rem' },
                    },
                    {
                        default: () =>
                            withActionBar
                                ? h('div', { class: 'flex min-w-0 mr-3' }, [
                                      h(Suspense, {}, { default: () => h(AppActionBar) }),
                                  ])
                                : h(
                                      'div',
                                      { style: { width: '650px' }, 'data-test': 'actions' },
                                      'Actions',
                                  ),
                    },
                )
        },
    })
    const wrapper = await mountThemed(Harness, {}, 'light', {
        global: { plugins: [router, createPinia()], directives: { tooltip: () => {} } },
    })
    cleanup.push(() => wrapper.unmount())
    return { wrapper, router, width }
}

it('keeps both history buttons visible and clickable while breadcrumbs yield to the window width', async () => {
    const { wrapper, width } = await mountTitlebar()
    const header = wrapper.getComponent(AppTitlebar).element as HTMLElement
    const breadcrumbs = wrapper.getComponent(Breadcrumbs).element as HTMLElement
    const actions = wrapper.get('[data-test="actions"]').element
    const initialFontSize = document.documentElement.style.fontSize
    cleanup.push(() => (document.documentElement.style.fontSize = initialFontSize))

    for (const fontSize of [16, 20]) {
        document.documentElement.style.fontSize = `${fontSize}px`
        for (const windowWidth of [1100, 1280, 1740, 1100]) {
            width.value = windowWidth
            await page.viewport(windowWidth, 720)
            await nextTick()
            const headerRect = header.getBoundingClientRect()
            const actionsRect = actions.getBoundingClientRect()
            const breadcrumbRect = breadcrumbs.getBoundingClientRect()
            expect(header.scrollWidth).toBeLessThanOrEqual(header.clientWidth)
            expect(actionsRect.right).toBeLessThanOrEqual(headerRect.right)
            expect(breadcrumbRect.right).toBeLessThanOrEqual(actionsRect.left)
            for (const label of ['Back', 'Forward']) {
                const button = wrapper.get(`button[aria-label="${label}"]`).element
                const rect = button.getBoundingClientRect()
                expect(rect.width).toBeGreaterThanOrEqual(24)
                expect(rect.left).toBeGreaterThanOrEqual(headerRect.left + fontSize * 5)
                expect(rect.right).toBeLessThanOrEqual(breadcrumbRect.left)
                expect(
                    button.contains(
                        document.elementFromPoint(
                            rect.x + rect.width / 2,
                            rect.y + rect.height / 2,
                        ),
                    ),
                ).toBe(true)
            }
        }
    }
})

it('returns from project details to the instance content search and restores it on forward', async () => {
    const { wrapper, router } = await mountTitlebar()
    await wrapper.get('button[aria-label="Back"]').trigger('click')
    await waitFor(() => router.currentRoute.value.fullPath === '/browse?i=demo')
    await wrapper.get('button[aria-label="Forward"]').trigger('click')
    await waitFor(() => router.currentRoute.value.fullPath === '/project/jei?i=demo')
})

it('reveals the agent action without moving other titlebar controls and handles native open failures', async () => {
    const { wrapper } = await mountTitlebar()
    const button = wrapper.get<HTMLButtonElement>('[data-testid="agent-window-button"]')
    const label = button.get('.agent-window-label').element as HTMLElement
    const actions = wrapper.get('[data-test="actions"]').element
    const actionsLeft = actions.getBoundingClientRect().left
    expect(getComputedStyle(label).visibility).toBe('hidden')
    const idleColor = getComputedStyle(button.element).color
    await userEvent.hover(page.elementLocator(button.element))
    await waitFor(() => getComputedStyle(label).opacity === '1')
    expect(getComputedStyle(button.element).color).not.toBe(idleColor)
    expect(getComputedStyle(label).backgroundColor).not.toBe('rgba(0, 0, 0, 0)')
    const backgroundRect = label.getBoundingClientRect()
    const buttonRect = button.element.getBoundingClientRect()
    expect(backgroundRect.right).toBe(buttonRect.right)
    expect(backgroundRect.height).toBe(buttonRect.height)
    const textRange = document.createRange()
    textRange.selectNodeContents(label)
    const icon = button.get('svg').element
    const gap = icon.getBoundingClientRect().left - textRange.getBoundingClientRect().right
    expect(gap).toBeGreaterThanOrEqual(0)
    expect(gap).toBeLessThanOrEqual(6)
    expect(actions.getBoundingClientRect().left).toBe(actionsLeft)

    fixture.openAgentWindow.mockClear()
    fixture.handleError.mockClear()
    let finish: () => void = () => {}
    fixture.openAgentWindow.mockImplementationOnce(
        () => new Promise<void>((resolve) => (finish = resolve)),
    )
    await button.trigger('click')
    expect(button.element.disabled).toBe(true)
    await button.trigger('click')
    expect(fixture.openAgentWindow).toHaveBeenCalledTimes(1)
    expect(fixture.openAgentWindow).toHaveBeenCalledWith({
        appearance: expect.objectContaining({
            locale: 'en-US',
            background: expect.any(String),
            surface: expect.any(String),
            text: expect.any(String),
            accent: expect.any(String),
        }),
    })
    finish()
    await waitFor(() => !button.element.disabled)

    fixture.openAgentWindow.mockRejectedValueOnce('Could not open agent window')
    await button.trigger('click')
    await waitFor(() => !button.element.disabled)
    expect(fixture.handleError).toHaveBeenCalledWith('Could not open agent window')
})

it('keeps navigation and all active actions inside the window with long names', async () => {
    const { wrapper, width } = await mountTitlebar(true)
    await waitFor(() => wrapper.find('a[href="/instance/demo"]').exists())
    const instanceLink = wrapper.get('a[href="/instance/demo"]').element as HTMLElement
    const header = wrapper.getComponent(AppTitlebar).element as HTMLElement
    expect(instanceLink.title).toBe(fixture.instanceName)
    expect(instanceLink.scrollWidth).toBeGreaterThan(instanceLink.clientWidth)
    expect(instanceLink.getBoundingClientRect().width).toBeLessThanOrEqual(192)
    const initialFontSize = document.documentElement.style.fontSize
    cleanup.push(() => (document.documentElement.style.fontSize = initialFontSize))
    for (const fontSize of [16, 20]) {
        document.documentElement.style.fontSize = `${fontSize}px`
        for (const windowWidth of [1100, 1280, 1740, 1100]) {
            width.value = windowWidth
            await page.viewport(windowWidth, 720)
            await nextTick()
            expect(header.scrollWidth, `all actions at ${fontSize}px`).toBeLessThanOrEqual(
                header.clientWidth,
            )
            expect(
                instanceLink.getBoundingClientRect().width,
                `instance link at ${windowWidth}px / ${fontSize}px`,
            ).toBeGreaterThanOrEqual(2 * fontSize)
            const download = wrapper.get('button[aria-label="Downloads"]').element as HTMLElement
            expect(
                download.scrollWidth,
                `download contents at ${windowWidth}px / ${fontSize}px`,
            ).toBeLessThanOrEqual(download.clientWidth)
            const downloadRect = download.getBoundingClientRect()
            for (const control of download.querySelectorAll(
                ':scope > svg, :scope > span:last-child',
            )) {
                const rect = control.getBoundingClientRect()
                expect(rect.width).toBeGreaterThanOrEqual(20)
                expect(rect.left).toBeGreaterThanOrEqual(downloadRect.left)
                expect(rect.right).toBeLessThanOrEqual(downloadRect.right)
            }
            const actions = wrapper.getComponent(AppActionBar).findAll('button')
            let previousRight = 0
            for (const action of actions) {
                const rect = action.element.getBoundingClientRect()
                expect(rect.width).toBeGreaterThanOrEqual(20)
                expect(rect.left).toBeGreaterThanOrEqual(previousRight)
                expect(rect.right).toBeLessThanOrEqual(header.getBoundingClientRect().right)
                expect(
                    action.element.contains(
                        document.elementFromPoint(
                            rect.x + rect.width / 2,
                            rect.y + rect.height / 2,
                        ),
                    ),
                ).toBe(true)
                previousRight = rect.right
            }
            for (const label of ['Back', 'Forward']) {
                const button = wrapper.get(`button[aria-label="${label}"]`).element
                const rect = button.getBoundingClientRect()
                expect(
                    button.contains(
                        document.elementFromPoint(
                            rect.x + rect.width / 2,
                            rect.y + rect.height / 2,
                        ),
                    ),
                ).toBe(true)
            }
        }
    }
})
