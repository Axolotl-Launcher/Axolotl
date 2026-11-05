import { mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import AgentWindowButton from './AgentWindowButton.vue'

const locale = ref('en-US')
const invoke = vi.fn()
const handleError = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }))
vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError }),
    useVIntl: () => ({
        locale,
        formatMessage: (message: { defaultMessage: string }) =>
            `${locale.value}: ${message.defaultMessage}`,
    }),
}))

let wrapper: VueWrapper
let initialAccent: string
beforeEach(() => {
    locale.value = 'en-US'
    invoke.mockReset()
    handleError.mockReset()
    initialAccent = document.documentElement.style.getPropertyValue('--color-brand')
})
afterEach(() => {
    wrapper?.unmount()
    document.documentElement.style.setProperty('--color-brand', initialAccent)
})

function deferred() {
    let resolve!: () => void
    const promise = new Promise<void>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

function updates() {
    return invoke.mock.calls.filter(
        ([command]) => command === 'plugin:agent-window|update_agent_window_appearance',
    )
}

it('syncs the latest locale, translated messages and theme after a pending open finishes', async () => {
    const opening = deferred()
    invoke.mockImplementation((command) =>
        command.endsWith('|open_agent_window') ? opening.promise : Promise.resolve(),
    )
    wrapper = mount(AgentWindowButton, { attachTo: document.body })
    await wrapper.get('button').trigger('click')
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))
    expect(invoke.mock.calls[0][1].appearance.messages.newChat).toBe('en-US: New task')
    locale.value = 'zh-TW'
    document.documentElement.style.setProperty('--color-brand', '#123456')
    await nextTick()
    expect(updates()).toHaveLength(0)

    opening.resolve()
    await vi.waitFor(() => expect(updates()).toHaveLength(1))
    expect(updates()[0][1].appearance).toMatchObject({
        locale: 'zh-TW',
        accent: 'rgb(18, 52, 86)',
        messages: { newChat: 'zh-TW: New task' },
    })
    expect(handleError).not.toHaveBeenCalled()
})

it('serializes appearance writes and merges changes made while a write is pending', async () => {
    const first = deferred()
    const second = deferred()
    invoke.mockImplementation((command) => {
        if (command.endsWith('|update_agent_window_appearance')) {
            return updates().length === 1 ? first.promise : second.promise
        }
        return Promise.resolve()
    })
    wrapper = mount(AgentWindowButton, { attachTo: document.body })
    await wrapper.get('button').trigger('click')
    await vi.waitFor(() => expect(updates()).toHaveLength(1))

    locale.value = 'zh-TW'
    await nextTick()
    locale.value = 'fr-FR'
    document.documentElement.style.setProperty('--color-brand', '#654321')
    await nextTick()
    await new Promise((resolve) => requestAnimationFrame(resolve))
    expect(updates()).toHaveLength(1)
    first.resolve()
    await vi.waitFor(() => expect(updates()).toHaveLength(2))
    expect(updates()[1][1].appearance).toMatchObject({
        locale: 'fr-FR',
        accent: 'rgb(101, 67, 33)',
        messages: { newChat: 'fr-FR: New task' },
    })
    second.resolve()
})
