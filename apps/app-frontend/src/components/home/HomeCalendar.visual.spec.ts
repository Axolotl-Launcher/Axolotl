import '@/test/visual-styles.scss'
import '@modrinth/ui/directives/tooltip-styles.ts'

import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import IntlMessageFormat from 'intl-messageformat'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { mountThemed, waitFor } from '@/test/visual-harness'

import HomeCalendar from './HomeCalendar.vue'

vi.mock('@/components/home/home-dashboard-runtime', () => ({
    useHomeDashboardRuntime: () => ({ instanceRevision: ref(0), runningInstanceIds: ref([]) }),
}))
/** Days the mocked backend reports; by default one short day at the start of the range. */
const playtime = vi.hoisted(() => ({ days: null as Record<string, number> | null }))
vi.mock('@/helpers/instance', () => ({
    get_daily_playtime: async (start: string) =>
        playtime.days
            ? Object.entries(playtime.days).map(([date, seconds]) => ({
                  date,
                  played_seconds: seconds,
                  session_count: 1,
                  top_instance_name: 'Create+',
              }))
            : [
                  {
                      date: start,
                      played_seconds: 90,
                      session_count: 2,
                      top_instance_name: 'Example <pack>',
                  },
              ],
    get_daily_playtime_details: async () => [],
    run: vi.fn(),
    kill: vi.fn(),
}))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/composables/useMinecraftLaunchError', () => ({ useMinecraftLaunchError: () => vi.fn() }))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', () => ({
    PopoutMenu: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('button', slots.default?.()),
    }),
    Button: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('button', slots.default?.()),
    }),
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        locale: { value: 'en-US' },
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, unknown>) =>
            new IntlMessageFormat(message.defaultMessage, 'en-US').format(values),
    }),
    useFormatDateTime: (options: Intl.DateTimeFormatOptions) => (date: Date) =>
        new Intl.DateTimeFormat('en-US', options).format(date),
}))

const cleanup: (() => void)[] = []
beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date(2026, 0, 15, 12))
})
afterEach(() => {
    playtime.days = null
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn())
    vi.useRealTimers()
})
const tip = () => document.querySelector<HTMLElement>('[role="tooltip"]')

async function calendar() {
    const host = document.createElement('div')
    host.style.cssText = 'position:fixed;top:40px;left:12px;width:270px;height:180px;overflow:auto'
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
    const { wrapper } = await calendar()
    const dateKeys = () => wrapper.findAll('[data-date-key]')
    let trigger = dateKeys()[0].element as HTMLButtonElement
    if (dateKeys().length < 2) {
        // On the first day of a period only one date is selectable. Move to
        // the previous period so the test always has two dates to switch
        // between; otherwise it fails on the first of every month.
        wrapper
            .findAll('header button')[0]
            .element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
        await waitFor(() => dateKeys().length > 1)
        trigger = dateKeys()[0].element as HTMLButtonElement
        trigger.focus()
        await waitFor(() => tip()?.parentElement?.dataset.state === 'open')
    }
    const oldId = tip()!.id
    const next = dateKeys()[1].element as HTMLButtonElement
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

async function heatmap(size: string, width: number, height: number) {
    const host = document.createElement('div')
    host.style.cssText = `position:fixed;top:20px;left:12px;width:${width}px;height:${height}px;display:flex;flex-direction:column`
    document.body.append(host)
    cleanup.push(() => host.remove())
    const wrapper = await mountThemed(
        HomeCalendar,
        { instances: [], dashboardSize: size },
        'dark',
        {
            attachTo: host,
            global: { directives: { tooltip: tooltipDirective } },
        },
    )
    cleanup.push(() => wrapper.unmount())
    await waitFor(() => wrapper.findAll('[data-date-key]').length > 0, { label: 'squares' })
    return wrapper
}

it('colours each day of the month by its share of the busiest day, and rings today', async () => {
    playtime.days = {
        '2026-01-05': 3 * 3600,
        '2026-01-06': 50 * 60,
        '2026-01-07': 5 * 60,
        '2026-01-12': 20 * 60,
    }
    const wrapper = await heatmap('1x2', 240, 336)
    await waitFor(() => wrapper.find('[data-date-key="2026-01-05"][data-level="4"]').exists())
    const level = (key: string) => wrapper.get(`[data-date-key="${key}"]`).attributes('data-level')
    expect(
        ['2026-01-05', '2026-01-06', '2026-01-07', '2026-01-12', '2026-01-08'].map(level),
    ).toEqual(['4', '2', '1', '2', '0'])
    // Every step is a different colour, and a cell is never taller than it is wide.
    const fills = ['0', '1', '2', '3', '4'].map(
        (step) =>
            getComputedStyle(wrapper.get(`.home-heat-legend [data-level="${step}"]`).element)
                .backgroundColor,
    )
    expect(new Set(fills).size).toBe(5)
    const square = wrapper.get('[data-date-key="2026-01-05"]').element.getBoundingClientRect()
    expect(square.height).toBeLessThanOrEqual(square.width + 0.5)
    // Today is a ring, not a fill; days to come are faint and cannot be picked.
    // Selected, it carries the selection ring alone instead of two stacked rings.
    const today = wrapper.get('.home-heat-cell.is-today')
    expect(today.classes()).toContain('is-selected')
    expect(getComputedStyle(today.element).boxShadow).not.toContain('inset')
    await wrapper.get('[data-date-key="2026-01-06"]').trigger('click')
    expect(today.classes()).not.toContain('is-selected')
    expect(getComputedStyle(today.element).boxShadow).toContain('inset')
    const future = wrapper.get('.home-heat-cell.is-future').element as HTMLButtonElement
    expect(future.disabled).toBe(true)
    expect(Number(getComputedStyle(future).opacity)).toBeLessThan(0.6)
    // Each square says what it holds.
    expect(wrapper.get('[data-date-key="2026-01-06"]').attributes('aria-label')).toContain('50m')
})

it('moves between days with the arrow keys and picks one with Enter', async () => {
    playtime.days = { '2026-01-07': 3600 }
    const wrapper = await heatmap('1x2', 240, 336)
    const stop = wrapper.findAll('[data-date-key][tabindex="0"]')
    expect(stop).toHaveLength(1)
    expect(stop[0].attributes('data-date-key')).toBe('2026-01-15')
    ;(stop[0].element as HTMLElement).focus()
    stop[0].element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }))
    expect(document.activeElement?.getAttribute('data-date-key')).toBe('2026-01-14')
    document.activeElement!.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }),
    )
    expect(document.activeElement?.getAttribute('data-date-key')).toBe('2026-01-07')
    ;(document.activeElement as HTMLButtonElement).click()
    await waitFor(() => wrapper.get('.home-calendar-day h3').text() === 'January 7')
    expect(wrapper.get('[data-date-key="2026-01-07"]').attributes('aria-pressed')).toBe('true')
})

it('stretches the month across the card, left-aligned with the title', async () => {
    playtime.days = { '2026-01-07': 3600 }
    const wrapper = await heatmap('1x2', 240, 336)
    const card = wrapper.get('.home-calendar').element.getBoundingClientRect()
    const grid = wrapper.get('.home-heat-month[role="grid"]').element.getBoundingClientRect()
    const title = wrapper.get('.home-widget-frame-header').element.getBoundingClientRect()
    expect(Math.abs(grid.left - title.left)).toBeLessThan(1)
    // Seven columns always take the full width: the right edge mirrors the left.
    expect(Math.abs(card.right - grid.right - (grid.left - card.left))).toBeLessThan(1)
    const first = wrapper.get('[data-date-key="2026-01-05"]').element.getBoundingClientRect()
    const last = wrapper.get('[data-date-key="2026-01-11"]').element.getBoundingClientRect()
    expect(Math.abs(first.left - grid.left)).toBeLessThan(1)
    expect(Math.abs(last.right - grid.right)).toBeLessThan(1)
    // A cell flattens rather than leaving a gap, and is never taller than wide.
    expect(first.width).toBeGreaterThanOrEqual(22)
    expect(first.height).toBeLessThanOrEqual(first.width + 0.5)
    // Grid, the chosen day and the legend all fit inside the card.
    for (const selector of ['.home-heat-month[role="grid"]', '.home-calendar-summary'])
        expect(wrapper.get(selector).element.getBoundingClientRect().bottom).toBeLessThanOrEqual(
            card.bottom + 0.5,
        )
})

it('draws the band back to the first week played, beside figures about it', async () => {
    playtime.days = { '2026-01-14': 3600, '2026-01-13': 5400 }
    const band = await heatmap('2x2', 496, 336)
    // The band measures the card first, then lays out its weeks.
    await waitFor(() => band.findAll('.home-heat-band .home-heat-cell').length >= 7 * 12, {
        label: 'week columns',
    })
    const grid = band.get('.home-heat-band').element.getBoundingClientRect()
    const box = band.get('.home-calendar').element.getBoundingClientRect()
    expect(grid.right).toBeLessThanOrEqual(box.right + 0.5)
    expect(grid.left).toBeGreaterThanOrEqual(box.left)
    // One week of data: the twelve-week minimum, and the rest of the width for figures.
    expect(band.findAll('.home-heat-band .home-heat-cell')).toHaveLength(7 * 12)
    expect(band.get('.home-calendar-insights').text()).toContain('1.5h')
    expect(band.get('.home-calendar-insights').text()).toContain('2 days')
    band.unmount()
    const week = await heatmap('1x1', 240, 160)
    expect(week.findAll('.home-heat-week .home-heat-cell')).toHaveLength(7)
    // The big figure is this week's hours, like the squares under it.
    expect(week.get('.home-calendar-stat strong').text()).toBe('2.5')
    expect(week.get('.home-calendar-stat-note').text()).toBe('2.5h this month')
    const tile = week.get('.home-calendar').element.getBoundingClientRect()
    for (const cell of week.findAll('.home-heat-week .home-heat-cell'))
        expect(cell.element.getBoundingClientRect().bottom).toBeLessThanOrEqual(tile.bottom)
})
