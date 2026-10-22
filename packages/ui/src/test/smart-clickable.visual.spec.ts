import './visual-styles.scss'

import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'
import { h } from 'vue'

import SmartClickable from '../components/base/SmartClickable.vue'

it('keeps overlay and content in one shrinking track without moving a row action outside its container', () => {
    const host = document.createElement('div')
    host.style.width = '250px'
    document.body.append(host)
    const wrapper = mount(SmartClickable, {
        attachTo: host,
        slots: {
            clickable: () => h('a', { href: '#item', 'aria-label': 'Item' }),
            default: () =>
                h('div', { class: 'flex min-w-0 items-center gap-2' }, [
                    h(
                        'div',
                        { class: 'min-w-0 flex-1' },
                        h('span', { class: 'block truncate' }, '长名称'.repeat(80)),
                    ),
                    h('button', { class: 'size-6 shrink-0', 'aria-label': 'More' }, '…'),
                ]),
        },
    })
    try {
        const action = wrapper.get('button').element.getBoundingClientRect()
        const bounds = host.getBoundingClientRect()
        expect(action.width).toBeGreaterThan(0)
        expect(action.right).toBeLessThanOrEqual(bounds.right)
        expect(
            wrapper.get('.smart-clickable__contents').element.getBoundingClientRect().width,
        ).toBeLessThanOrEqual(bounds.width)
        expect(wrapper.get('a').element.getBoundingClientRect().width).toBe(bounds.width)
    } finally {
        wrapper.unmount()
        host.remove()
    }
})
