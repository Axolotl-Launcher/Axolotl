import type { mount } from '@vue/test-utils'
import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import { mountThemed } from '../../test/visual-harness'
import ColorPicker from './ColorPicker.vue'

/**
 * Positioning resolves asynchronously and the browser decides the order, so a
 * test cannot make an older call land last on its own. Hold every computation
 * here and let the test release them in the order it wants to check.
 */
const control = vi.hoisted(() => ({
    manual: false,
    pending: [] as Array<(value: unknown) => void>,
}))

vi.mock('@floating-ui/vue', async (importOriginal) => {
    const actual = await importOriginal<typeof import('@floating-ui/vue')>()
    return {
        ...actual,
        computePosition: (...args: Parameters<typeof actual.computePosition>) => {
            if (!control.manual) return actual.computePosition(...args)
            return new Promise((resolve) =>
                control.pending.push(resolve as (value: unknown) => void),
            )
        },
    }
})

const mounted: ReturnType<typeof mount>[] = []

afterEach(() => {
    control.manual = false
    control.pending.length = 0
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
})

it('keeps the newest position when an older computation lands last', async () => {
    control.manual = true
    const wrapper = await mountThemed(ColorPicker, { modelValue: '#ff0000' }, 'dark')
    mounted.push(wrapper)

    await wrapper.get('button').trigger('click')
    await flushPromises()
    // Opening asks for a position, and the auto-update watcher asks again.
    expect(control.pending.length).toBeGreaterThan(1)

    const newest = control.pending.length - 1
    const superseded = control.pending.slice(0, newest)
    control.pending[newest]({ x: 200, y: 300 })
    await flushPromises()
    for (const resolve of superseded) resolve({ x: 10, y: 20 })
    await flushPromises()

    const panel = document.querySelector('[role="dialog"]') as HTMLElement
    expect(panel.style.left).toBe('200px')
    expect(panel.style.top).toBe('300px')
})
