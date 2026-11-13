import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, provide, ref } from 'vue'

import { homeWidgetMoreTargetKey } from './home-utils'
import HomeFitList from './HomeFitList.vue'

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        // The popover's panel is always drawn here, so its rows can be counted.
        PopoutMenu: defineComponent({
            setup:
                (_, { slots, attrs }) =>
                () => [
                    h('button', attrs, slots.default?.()),
                    h('div', { class: 'popout-stub' }, slots.menu?.({ hide: () => {} })),
                ],
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, unknown>,
            ) => new IntlMessageFormat(message.defaultMessage, 'en-US').format(values) as string,
        }),
    }
})

const items = ['One', 'Two', 'Three', 'Four', 'Five']

/** A widget-like box: a header slot for the "+N" button and a list body of a given height. */
function mountList(height: number, variant: 'row' | 'card' = 'row') {
    const host = document.createElement('div')
    host.style.cssText = 'width:240px'
    document.body.append(host)
    const Box = defineComponent({
        setup() {
            const target = ref<HTMLElement | null>(null)
            provide(homeWidgetMoreTargetKey, target)
            return () =>
                h('section', [
                    h('header', [h('span', { ref: target, class: 'more-target' })]),
                    h('div', { style: `display:flex;flex-direction:column;height:${height}px` }, [
                        h(
                            HomeFitList,
                            { items, itemKey: (item: string) => item, variant },
                            {
                                default: ({ item }: { item: string }) =>
                                    h('div', { class: 'item', style: 'height:40px' }, item),
                            },
                        ),
                    ]),
                ])
        },
    })
    const wrapper = mount(Box, { attachTo: host, global: { directives: { tooltip: () => {} } } })
    return {
        wrapper,
        done: () => {
            wrapper.unmount()
            host.remove()
        },
    }
}

async function settle() {
    await nextTick()
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)))
    await nextTick()
}

it('shows only whole rows and hides the one cut by the edge', async () => {
    // 100px holds two 40px rows (with the 2px gap) and part of a third.
    const { wrapper, done } = mountList(100)
    try {
        await settle()
        const rows = wrapper.findAll('.home-pinned-list > li')
        expect(rows).toHaveLength(5)
        const shown = rows.filter((row) => getComputedStyle(row.element).visibility === 'visible')
        expect(shown.map((row) => row.text())).toEqual(['One', 'Two'])
        const list = wrapper.get('.home-pinned-list').element.getBoundingClientRect()
        for (const row of shown)
            expect(row.element.getBoundingClientRect().bottom).toBeLessThanOrEqual(list.bottom)
        // Hidden rows cannot be reached by keyboard.
        expect(rows[2].attributes('inert')).toBeDefined()
        expect(getComputedStyle(wrapper.get('.home-pinned-list').element).overflowY).toBe('hidden')
    } finally {
        done()
    }
})

it('puts the rest behind a "+N" button in the header, as rows', async () => {
    const { wrapper, done } = mountList(100, 'card')
    try {
        await settle()
        const more = wrapper.get('.more-target .home-fit-more')
        expect(more.text()).toContain('3 more')
        expect(more.attributes('aria-label')).toBe('Show the 3 items that do not fit')
        expect(wrapper.findAll('.home-fit-more-list > li').map((row) => row.text())).toEqual([
            'Three',
            'Four',
            'Five',
        ])
    } finally {
        done()
    }
})

it('has no "+N" button when everything fits', async () => {
    const { wrapper, done } = mountList(400)
    try {
        await settle()
        expect(wrapper.find('.home-fit-more').exists()).toBe(false)
        expect(wrapper.findAll('.home-pinned-list > li[data-fit-hidden]')).toHaveLength(0)
    } finally {
        done()
    }
})
