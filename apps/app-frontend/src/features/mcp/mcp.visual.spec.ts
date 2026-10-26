import { provideI18n } from '@modrinth/ui'
import { afterEach, expect, it } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, nextTick, ref } from 'vue'

import McpSettings from '@/components/ui/settings/McpSettings.vue'
import chinese from '@/locales/zh-CN/index.json'
import { mountThemed } from '@/test/visual-harness'

import McpConnectionOverlay from './McpConnectionOverlay.vue'
import McpConnectionStatus from './McpConnectionStatus.vue'
import McpDebugSettings from './McpDebugSettings.vue'
import { useMcpPreview } from './preview'

const cleanup: (() => void)[] = []
const preview = useMcpPreview()

afterEach(() => {
    cleanup.splice(0).forEach((dispose) => dispose())
    preview.setFullAccess(false)
})

async function render(locale = 'en-US', theme: 'light' | 'dark' = 'dark', width = 900) {
    preview.setFullAccess(false)
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
        .element(page.getByText('Codex is requesting access to your launcher.'))
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
    expect(preview.fullAccess.value).toBe(true)
    preview.simulateConnection('Codex')
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
    expect(preview.fullAccess.value).toBe(false)
    expect(preview.connectedClient.value).toBeNull()
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
        expect(wrapper.text()).toContain('MCP 连接暂不可用')
        for (const name of ['Codex', 'Claude Code', 'Cursor', 'VS Code']) {
            await page.getByRole('button', { name, exact: true }).click()
            expect(wrapper.get('a[target="_blank"]').attributes('href')).toMatch(/^https:\/\//)
            expect(wrapper.element.scrollWidth).toBeLessThanOrEqual(wrapper.element.clientWidth)
        }
        wrapper.unmount()
        cleanup.pop()
    }
})
