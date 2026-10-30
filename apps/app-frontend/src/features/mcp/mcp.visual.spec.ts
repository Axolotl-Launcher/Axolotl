import { provideI18n } from '@modrinth/ui'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, nextTick, ref } from 'vue'

import McpSettings from '@/components/ui/settings/McpSettings.vue'
import chinese from '@/locales/zh-CN/index.json'
import { mountThemed } from '@/test/visual-harness'

import McpConnectionOverlay from './McpConnectionOverlay.vue'
import McpConnectionStatus from './McpConnectionStatus.vue'
import McpDebugSettings from './McpDebugSettings.vue'
import { useMcpPreview } from './preview'
import { type McpState, startMcpStateSync, useMcpRuntime } from './runtime'

const native = vi.hoisted(() => ({
    revision: 0,
    connection: null as McpState['connection'],
    enabled: false,
    fullAccess: false,
    requireAuth: false,
    fail: false,
    listener: undefined as ((event: { payload: McpState }) => void) | undefined,
}))

vi.mock('@tauri-apps/api/event', () => ({
    listen: async (_name: string, listener: (event: { payload: McpState }) => void) => {
        native.listener = listener
        return () => {
            native.listener = undefined
        }
    },
}))
vi.mock('@tauri-apps/api/core', () => ({
    invoke: async (command: string, args?: Record<string, unknown>) => {
        if (native.fail) throw new Error('MCP configuration failed')
        if (command.endsWith('mcp_configure')) {
            native.enabled = !!args?.enabled
            native.fullAccess = !!args?.fullAccess
            native.requireAuth = !!args?.requireAuth
            native.connection = null
        }
        if (command.endsWith('mcp_disconnect')) native.connection = null
        if (command.endsWith('mcp_respond') && native.connection) {
            native.connection.connected = true
            native.connection.awaiting_approval = false
        }
        return nativeSnapshot()
    },
}))

function nativeSnapshot(): McpState {
    return {
        revision: ++native.revision,
        enabled: native.enabled,
        running: native.enabled,
        full_access: native.fullAccess,
        require_auth: native.requireAuth,
        connection: native.connection ? { ...native.connection } : null,
        endpoint: native.enabled ? 'http://127.0.0.1:5219/mcp' : null,
        token: native.enabled && native.requireAuth ? 'local-test-token' : null,
        error: null,
    }
}

const cleanup: (() => void)[] = []
const preview = useMcpPreview()

afterEach(() => {
    cleanup.splice(0).forEach((dispose) => dispose())
    preview.setFullAccess(false)
})

async function render(locale = 'en-US', theme: 'light' | 'dark' = 'dark', width = 900) {
    preview.setFullAccess(false)
    native.enabled = false
    native.fullAccess = false
    native.requireAuth = false
    native.connection = null
    native.fail = false
    const stop = await startMcpStateSync()
    cleanup.push(stop)
    await page.viewport(width, 1000)
    const Harness = defineComponent({
        setup() {
            provideI18n({
                locale: ref(locale),
                t: (key) =>
                    locale === 'zh-CN'
                        ? ((chinese as Record<string, { message: string }>)[key]?.message ?? key)
                        : key,
                setLocale: () => {},
            })
            return () =>
                h(
                    'div',
                    {
                        class: 'flex min-w-0 flex-col gap-6 bg-surface-2 p-6',
                        style: { width: '100%', boxSizing: 'border-box' },
                    },
                    [
                        h(McpSettings),
                        h(McpDebugSettings),
                        h(McpConnectionStatus),
                        h(McpConnectionOverlay, { rounded: true }),
                    ],
                )
        },
    })
    const wrapper = await mountThemed(Harness, {}, theme, {
        global: { directives: { tooltip: () => {} }, stubs: { transition: false } },
    })
    cleanup.push(() => wrapper.unmount())
    return wrapper
}

it('requires approval and clears both connection effects on disconnect', async () => {
    const wrapper = await render()
    preview.simulateConnection('Claude Code')
    await expect.element(page.getByRole('dialog')).toBeVisible()
    expect(document.querySelector('[data-testid="mcp-connection-border"]')).toBeNull()
    expect(wrapper.find('[data-testid="mcp-connection-status"]').exists()).toBe(false)
    await page.getByRole('button', { name: 'Allow connection', exact: true }).click()
    await expect
        .element(page.getByRole('button', { name: 'Disconnect Claude Code', exact: true }))
        .toBeVisible()
    const border = document.querySelector('[data-testid="mcp-connection-border"]')!
    expect(wrapper.find('[data-testid="mcp-full-access-shield"]').exists()).toBe(false)
    expect(getComputedStyle(border).pointerEvents).toBe('none')
    expect(getComputedStyle(border, '::before').maskImage).toContain('linear-gradient')
    await page.getByRole('button', { name: 'Disconnect Claude Code', exact: true }).click()
    await expect
        .poll(() => document.querySelector('[data-testid="mcp-connection-border"]'))
        .toBeNull()
    expect(preview.connectedClient.value).toBeNull()
})

it('covers all four viewport edges above the title bar even inside a clipped shell', async () => {
    const wrapper = await render()
    const shell = wrapper.element as HTMLElement
    shell.style.clipPath = 'inset(0 round 8px)'
    shell.style.transform = 'translate(16px, 16px)'
    preview.setFullAccess(true)
    preview.simulateConnection('Codex')
    await expect
        .poll(() => document.querySelector('[data-testid="mcp-connection-border"]'))
        .not.toBeNull()
    const border = document.querySelector('[data-testid="mcp-connection-border"]')!
    const rect = border.getBoundingClientRect()
    expect(border.parentElement).toBe(document.body)
    expect([rect.left, rect.top, rect.right, rect.bottom]).toEqual([
        0,
        0,
        window.innerWidth,
        window.innerHeight,
    ])
    const style = getComputedStyle(border)
    expect([
        style.borderTopWidth,
        style.borderRightWidth,
        style.borderBottomWidth,
        style.borderLeftWidth,
    ]).toEqual(['0px', '0px', '0px', '0px'])
    expect(style.boxShadow).toContain('inset')
    expect([
        style.borderTopLeftRadius,
        style.borderTopRightRadius,
        style.borderBottomRightRadius,
        style.borderBottomLeftRadius,
    ]).toEqual(['8px', '8px', '8px', '8px'])
    expect(Number(style.zIndex)).toBeGreaterThan(200)
    const glow = getComputedStyle(border, '::before')
    expect(glow.maskImage.match(/linear-gradient/g)).toHaveLength(4)
    expect(glow.maskSize).toBe('100% 40px, 40px 100%, 100% 40px, 40px 100%')
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        expect(glow.animationName).toBe('none')
        expect(Number(glow.opacity)).toBe(0.4)
    } else {
        await expect
            .poll(() => getComputedStyle(border, '::before').animationName)
            .toContain('mcp-connection-breathe')
        const breathingGlow = getComputedStyle(border, '::before')
        expect(breathingGlow.animationDuration).toBe('4s')
        expect(Number(breathingGlow.opacity)).toBeGreaterThanOrEqual(0.36)
        expect(Number(breathingGlow.opacity)).toBeLessThanOrEqual(0.46)
    }
    const originalFontSize = document.documentElement.style.fontSize
    try {
        document.documentElement.style.fontSize = '20px'
        expect(getComputedStyle(border).borderTopLeftRadius).toBe('8px')
    } finally {
        document.documentElement.style.fontSize = originalFontSize
    }
})

it('peaks on arrival, settles into breathing, and can reconnect during the exit fade', async () => {
    await render()
    preview.setFullAccess(true)
    preview.simulateConnection('Codex')
    await nextTick()
    const border = document.querySelector('[data-testid="mcp-connection-border"]')!
    if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        await expect
            .poll(() =>
                border
                    .getAnimations({ subtree: true })
                    .some((animation) =>
                        (animation as CSSAnimation).animationName?.includes(
                            'mcp-connection-arrive',
                        ),
                    ),
            )
            .toBe(true)
        const arrival = border
            .getAnimations({ subtree: true })
            .find((animation) =>
                (animation as CSSAnimation).animationName?.includes('mcp-connection-arrive'),
            )!
        arrival.pause()
        arrival.currentTime = 120
        expect(Number(getComputedStyle(border, '::before').opacity)).toBeGreaterThan(0.8)
        arrival.currentTime = 600
        expect(Number(getComputedStyle(border, '::before').opacity)).toBeCloseTo(0.36)
        await expect
            .poll(() => getComputedStyle(border, '::before').animationName)
            .toContain('mcp-connection-breathe')
    }
    preview.disconnect()
    await nextTick()
    expect(preview.connectedClient.value).toBeNull()
    expect(document.querySelector('[data-testid="mcp-connection-border"]')).not.toBeNull()
    expect(getComputedStyle(border).transitionProperty).toBe('opacity')
    preview.simulateConnection('Cursor')
    await nextTick()
    expect(document.querySelectorAll('[data-testid="mcp-connection-border"]')).toHaveLength(1)
    expect(preview.connectedClient.value).toBe('Cursor')
    preview.disconnect()
    await expect
        .poll(() => document.querySelector('[data-testid="mcp-connection-border"]'))
        .toBeNull()
})

it('declines dismissed requests and lets a later request ask again', async () => {
    await render()
    preview.simulateConnection('Cursor')
    await page.getByRole('button', { name: 'Decline', exact: true }).click()
    expect(preview.pendingClient.value).toBeNull()
    expect(preview.connectedClient.value).toBeNull()
    preview.simulateConnection('Codex')
    await expect
        .element(page.getByText('Codex wants to start a launcher control session.'))
        .toBeVisible()
    preview.disconnect()
    await expect.element(page.getByRole('dialog')).not.toBeInTheDocument()
})

it('keeps full access off until risk acknowledgement and disconnects when permissions change', async () => {
    const wrapper = await render()
    await wrapper.get('#mcp-full-access').trigger('click')
    expect(preview.fullAccess.value).toBe(false)
    const enable = page.getByRole('button', { name: 'Enable full access', exact: true })
    await expect.element(enable).toBeDisabled()
    await page.getByRole('button', { name: 'Cancel', exact: true }).click()
    expect(preview.fullAccess.value).toBe(false)
    await wrapper.get('#mcp-full-access').trigger('click')
    await page.getByRole('checkbox').click()
    await enable.click()
    expect(useMcpRuntime().state.value.full_access).toBe(true)
    native.enabled = true
    native.connection = {
        id: 'real-codex',
        client: 'Codex',
        connected: true,
        awaiting_approval: false,
    }
    native.listener?.({ payload: nativeSnapshot() })
    await expect
        .element(page.getByRole('button', { name: 'Disconnect Codex', exact: true }))
        .toBeVisible()
    const shield = wrapper.get('[data-testid="mcp-full-access-shield"]').element
    const paletteColor = document.createElement('span')
    paletteColor.style.color = 'var(--color-orange-300)'
    document.body.append(paletteColor)
    expect(getComputedStyle(shield).color).toBe(getComputedStyle(paletteColor).color)
    paletteColor.remove()
    expect(preview.pendingClient.value).toBeNull()
    await wrapper.get('#mcp-full-access').trigger('click')
    await expect.poll(() => useMcpRuntime().state.value.full_access).toBe(false)
    expect(useMcpRuntime().state.value.connection).toBeNull()
    expect(wrapper.find('[data-testid="mcp-full-access-shield"]').exists()).toBe(false)
    await expect
        .poll(() => document.querySelector('[data-testid="mcp-connection-border"]'))
        .toBeNull()
})

it('removes effects on both task completion and connection loss', async () => {
    await render()
    preview.setFullAccess(true)
    for (const action of ['Simulate task completion', 'Simulate connection loss']) {
        await page.getByRole('button', { name: 'Simulate connection', exact: true }).click()
        await expect
            .element(page.getByRole('button', { name: 'Disconnect Preview agent', exact: true }))
            .toBeVisible()
        await page.getByRole('button', { name: action, exact: true }).click()
        await expect
            .poll(() => document.querySelector('[data-testid="mcp-connection-border"]'))
            .toBeNull()
        expect(preview.connectedClient.value).toBeNull()
    }
})

it('shows verified client instructions and contains Chinese content at a narrow width in both themes', async () => {
    for (const theme of ['dark', 'light'] as const) {
        const wrapper = await render('zh-CN', theme, 560)
        expect(wrapper.text()).toContain('完全访问模式')
        expect(wrapper.text()).toContain('启用 MCP')
        for (const name of ['Codex', 'Claude Code', 'Cursor', 'VS Code']) {
            await page.getByRole('button', { name, exact: true }).click()
            expect(wrapper.get('a[target="_blank"]').attributes('href')).toMatch(/^https:\/\//)
            expect(wrapper.element.scrollWidth).toBeLessThanOrEqual(wrapper.element.clientWidth)
        }
        wrapper.unmount()
        cleanup.pop()
    }
})

it('starts and stops the native service and preserves configuration when saving fails', async () => {
    const wrapper = await render()
    expect(wrapper.get('#mcp-enabled').attributes('aria-checked')).toBe('false')
    await wrapper.get('#mcp-enabled').trigger('click')
    await expect.poll(() => useMcpRuntime().state.value.running).toBe(true)
    expect(wrapper.text()).toContain('http://127.0.0.1:5219/mcp')
    expect(wrapper.get('#mcp-require-auth').attributes('aria-checked')).toBe('false')
    expect(wrapper.find('input[type="password"]').exists()).toBe(false)
    expect(wrapper.get('pre').text()).toContain('startup_timeout_sec = 60')
    expect(wrapper.get('pre').text()).not.toContain('Authorization')
    await wrapper.get('#mcp-require-auth').trigger('click')
    await expect.poll(() => useMcpRuntime().state.value.require_auth).toBe(true)
    expect(wrapper.get('pre').text()).toContain('Authorization = "Bearer local-test-token"')
    const writeClipboard = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue()
    try {
        await page.getByRole('button', { name: 'Copy token', exact: true }).click()
        expect(writeClipboard).toHaveBeenCalledWith('local-test-token')
        await expect
            .element(page.getByRole('button', { name: 'Token copied', exact: true }))
            .toBeVisible()
    } finally {
        writeClipboard.mockRestore()
    }
    await page.getByRole('button', { name: 'Cursor', exact: true }).click()
    expect(wrapper.get('pre').text()).toContain('Authorization')
    await wrapper.get('#mcp-require-auth').trigger('click')
    await expect.poll(() => useMcpRuntime().state.value.require_auth).toBe(false)
    expect(wrapper.get('pre').text()).not.toContain('Authorization')
    expect(useMcpRuntime().state.value.token).toBeNull()
    native.fail = true
    await wrapper.get('#mcp-enabled').trigger('click')
    await expect.element(page.getByRole('alert')).toHaveTextContent('MCP configuration failed')
    expect(wrapper.get('#mcp-enabled').attributes('aria-checked')).toBe('true')
    native.fail = false
    await wrapper.get('#mcp-enabled').trigger('click')
    await expect.poll(() => useMcpRuntime().state.value.running).toBe(false)
    expect(useMcpRuntime().state.value.token).toBeNull()
})

it('drives the confirmation and connected effects from real session events and ignores stale snapshots', async () => {
    await render()
    preview.setFullAccess(true)
    preview.simulateConnection('Preview agent')
    native.enabled = true
    native.connection = {
        id: 'real-session',
        client: 'Native Agent',
        connected: false,
        awaiting_approval: true,
    }
    const pending = nativeSnapshot()
    native.listener?.({ payload: pending })
    expect(preview.connectedClient.value).toBeNull()
    await expect
        .element(page.getByText('Native Agent wants to start a launcher control session.'))
        .toBeVisible()
    await page.getByRole('button', { name: 'Allow connection', exact: true }).click()
    await expect
        .element(page.getByRole('button', { name: 'Disconnect Native Agent', exact: true }))
        .toBeVisible()
    native.listener?.({ payload: pending })
    expect(useMcpRuntime().state.value.connection?.connected).toBe(true)
    await page.getByRole('button', { name: 'Disconnect Native Agent', exact: true }).click()
    await expect.poll(() => useMcpRuntime().state.value.connection).toBeNull()
    await expect
        .poll(() => document.querySelector('[data-testid="mcp-connection-border"]'))
        .toBeNull()
    native.listener?.({ payload: pending })
    expect(useMcpRuntime().state.value.connection).toBeNull()
})
