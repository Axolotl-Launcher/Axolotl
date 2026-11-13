import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import HomeItemRow from './HomeItemRow.vue'

vi.mock('@/components/ui/InstanceIcon.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['size'],
            setup: (props) => () =>
                h('img', { class: 'icon-stub', style: `width:${props.size};height:${props.size}` }),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Avatar: defineComponent({
            props: ['size'],
            setup: (props) => () =>
                h('img', { class: 'avatar', style: `width:${props.size};height:${props.size}` }),
        }),
    }
})

function mountRow(props: Record<string, unknown>) {
    const host = document.createElement('div')
    host.style.cssText = 'width:180px'
    document.body.append(host)
    const wrapper = mount(HomeItemRow, {
        attachTo: host,
        props: {
            title: 'Survival Island',
            meta: 'Survival',
            instance: { id: 'fo' },
            playLabel: 'Play',
            stopLabel: 'Stop',
            ...props,
        },
        global: {
            directives: { tooltip: () => {} },
            stubs: { RouterLink: { template: '<a><slot /></a>' } },
        },
    })
    return {
        wrapper,
        done: () => {
            wrapper.unmount()
            host.remove()
        },
    }
}

it('draws a tile with the icon as its cover and the play button hidden until pointed at', async () => {
    const { wrapper, done } = mountRow({ variant: 'tile' })
    try {
        const row = wrapper.get('.home-item-row')
        expect(row.classes()).toContain('is-tile')
        expect(getComputedStyle(row.element).flexDirection).toBe('column')
        expect(wrapper.get('.icon-stub').element.getBoundingClientRect().width).toBe(52)
        const actions = wrapper.get('.home-item-row-actions').element
        expect(getComputedStyle(actions).position).toBe('absolute')
        expect(getComputedStyle(actions).opacity).toBe('0')
        // Keyboard focus shows it like a pointer does.
        ;(wrapper.get('.home-item-row-play').element as HTMLElement).focus()
        await new Promise((resolve) => setTimeout(resolve, 250))
        expect(getComputedStyle(actions).opacity).toBe('1')
    } finally {
        done()
    }
})

it('keeps a running tile’s stop button showing', () => {
    const { wrapper, done } = mountRow({ variant: 'tile', running: true })
    try {
        expect(wrapper.find('.home-item-row-play.is-stop').exists()).toBe(true)
        expect(getComputedStyle(wrapper.get('.home-item-row-actions').element).opacity).toBe('1')
    } finally {
        done()
    }
})

it('keeps rows and cards on one line, and greys out a dimmed icon', () => {
    const row = mountRow({ variant: 'row' })
    const card = mountRow({ variant: 'card', dimmed: true })
    try {
        expect(getComputedStyle(row.wrapper.get('.home-item-row').element).flexDirection).toBe(
            'row',
        )
        expect(row.wrapper.get('.icon-stub').element.getBoundingClientRect().width).toBe(32)
        expect(card.wrapper.get('.icon-stub').element.getBoundingClientRect().width).toBe(32)
        expect(getComputedStyle(card.wrapper.get('.icon-stub').element).filter).toContain(
            'grayscale',
        )
    } finally {
        row.done()
        card.done()
    }
})

it('hides the tile actions until pointed at only for a fine pointer that can hover', () => {
    const { done } = mountRow({ variant: 'tile' })
    try {
        // Every rule that hides the actions sits under a hover-capable, fine-pointer query,
        // so a touch screen (hover: none, pointer: coarse) always shows them.
        const hiding: string[] = []
        const walk = (rules: CSSRuleList, media: string | null) => {
            for (const rule of Array.from(rules)) {
                if (rule instanceof CSSMediaRule) walk(rule.cssRules, rule.conditionText)
                else if (
                    rule instanceof CSSStyleRule &&
                    rule.selectorText.includes('.home-item-row-actions') &&
                    rule.style.opacity === '0'
                )
                    hiding.push(media ?? 'none')
            }
        }
        for (const sheet of Array.from(document.styleSheets)) walk(sheet.cssRules, null)
        expect(hiding.length).toBeGreaterThan(0)
        for (const media of hiding) {
            expect(media).toContain('hover: hover')
            expect(media).toContain('pointer: fine')
        }
    } finally {
        done()
    }
})
