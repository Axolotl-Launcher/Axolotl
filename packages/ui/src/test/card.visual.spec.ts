import { afterEach, expect, it } from 'vitest'

import Card from '../components/base/Card.vue'
import { mountThemed } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

it('names the collapse control and exposes its expanded state', async () => {
    const wrapper = await mountThemed(Card, { collapsible: true }, 'dark', {
        slots: {
            header: '<span>Advanced</span>',
            default: '<p>Body</p>',
        },
    })
    cleanup.push(() => wrapper.unmount())

    const toggle = wrapper.get('button')
    expect(toggle.attributes('aria-label')).toBe('Collapse')
    expect(toggle.attributes('aria-expanded')).toBe('true')

    await toggle.trigger('click')

    expect(toggle.attributes('aria-label')).toBe('Expand')
    expect(toggle.attributes('aria-expanded')).toBe('false')
})
