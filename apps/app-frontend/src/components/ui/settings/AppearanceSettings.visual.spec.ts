import '@/test/visual-styles.scss'

import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, Suspense } from 'vue'

import AppearanceSettings from './AppearanceSettings.vue'

const update = vi.hoisted(() => vi.fn())

vi.mock('@/helpers/settings.ts', () => ({
    get: async () => ({
        accent_color: 'pink',
        default_page: 'Home',
        feature_flags: {},
        hidden_nav_items: [],
        home_layout: 'standard',
        home_widget_background_opacity: 100,
        sidebar_instance_count: 5,
    }),
    update,
}))
vi.mock('@/helpers/utils', () => ({ getOS: async () => 'Windows' }))
vi.mock('@/components/ui/headless/HeadlessSelect.vue', () => ({ default: { render: () => null } }))
vi.mock('./AccentColorOption.vue', () => ({ default: { render: () => null } }))
vi.mock('./LauncherBackgroundSettings.vue', () => ({ default: { render: () => null } }))
vi.mock('./PinnedOverrideHint.vue', () => ({ default: { render: () => null } }))
vi.mock('./theme-suite/ThemeSuiteModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { default: Card } = await import('@modrinth/ui/src/components/base/Card.vue')
    const { default: Toggle } = await import('@modrinth/ui/src/components/base/Toggle.vue')
    const button = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('button', slots.default?.()),
    })
    const unusedControl = { render: () => null }
    return {
        Button: button,
        Card,
        Combobox: unusedControl,
        Slider: unusedControl,
        ThemeSelector: unusedControl,
        Toggle,
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('aligns navigation headings and switches with neighboring settings rows on wide and narrow screens', async () => {
    const previousWidth = window.innerWidth
    const previousHeight = window.innerHeight
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(Suspense, {}, () => h(AppearanceSettings, { scope: 'home-navigation' })),
        }),
        {
            attachTo: document.body,
            global: { plugins: [createPinia()], directives: { tooltip: () => {} } },
        },
    )
    try {
        await flushPromises()
        const group = wrapper
            .findAll('button[aria-expanded]')
            .find((button) => button.text() === 'Browse')!
        const locked = wrapper.get('#nav-item-home')
        const toggle = wrapper.get('#nav-item-discover')
        const toggleLabel = toggle.element.closest('label')!.firstElementChild!
        const standardLabel = wrapper.get('#settings-target-appearance-show-scroll-top')
        const standardToggle = wrapper.get('#show-scroll-top')

        for (const width of [1000, 620]) {
            await page.viewport(width, 800)
            const labelLeft = standardLabel.element.getBoundingClientRect().left
            const switchRight = standardToggle.element.getBoundingClientRect().right
            expect(standardToggle.element.getBoundingClientRect().width).toBeGreaterThan(0)
            expect(group.get('span').element.getBoundingClientRect().left).toBeCloseTo(labelLeft, 1)
            expect(toggleLabel.getBoundingClientRect().left).toBeCloseTo(labelLeft, 1)
            expect(toggle.element.getBoundingClientRect().right).toBeCloseTo(switchRight, 1)
            expect(locked.element.getBoundingClientRect().right).toBeCloseTo(switchRight, 1)
        }

        expect(locked.attributes()).toHaveProperty('disabled')
        await group.trigger('click')
        expect(group.attributes('aria-expanded')).toBe('false')
        expect(toggle.isVisible()).toBe(false)
        await group.trigger('click')
        await toggle.trigger('click')
        await flushPromises()
        expect(update).toHaveBeenCalledWith({ hidden_nav_items: ['discover'] })
    } finally {
        wrapper.unmount()
        await page.viewport(previousWidth, previousHeight)
    }
})
