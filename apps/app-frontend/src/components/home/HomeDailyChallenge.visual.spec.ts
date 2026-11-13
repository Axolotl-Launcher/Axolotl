import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import HomeDailyChallenge from './HomeDailyChallenge.vue'

// The challenge list ships with the app; an empty one must still draw a card.
vi.mock('@/data/daily-challenges', () => ({ dailyChallenges: [] }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
            locale: { value: 'en-US' },
        }),
    }
})

it('shows an empty state, not a broken quote, when there is no challenge', () => {
    const wrapper = mount(HomeDailyChallenge, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} } },
    })
    wrapper.element.setAttribute('style', 'width:280px')
    try {
        expect(wrapper.find('.home-challenge').exists()).toBe(false)
        expect(wrapper.get('.home-widget-frame-empty-title').text()).toBe('No challenge today')
        expect(wrapper.get('.home-widget-frame-empty-hint').text()).toBe(
            'Pick your own goal for this session.',
        )
        // Nothing to shuffle through.
        expect(wrapper.find('button').exists()).toBe(false)
        const art = wrapper.get('.home-widget-frame-empty-art').element.getBoundingClientRect()
        expect(art.width).toBeGreaterThan(30)
    } finally {
        wrapper.unmount()
    }
})

it('draws as a sidebar section: full width, a divider, the sidebar background, a plain title', () => {
    document.documentElement.style.setProperty('--brand-gradient-border', 'rgb(10, 20, 30)')
    const wrapper = mount(HomeDailyChallenge, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} } },
    })
    try {
        const section = wrapper.get('.home-widget-frame')
        expect(section.attributes('data-variant')).toBe('sidebar')
        // No widget card inside the sidebar: no surface, edge, radius or margin of its own.
        expect(section.classes()).not.toContain('home-surface')
        const style = getComputedStyle(section.element)
        expect(style.backgroundColor).toBe('rgba(0, 0, 0, 0)')
        expect(style.marginTop).toBe('0px')
        expect(style.paddingLeft).toBe('16px')
        expect(style.borderTopWidth).toBe('0px')
        expect(style.borderBottomWidth).toBe('1px')
        expect(style.borderBottomColor).toBe('rgb(10, 20, 30)')
        expect(style.backdropFilter === '' || style.backdropFilter === 'none').toBe(true)
        // A steady 16px section title with no icon, like the sidebar's other sections;
        // the empty state still draws under it.
        const title = getComputedStyle(wrapper.get('.home-widget-frame-title').element)
        expect(title.fontSize).toBe('16px')
        expect(wrapper.find('.home-widget-frame-icon').exists()).toBe(false)
        expect(wrapper.find('.home-widget-frame-empty-title').exists()).toBe(true)
    } finally {
        wrapper.unmount()
        document.documentElement.style.removeProperty('--brand-gradient-border')
    }
})
