import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'

import chinese from '@/locales/zh-CN/index.json'

import { diagnosticReport, type NetworkTestResult, resultStatus } from './network-diagnostics'
import NetworkConnectionSettings from './NetworkConnectionSettings.vue'

const calls = vi.hoisted(() => ({ invoke: vi.fn(), chinese: false, clipboard: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: calls.invoke }))
vi.mock('@modrinth/ui', async () => ({
    Button: (await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    Card: (await import('../../../../../../packages/ui/src/components/base/Card.vue')).default,
    Combobox: (await import('../../../../../../packages/ui/src/components/base/Combobox.vue'))
        .default,
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (
            message: { id: string; defaultMessage: string },
            values?: Record<string, string>,
        ) => {
            const text = calls.chinese
                ? ((chinese as Record<string, { message: string }>)[message.id]?.message ??
                  message.defaultMessage)
                : message.defaultMessage
            return text.replace('{name}', values?.name ?? '')
        },
    }),
}))

const result = (extra: Partial<NetworkTestResult> = {}): NetworkTestResult => ({
    target: 'modrinth_cdn',
    mode: 'launcher',
    tested_at: '2026-10-18T12:00:00Z',
    launcher_version: '1.10.0-beta.2',
    platform: 'windows-x86_64',
    url: 'https://cdn.modrinth.com/data/project/versions/version/file.jar',
    proxy_mode: 'system',
    dns_mode: 'doh',
    tls_verification: true,
    resolved_ips: ['151.101.1.242', '151.101.65.242', '2a04:4e42:600::754'],
    dns_ms: 31,
    dns_error: null,
    peer_ip: '2a04:4e42:600::754',
    status: 206,
    response_ms: 243,
    total_ms: 640,
    http_version: 'HTTP/2.0',
    server: 'varnish',
    served_by: 'cache-nrt-rjtf7700100-NRT',
    cache: 'HIT',
    bytes_received: 982671,
    sample_ms: 366,
    sample_mib_per_second: 2.56,
    error: null,
    ...extra,
})

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.restoreAllMocks()
    calls.invoke.mockReset()
    calls.clipboard.mockReset()
    calls.chinese = false
})

async function render(theme = 'dark', width = 900) {
    if (!document.querySelector('#teleports')) {
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        document.body.append(teleports)
    }
    document.documentElement.classList.remove('light-mode', 'dark-mode', 'oled-mode')
    document.documentElement.classList.add(`${theme}-mode`)
    Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: { writeText: calls.clipboard },
    })
    const wrapper = mount(NetworkConnectionSettings, { attachTo: document.body })
    wrapper.element.setAttribute(
        'style',
        `width: min(${width}px, 100%); padding: 24px; box-sizing: border-box; background: var(--surface-2)`,
    )
    wrappers.push(wrapper)
    return wrapper
}

it('tests a target once, displays DNS separately from the peer, and copies the measured report', async () => {
    calls.invoke.mockResolvedValue(result())
    const wrapper = await render()
    await wrapper.get('[aria-label="Test Modrinth CDN"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('243'))
    expect(calls.invoke).toHaveBeenCalledTimes(1)
    expect(calls.invoke.mock.calls[0][1]).toMatchObject({
        target: 'modrinth_cdn',
        mode: 'launcher',
    })
    await wrapper.get('details summary').trigger('click')
    expect(wrapper.text()).toContain('151.101.1.242')
    expect(wrapper.text()).toContain('2a04:4e42:600::754')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Copy report')!
        .trigger('click')
    expect(
        JSON.parse(calls.clipboard.mock.calls[0][0]).tests.find(
            (test: { target: string }) => test.target === 'modrinth_cdn',
        ).result.status,
    ).toBe(206)
    expect(wrapper.text()).toContain('Report copied')
})

it('cancels queued tests and discards a late reply before a new run', async () => {
    let finish!: (value: NetworkTestResult) => void
    calls.invoke.mockImplementation((command: string) =>
        command.endsWith('network_test_cancel')
            ? Promise.resolve()
            : new Promise((resolve) => {
                  finish = resolve
              }),
    )
    const wrapper = await render()
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Test all services')!
        .trigger('click')
    expect(wrapper.findAll('[data-status="running"]')).toHaveLength(1)
    expect(wrapper.findAll('[data-status="queued"]')).toHaveLength(4)
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Stop testing')!
        .trigger('click')
    expect(wrapper.findAll('[data-status="cancelled"]')).toHaveLength(5)
    finish(result())
    await vi.waitFor(() => expect(calls.invoke).toHaveBeenCalledTimes(2))
    expect(wrapper.findAll('[data-status="cancelled"]')).toHaveLength(5)
    calls.invoke.mockResolvedValue(result({ response_ms: 67 }))
    await wrapper.get('[aria-label="Test Modrinth CDN"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('67'))
    expect(wrapper.findAll('[data-status="success"]')).toHaveLength(1)
})

it('continues after an IPC failure and reports HTTP rejection without claiming a timeout', async () => {
    calls.invoke
        .mockRejectedValueOnce(new Error('IPC unavailable'))
        .mockImplementation((_command, args) =>
            Promise.resolve(result({ target: args.target, status: 403, response_ms: 117 })),
        )
    const wrapper = await render()
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Test all services')!
        .trigger('click')
    await vi.waitFor(() => expect(wrapper.findAll('[data-status="restricted"]')).toHaveLength(4))
    expect(wrapper.findAll('[data-status="failed"]')).toHaveLength(1)
    expect(wrapper.text()).toContain('refused this request')
    expect(resultStatus(result({ error: 'timeout', status: null }))).toBe('failed')
    expect(resultStatus(result({ status: 500 }))).toBe('http_error')
})

it('omits stale results from cancelled reports and cancels when leaving the page', async () => {
    const report = JSON.parse(
        diagnosticReport(
            Object.fromEntries(
                [
                    'curseforge_api',
                    'modrinth_api',
                    'modrinth_cdn',
                    'mojang_api',
                    'minecraft_services',
                ].map((target) => [target, { state: 'cancelled', result: result() }]),
            ) as Parameters<typeof diagnosticReport>[0],
        ),
    )
    expect(report.tests.every((test: { result?: unknown }) => test.result === undefined)).toBe(true)
    calls.invoke.mockReturnValue(new Promise(() => {}))
    const wrapper = await render()
    await wrapper.get('[aria-label="Test Mojang API"]').trigger('click')
    wrapper.unmount()
    wrappers.splice(wrappers.indexOf(wrapper), 1)
    expect(
        calls.invoke.mock.calls.some(
            ([command]) => command === 'plugin:settings|network_test_cancel',
        ),
    ).toBe(true)
})

it('uses direct mode when selected and exposes a clipboard failure without losing results', async () => {
    calls.invoke.mockResolvedValue(result({ mode: 'direct', proxy_mode: 'none' }))
    calls.clipboard.mockRejectedValue(new Error('Clipboard denied'))
    const wrapper = await render()
    await wrapper.get('#network-test-mode [role="button"]').trigger('keydown', { key: 'Enter' })
    await vi.waitFor(() => expect(document.querySelector('[role="listbox"]')).not.toBeNull())
    const option = Array.from(document.querySelectorAll('[role="option"]')).find((item) =>
        item.textContent?.includes('Direct connection'),
    )!
    ;(option as HTMLElement).click()
    await wrapper.get('[aria-label="Test Modrinth CDN"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.findAll('[data-status="success"]')).toHaveLength(1))
    expect(calls.invoke.mock.calls[0][1].mode).toBe('direct')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Copy report')!
        .trigger('click')
    expect(wrapper.text()).toContain('Could not copy the report')
    expect(wrapper.text()).toContain('243')
})

it('renders Chinese results, long addresses and errors in light, dark and OLED without overflow', async () => {
    calls.chinese = true
    calls.invoke.mockImplementation((_command, args) =>
        Promise.resolve(
            result({
                target: args.target,
                ...(args.target === 'curseforge_api' ? { status: 403 } : {}),
                ...(args.target === 'mojang_api'
                    ? { error: 'timeout', status: null, peer_ip: null, response_ms: null }
                    : {}),
            }),
        ),
    )
    await page.viewport(1100, 2500)
    for (const theme of ['light', 'dark', 'oled']) {
        const wrapper = await render(theme, 780)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === '测试全部服务')!
            .trigger('click')
        await vi.waitFor(() => expect(wrapper.findAll('[data-status="success"]')).toHaveLength(3))
        const details = wrapper.findAll('details')[2]
        ;(details.element as HTMLDetailsElement).open = true
        expect(wrapper.element.scrollWidth).toBeLessThanOrEqual(wrapper.element.clientWidth)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: wrapper.element,
                style: '[data-scale] { transform: none !important; }',
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/network-${theme}.png`,
            })
        wrapper.unmount()
        wrappers.splice(wrappers.indexOf(wrapper), 1)
    }
    await page.viewport(500, 1000)
    const narrow = await render('dark', 460)
    await narrow
        .findAll('button')
        .find((button) => button.text() === '测试全部服务')!
        .trigger('click')
    await vi.waitFor(() => expect(narrow.findAll('[data-status="success"]')).toHaveLength(3))
    ;(narrow.findAll('details')[2].element as HTMLDetailsElement).open = true
    await page.viewport(500, 3000)
    expect(narrow.element.scrollWidth).toBeLessThanOrEqual(narrow.element.clientWidth)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: narrow.element,
            style: '[data-scale] { transform: none !important; }',
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/network-narrow.png`,
        })
})
