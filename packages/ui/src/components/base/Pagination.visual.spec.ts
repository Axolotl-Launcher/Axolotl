import type { mount } from '@vue/test-utils'
import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'

import { mountThemed } from '../../test/visual-harness'
import Pagination from './Pagination.vue'

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

async function mountBar(page: number, count: number) {
    const wrapper = await mountThemed(Pagination, { page, count }, 'dark')
    mounted.push(wrapper)
    return wrapper
}

function jumpField(wrapper: Awaited<ReturnType<typeof mountBar>>) {
    return wrapper.get('input[type="number"]')
}

it('jumps to the typed page on Enter', async () => {
    const wrapper = await mountBar(1, 10)
    const input = jumpField(wrapper)
    await input.setValue('5')
    await input.trigger('keydown', { key: 'Enter' })
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[5]])
})

it('jumps to the typed page on blur', async () => {
    const wrapper = await mountBar(1, 10)
    const input = jumpField(wrapper)
    await input.setValue('7')
    await input.trigger('blur')
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[7]])
})

it('leaves a keydown that confirms an IME candidate to the input method', async () => {
    const wrapper = await mountBar(1, 10)
    const input = jumpField(wrapper)
    await input.setValue('5')
    await input.trigger('keydown', { key: 'Enter', isComposing: true })
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toBeUndefined()
})

it('clamps a typed page into the available range', async () => {
    const wrapper = await mountBar(4, 10)
    const input = jumpField(wrapper)
    await input.setValue('99')
    await input.trigger('keydown', { key: 'Enter' })
    await input.setValue('0')
    await input.trigger('keydown', { key: 'Enter' })
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[10], [1]])
})

it('ignores the current page', async () => {
    const wrapper = await mountBar(4, 10)
    const input = jumpField(wrapper)
    await input.setValue('4')
    await input.trigger('keydown', { key: 'Enter' })
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toBeUndefined()
})

it('reaches both ends through the end buttons', async () => {
    const wrapper = await mountBar(5, 10)
    const buttons = wrapper.findAll('button')
    await buttons[0].trigger('click')
    await buttons[buttons.length - 1].trigger('click')
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[1], [10]])
})

it('asks for a page past the count when the source only knows a lower bound', async () => {
    const wrapper = await mountThemed(Pagination, { page: 1, count: 2, countKnown: false }, 'dark')
    mounted.push(wrapper)
    const input = jumpField(wrapper)
    await input.setValue('20')
    await input.trigger('keydown', { key: 'Enter' })
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[20]])
})
