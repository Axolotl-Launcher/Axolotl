import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { useAppEvent } from './use-app-event'

const backend = vi.hoisted(() => ({ listen: vi.fn() }))
vi.mock('@/helpers/events.js', () => ({ instance_listener: backend.listen }))

it('releases late subscriptions and suppresses callbacks after unmount', async () => {
    let finish!: (dispose: () => void) => void
    backend.listen.mockReturnValue(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const callback = vi.fn()
    const wrapper = mount({
        setup() {
            useAppEvent('instance', callback)
            return () => null
        },
    })
    const receive = backend.listen.mock.calls[0][0]
    receive({ event: 'screenshots_updated' })
    expect(callback).toHaveBeenCalledOnce()
    wrapper.unmount()
    receive({ event: 'screenshots_updated' })
    expect(callback).toHaveBeenCalledOnce()
    const dispose = vi.fn()
    finish(dispose)
    await flushPromises()
    expect(dispose).toHaveBeenCalledOnce()
})
