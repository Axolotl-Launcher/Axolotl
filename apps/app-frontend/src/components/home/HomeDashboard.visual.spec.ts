import IntlMessageFormat from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { userEvent } from 'vitest/browser'

import { mountThemed, waitFor } from '@/test/visual-harness'

import type { HomeDashboardConfig, HomeWidgetLayout } from './home-dashboard'
import HomeDashboard from './HomeDashboard.vue'

vi.mock('@/components/home/home-dashboard-runtime', () => ({
    provideHomeDashboardRuntime: vi.fn(),
}))
vi.mock('@/components/home/HomeCalendar.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomeGreeting.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomeGreetingSettingsModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/home/HomePinnedInstances.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomePinnedServers.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomePinnedWorlds.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomeRecentWorlds.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomeShortcutWidget.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/home/HomeWidgetPickerModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => ({
    Button: (await import('../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    OverflowMenu: (await import('../../../../../packages/ui/src/components/base/OverflowMenu.vue'))
        .default,
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (
            message: { defaultMessage: string },
            values?: Record<string, string | number | boolean>,
        ) => new IntlMessageFormat(message.defaultMessage, 'en-US').format(values),
    }),
}))

const cleanups: (() => void)[] = []
afterEach(() => {
    cleanups
        .splice(0)
        .reverse()
        .forEach((cleanup) => cleanup())
})

const panel = () => document.querySelector<HTMLElement>('[role="menu"]')
const settleToolbar = () => new Promise((resolve) => setTimeout(resolve, 180))

async function dashboard(layout: HomeWidgetLayout) {
    const outside = document.createElement('button')
    outside.textContent = 'Outside dashboard'
    outside.style.cssText = 'position:fixed;top:4px;left:16px'
    const host = document.createElement('div')
    host.style.cssText =
        'position:fixed;top:48px;left:16px;width:calc(100vw - 32px);max-width:680px'
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(outside, host, teleports)
    cleanups.push(() => {
        outside.remove()
        host.remove()
        teleports.remove()
    })
    const config: HomeDashboardConfig = {
        version: 1,
        layout,
        widgets: [
            {
                id: 'recent-one',
                kind: 'recent',
                size: '2x1',
                options: { recentLimit: 4 },
                position: { column: 0, row: 0 },
            },
        ],
    }
    const wrapper = await mountThemed(
        HomeDashboard,
        { config, instances: [], playerName: null, editing: true },
        'dark',
        { attachTo: host, global: { directives: { tooltip: {} } } },
    )
    cleanups.push(() => wrapper.unmount())
    const widget = wrapper.get<HTMLElement>('.home-widget').element
    const options = wrapper.get<HTMLElement>('.home-widget-options').element
    const bar = wrapper.get<HTMLElement>('.home-widget-edit-bar').element
    const trigger = wrapper.get<HTMLButtonElement>('button[aria-haspopup="menu"]').element
    return { wrapper, outside, teleports, widget, options, bar, trigger }
}

it.each(['grid', 'free'] as const)(
    'keeps the %s widget menu anchored outside its card and collapses after a mouse selection',
    async (layout) => {
        const { wrapper, outside, teleports, widget, options, bar, trigger } =
            await dashboard(layout)
        await userEvent.hover(widget)
        await waitFor(() => options.getBoundingClientRect().width > 24, {
            label: 'widget tools to expand',
        })
        await settleToolbar()
        await userEvent.click(trigger)
        await waitFor(() => !!panel() && panel()!.getBoundingClientRect().width > 0, {
            label: 'menu to open',
        })
        await settleToolbar()
        expect(teleports.contains(panel())).toBe(true)
        expect(widget.contains(panel())).toBe(false)
        const originalBar = bar.getBoundingClientRect()
        const originalPanel = panel()!.getBoundingClientRect()
        const item = Array.from(panel()!.querySelectorAll<HTMLElement>('[role="menuitem"]')).find(
            (candidate) => candidate.textContent?.trim() === 'Show 2 recent items',
        )!
        await userEvent.hover(item)
        expect(widget.matches(':hover')).toBe(false)
        // The old toolbar started collapsing only after leaving the card.
        await settleToolbar()
        expect(options.getBoundingClientRect().width).toBeGreaterThan(24)
        expect(Math.abs(bar.getBoundingClientRect().width - originalBar.width)).toBeLessThan(2)
        expect(Math.abs(panel()!.getBoundingClientRect().left - originalPanel.left)).toBeLessThan(2)
        expect(Math.abs(panel()!.getBoundingClientRect().top - originalPanel.top)).toBeLessThan(2)
        await userEvent.click(item)
        await waitFor(() => !panel(), { label: 'menu to close after selection' })
        const changes = wrapper.emitted('change')!
        expect(changes).toHaveLength(1)
        const updated = changes[0][0] as HomeDashboardConfig
        expect(updated.widgets[0].options?.recentLimit).toBe(2)
        await wrapper.setProps({ config: updated })
        await userEvent.hover(outside)
        await waitFor(() => options.getBoundingClientRect().width < 1, {
            label: 'widget tools to collapse',
        })
    },
)

it.each(['grid', 'free'] as const)(
    'keeps %s widget options reachable by keyboard and restores the trigger on Escape',
    async (layout) => {
        const { outside, options, trigger } = await dashboard(layout)
        await userEvent.click(outside)
        await userEvent.tab()
        await waitFor(() => options.getBoundingClientRect().width > 24, {
            label: 'widget tools to expand',
        })
        await userEvent.tab()
        expect(document.activeElement).toBe(trigger)
        await userEvent.keyboard('{ArrowDown}')
        await waitFor(() => document.activeElement?.getAttribute('role') === 'menuitem')
        await settleToolbar()
        expect(options.getBoundingClientRect().width).toBeGreaterThan(24)
        await userEvent.keyboard('{Escape}')
        await waitFor(() => !panel() && document.activeElement === trigger)
        expect(trigger.matches(':focus-visible')).toBe(true)
        expect(options.getBoundingClientRect().width).toBeGreaterThan(24)
        await userEvent.click(outside)
        await waitFor(() => options.getBoundingClientRect().width < 1, {
            label: 'widget tools to collapse',
        })
    },
)
