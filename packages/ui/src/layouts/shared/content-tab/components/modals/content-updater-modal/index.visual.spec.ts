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

const version = (id: string) => ({
    id,
    project_id: 'project',
    name: id,
    version_number: id,
    date_published: '2024-01-01T00:00:00Z',
    version_type: 'release',
    game_versions: ['1.20.1'],
    loaders: ['fabric'],
    files: [],
})

it('keeps loaded versions listed while more load or after a later page fails', async () => {
    const wrapper = await modal({ versions: [version('first')], loading: true })
    const listed = () =>
        [...document.querySelectorAll('[role="option"]')].map((option) => option.textContent)
    expect(listed().some((text) => text?.includes('first'))).toBe(true)
    expect(document.querySelector('[role="status"]')?.textContent).toContain(
        'Loading more versions',
    )
    await wrapper.setProps({ loading: false, loadingError: 'Versions unavailable' })
    expect(listed().some((text) => text?.includes('first'))).toBe(true)
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('Versions unavailable')
})

it('waits for a requested version on a later page instead of selecting the first one', async () => {
    const wrapper = await modal({ versions: [], loading: true })
    ;(wrapper.vm as unknown as { show: (id?: string) => void }).show('wanted')
    await wrapper.setProps({ versions: [version('first')] })
    expect(wrapper.emitted('versionSelect')).toBeUndefined()
    await wrapper.setProps({ versions: [version('first'), version('wanted')], loading: false })
    expect(wrapper.emitted('versionSelect')?.at(-1)?.[0]).toMatchObject({ id: 'wanted' })
})

it('keeps waiting for the requested version after a later page fails', async () => {
    const wrapper = await modal({ versions: [], loading: true })
    ;(wrapper.vm as unknown as { show: (id?: string) => void }).show('wanted')
    await wrapper.setProps({ versions: [version('first')], loading: false, loadingError: 'Failed' })
    expect(wrapper.emitted('versionSelect')).toBeUndefined()
    await wrapper.setProps({
        versions: [version('first'), version('wanted')],
        loadingError: undefined,
    })
    expect(wrapper.emitted('versionSelect')?.at(-1)?.[0]).toMatchObject({ id: 'wanted' })
})

it('opened while pages still arrive, waits for the requested version instead of the first listed', async () => {
    const wrapper = await modal({ versions: [version('first')], loading: true })
    const before = wrapper.emitted('versionSelect')?.length ?? 0
    ;(wrapper.vm as unknown as { show: (id?: string) => void }).show('wanted')
    await nextTick()
    expect(wrapper.emitted('versionSelect')?.length ?? 0).toBe(before)
    await wrapper.setProps({ versions: [version('first'), version('wanted')], loading: false })
    expect(wrapper.emitted('versionSelect')?.at(-1)?.[0]).toMatchObject({ id: 'wanted' })
})
