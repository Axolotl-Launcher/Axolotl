import { expect, it } from 'vitest'
import { nextTick, ref } from 'vue'

import { createDeferredUi } from './deferred-ui'

const settle = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms))

it('mounts on the first interaction when waiting for one', async () => {
    const ui = createDeferredUi(null)
    await settle(50)
    expect(ui.mounted.value).toBe(false)
    window.dispatchEvent(new PointerEvent('pointerdown'))
    expect(ui.mounted.value).toBe(true)
})

it('mounts once the page is idle at the latest', async () => {
    const ui = createDeferredUi(30)
    expect(ui.mounted.value).toBe(false)
    await settle(120)
    expect(ui.mounted.value).toBe(true)
})

it('a stand-in mounts the UI, waits for the component and forwards calls', async () => {
    const ui = createDeferredUi(null)
    const component = ref<{ show: (value: string) => string } | null>(null)
    const modal = ui.handle(component)

    const shown = modal.show('hello')
    expect(ui.mounted.value).toBe(true)

    // The component mounts after the deferred UI renders it.
    component.value = { show: (value) => `shown ${value}` }
    await nextTick()
    await expect(shown).resolves.toBe('shown hello')
})

it('a stand-in is not a thenable and can be stored in reactive state', async () => {
    const ui = createDeferredUi(null)
    const modal = ui.handle(ref<{ show: () => void } | null>(null))
    expect((modal as unknown as { then?: unknown }).then).toBeUndefined()
    // Awaiting it must resolve to the stand-in itself rather than hang on a fake `then`.
    await expect(Promise.resolve(modal)).resolves.toBe(modal)
    expect(ref({ modal }).value.modal).toBe(modal)
})
