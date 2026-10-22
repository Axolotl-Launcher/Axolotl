import { afterEach, expect, it, vi } from 'vitest'
import { h } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import OverflowMenu from '../components/base/OverflowMenu.vue'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

function key(key: string) {
    document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', {
            key,
            bubbles: true,
            cancelable: true,
        }),
    )
}

async function menu(options: InstanceType<typeof OverflowMenu>['$props']['options']) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/:pathMatch(.*)*', component: { render: () => h('div') } }],
    })
    await router.push('/')
    await router.isReady()
    const wrapper = await mountThemed(OverflowMenu, { options }, 'dark', {
        slots: { default: 'Options', 'menu-header': () => h('div', 'Actions') },
        global: { plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    const trigger = wrapper.get('button[aria-haspopup="menu"]').element as HTMLButtonElement
    trigger.focus()
    key('ArrowDown')
    await waitFor(() => document.activeElement?.getAttribute('role') === 'menuitem')
    return { wrapper, router, trigger }
}

it('registers actions, links and separators for arrow navigation and typeahead', async () => {
    const action = vi.fn()
    const { router, trigger } = await menu([
        { id: 'Unavailable', disabled: true, action },
        { id: 'Alpha', action },
        { divider: true },
        { id: 'Hidden', shown: false, action },
        { id: 'Beta', link: '/beta', action },
        { id: 'Disabled link', link: '/disabled', disabled: true, action },
        { id: 'Charlie', action },
    ])
    expect(document.querySelectorAll('[role="menuitem"]').length).toBe(5)
    expect(document.querySelectorAll('[role="separator"]').length).toBe(1)
    expect(document.activeElement?.textContent?.trim()).toBe('Alpha')
    key('ArrowDown')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Beta')
    key('ArrowDown')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Charlie')
    key('ArrowUp')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Beta')
    key('Home')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Alpha')
    key('c')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Charlie')
    key('Home')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Alpha')
    key('ArrowDown')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Beta')
    key('Enter')
    await waitFor(() => router.currentRoute.value.path === '/beta')
    await waitFor(() => !document.querySelector('[role="menu"]'))
    await waitFor(() => document.activeElement === trigger)
    expect(action).toHaveBeenCalledOnce()
})

it('keeps remainOnClick actions open and prevents disabled activation', async () => {
    const keep = vi.fn()
    const disabled = vi.fn()
    const close = vi.fn()
    const { trigger } = await menu([
        { id: 'Keep open', action: keep, remainOnClick: true },
        { id: 'Disabled', action: disabled, disabled: true },
        { id: 'Close', action: close },
    ])
    key('Enter')
    await waitFor(() => keep.mock.calls.length === 1)
    expect(document.querySelector('[role="menu"]')).not.toBeNull()
    const disabledItem = Array.from(
        document.querySelectorAll<HTMLElement>('[role="menuitem"]'),
    ).find((item) => item.textContent?.trim() === 'Disabled')!
    disabledItem.click()
    expect(disabled).not.toHaveBeenCalled()
    key('ArrowDown')
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Close')
    key('Enter')
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(close).toHaveBeenCalledOnce()
    await waitFor(() => document.activeElement === trigger)
})

it('preserves external link and download attributes, and restores focus on Escape', async () => {
    const { trigger } = await menu([
        {
            id: 'Download',
            link: 'https://example.com/archive.zip',
            external: true,
            download: 'archive.zip',
        },
    ])
    const link = document.querySelector<HTMLAnchorElement>('[role="menuitem"]')!
    expect(link.tagName).toBe('A')
    expect(link.href).toBe('https://example.com/archive.zip')
    expect(link.target).toBe('_blank')
    expect(link.download).toBe('archive.zip')
    key('Escape')
    await waitFor(() => !document.querySelector('[role="menu"]'))
    await waitFor(() => document.activeElement === trigger)
})
