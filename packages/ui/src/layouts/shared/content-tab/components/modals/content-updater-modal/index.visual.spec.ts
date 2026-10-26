import { afterEach, expect, it } from 'vitest'
import { nextTick } from 'vue'

import { mountThemed, waitFor } from '#ui/test/visual-harness'

import ContentUpdaterModal from './index.vue'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

async function modal(props: Record<string, unknown> = {}) {
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    cleanup.push(() => teleports.remove())
    const wrapper = await mountThemed(
        ContentUpdaterModal,
        {
            versions: [],
            currentGameVersion: '1.20.1',
            currentLoader: 'fabric',
            currentVersionId: 'old',
            isApp: true,
            ...props,
        },
        'dark',
    )
    cleanup.push(() => wrapper.unmount())
    ;(wrapper.vm as unknown as { show: () => void }).show()
    await waitFor(() => !!document.querySelector('.modal-container.shown'))
    return wrapper
}

it('keeps a submitting updater open through cancel, Escape, close and overlay actions', async () => {
    const wrapper = await modal({
        actionLoading: true,
        mode: 'incompatibility-warning',
        warning: 'Warning',
    })
    const cancel = [...document.querySelectorAll('button')].find((button) =>
        button.textContent?.includes('Cancel'),
    )!
    expect(cancel.disabled).toBe(true)
    cancel.click()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    const close = document.querySelector<HTMLButtonElement>('button[aria-label="Close"]')!
    expect(close.disabled).toBe(true)
    close.click()
    document.querySelector<HTMLElement>('.modal-overlay')?.click()
    await nextTick()
    expect(document.querySelector('.modal-container.shown')).not.toBeNull()
    expect(wrapper.emitted('cancel')).toBeUndefined()
    await wrapper.setProps({ actionLoading: false })
    cancel.click()
    await waitFor(() => !document.querySelector('.modal-container.shown'))
    expect(wrapper.emitted('cancel')).toBeTruthy()
})

it('shows a retryable version error without loading or empty-version feedback', async () => {
    const wrapper = await modal({ loadingError: 'Versions unavailable' })
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('Versions unavailable')
    const retry = [...document.querySelectorAll('button')].find((button) =>
        button.textContent?.includes('Retry'),
    )!
    retry.click()
    expect(wrapper.emitted('retry')).toHaveLength(1)
})
