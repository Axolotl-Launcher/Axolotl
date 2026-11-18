import type { mount } from '@vue/test-utils'
import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'

import { mountThemed } from '../../test/visual-harness'
import Pagination from './Pagination.vue'

type BarProps = {
    page: number
    count: number
    countKnown?: boolean
    cursorPaged?: boolean
    jumpLimit?: number
}

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

async function mountBar(props: BarProps) {
    const wrapper = await mountThemed(Pagination, props, 'dark')
    mounted.push(wrapper)
    return wrapper
}

function jumpField(wrapper: Awaited<ReturnType<typeof mountBar>>) {
    return wrapper.get('input[type="number"]')
}

async function commit(input: ReturnType<typeof jumpField>, value: string) {
    await input.setValue(value)
    await input.trigger('keydown', { key: 'Enter' })
    await flushPromises()
}

it('jumps to the typed page on Enter', async () => {
    const wrapper = await mountBar({ page: 1, count: 10 })
    await commit(jumpField(wrapper), '5')
    expect(wrapper.emitted('switch-page')).toEqual([[5]])
})

it('jumps to the typed page on blur', async () => {
    const wrapper = await mountBar({ page: 1, count: 10 })
    const input = jumpField(wrapper)
    await input.setValue('7')
    await input.trigger('blur')
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[7]])
})

it('leaves a keydown that confirms an IME candidate to the input method', async () => {
    const wrapper = await mountBar({ page: 1, count: 10 })
    const input = jumpField(wrapper)
    await input.setValue('5')
    const event = new KeyboardEvent('keydown', {
        key: 'Enter',
        isComposing: true,
        cancelable: true,
        bubbles: true,
    })
    input.element.dispatchEvent(event)
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toBeUndefined()
    // Cancelling the key would stop the IME from confirming its candidate.
    expect(event.defaultPrevented).toBe(false)
})

it('clamps a typed page into the available range', async () => {
    const wrapper = await mountBar({ page: 4, count: 10 })
    const input = jumpField(wrapper)
    await commit(input, '99')
    await commit(input, '0')
    expect(wrapper.emitted('switch-page')).toEqual([[10], [1]])
})

it('ignores the current page', async () => {
    const wrapper = await mountBar({ page: 4, count: 10 })
    await commit(jumpField(wrapper), '4')
    expect(wrapper.emitted('switch-page')).toBeUndefined()
})

it('reads a whole page number and rejects a fractional one', async () => {
    const wrapper = await mountBar({ page: 1, count: 30 })
    const input = jumpField(wrapper)
    // A number field holds both of these, and `parseInt` would read them as 2 and 1.
    await commit(input, '2.5')
    expect(wrapper.emitted('switch-page')).toBeUndefined()
    expect((input.element as HTMLInputElement).value).toBe('1')

    await commit(input, '1e1')
    expect(wrapper.emitted('switch-page')).toEqual([[10]])
})

it('reaches both ends through the end buttons', async () => {
    const wrapper = await mountBar({ page: 5, count: 10 })
    const buttons = wrapper.findAll('button')
    await buttons[0].trigger('click')
    await buttons[buttons.length - 1].trigger('click')
    await flushPromises()
    expect(wrapper.emitted('switch-page')).toEqual([[1], [10]])
})

it('hides the last-page control on a cursor source', async () => {
    const direct = await mountBar({ page: 5, count: 10 })
    const cursor = await mountBar({ page: 5, count: 10, cursorPaged: true })
    // Same strip, one button fewer: a cursor source cannot reach its last page
    // without walking every page before it.
    expect(cursor.findAll('button').length).toBe(direct.findAll('button').length - 1)
})

it('asks for a page past the count when the source only knows a lower bound', async () => {
    const wrapper = await mountBar({ page: 1, count: 2, countKnown: false, cursorPaged: true })
    await commit(jumpField(wrapper), '3')
    expect(wrapper.emitted('switch-page')).toEqual([[3]])
})

it('caps a jump when the page count is only a lower bound', async () => {
    const wrapper = await mountBar({ page: 4, count: 5, countKnown: false, cursorPaged: true })
    await commit(jumpField(wrapper), '100')
    // Default jumpLimit is 10, so the request stops 10 pages ahead of the reader.
    expect(wrapper.emitted('switch-page')).toEqual([[14]])
})

it('caps a jump on a cursor source even when the count is exact', async () => {
    const wrapper = await mountBar({ page: 4, count: 30, cursorPaged: true })
    await commit(jumpField(wrapper), '100')
    expect(wrapper.emitted('switch-page')).toEqual([[14]])
})

it('honours a custom jump limit', async () => {
    const wrapper = await mountBar({
        page: 4,
        count: 5,
        countKnown: false,
        cursorPaged: true,
        jumpLimit: 2,
    })
    await commit(jumpField(wrapper), '100')
    expect(wrapper.emitted('switch-page')).toEqual([[6]])
})

it('does not cap a jump when any page can be read directly', async () => {
    const wrapper = await mountBar({ page: 1, count: 30 })
    await commit(jumpField(wrapper), '100')
    // `count` is the real end of the result set and reading it is one request.
    expect(wrapper.emitted('switch-page')).toEqual([[30]])
})

it('shows the page a jump landed on instead of the value that was typed', async () => {
    // A cursor source that has run out reports its last page as the count, and
    // clamping onto the page already shown emits nothing — so the field is the only
    // thing that can tell the reader the typed value was not used as-is.
    const wrapper = await mountBar({ page: 10, count: 10, cursorPaged: true })
    const input = jumpField(wrapper)
    await commit(input, '100')
    expect(wrapper.emitted('switch-page')).toBeUndefined()
    expect((input.element as HTMLInputElement).value).toBe('10')
})
