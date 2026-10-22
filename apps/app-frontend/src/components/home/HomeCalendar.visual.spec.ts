import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import IntlMessageFormat from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { mountThemed, waitFor } from '@/test/visual-harness'

import HomeCalendar from './HomeCalendar.vue'

vi.mock('@/components/home/home-dashboard-runtime', () => ({
    useHomeDashboardRuntime: () => ({ instanceRevision: ref(0), runningInstanceIds: ref([]) }),
}))
vi.mock('@/helpers/instance', () => ({
    get_daily_playtime: async (start: string) => [
        { date: start, played_seconds: 90, session_count: 2, top_instance_name: 'Example <pack>' },
    ],
    get_daily_playtime_details: async () => [],
    run: vi.fn(),
    kill: vi.fn(),
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', () => ({
    Button: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('button', slots.default?.()),
    }),
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, unknown>) =>
            new IntlMessageFormat(message.defaultMessage, 'en-US').format(values),
    }),
    useFormatDateTime: (options: Intl.DateTimeFormatOptions) => (date: Date) =>
        new Intl.DateTimeFormat('en-US', options).format(date),
}))

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)
const tip = () => document.querySelector<HTMLElement>('[role="tooltip"]')

async function calendar() {
    const host = document.createElement('div')
    host.style.cssText = 'position:fixed;top:40px;left:12px;width:270px;height:240px;overflow:auto'
    document.body.append(host)
    cleanup.push(() => host.remove())
    const wrapper = await mountThemed(HomeCalendar, { instances: [] }, 'dark', {
        attachTo: host,
        global: { directives: { tooltip: tooltipDirective } },
    })
    cleanup.push(() => wrapper.unmount())
    await waitFor(() => wrapper.findAll('[data-date-key]').length > 0)
    const trigger = wrapper.get('[data-date-key]').element as HTMLButtonElement
    trigger.focus()
    await waitFor(() => tip()?.parentElement?.dataset.state === 'open')
    return { wrapper, host, trigger }
}

it('keeps the calendar tooltip attached on scroll and layout changes, with visible bounds', async () => {
    const { host, trigger } = await calendar()
    expect(tip()!.textContent).toContain('Example <pack>')
    expect(tip()!.querySelector('pack')).toBeNull()
    expect(tip()!.textContent).toContain('2 successful launches')
    expect(getComputedStyle(tip()!).whiteSpace).toBe('pre-line')
    expect(trigger.getAttribute('aria-describedby')).toBe(tip()!.id)
    const old = tip()!.getBoundingClientRect().top
    host.scrollTop = 35
    await waitFor(() => Math.abs(tip()!.getBoundingClientRect().top - old) > 10)
    host.scrollTop = 0
    host.style.top = '0px'
    host.style.left = 'calc(100vw - 280px)'
    await waitFor(
        () => tip()!.querySelector<HTMLElement>('.tooltip-popper-arrow')?.dataset.side === 'bottom',
    )
    await waitFor(
        () =>
            Math.abs(
                tip()!.getBoundingClientRect().top -
                    Math.max(8, trigger.getBoundingClientRect().bottom + 5),
            ) < 2,
    )
    const rect = tip()!.getBoundingClientRect()
    expect(rect.top).toBeGreaterThanOrEqual(7)
    expect(rect.left).toBeGreaterThanOrEqual(7)
    expect(rect.right).toBeLessThanOrEqual(window.innerWidth - 7)
    expect(rect.bottom).toBeLessThanOrEqual(window.innerHeight - 7)
    const anchor = trigger.getBoundingClientRect()
    expect(rect.top).toBeGreaterThanOrEqual(anchor.bottom)
    expect(Math.abs(rect.top - Math.max(8, anchor.bottom + 5))).toBeLessThan(2)
})

it('switches tooltip ownership between dates and cleans up on period change and unmount', async () => {
    const { wrapper, trigger } = await calendar()
    const oldId = tip()!.id
    const next = wrapper.findAll('[data-date-key]')[1].element as HTMLButtonElement
    next.focus()
    await waitFor(() => !!tip() && tip()!.id !== oldId)
    expect(trigger.hasAttribute('aria-describedby')).toBe(false)
    expect(next.getAttribute('aria-describedby')).toBe(tip()!.id)
    wrapper
        .findAll('header button')[0]
        .element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await waitFor(() => !tip())
    const changed = wrapper.get('[data-date-key]').element as HTMLButtonElement
    changed.focus()
    await waitFor(() => !!tip())
    wrapper.unmount()
    window.dispatchEvent(new Event('resize'))
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(tip()).toBeNull()
})
