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
