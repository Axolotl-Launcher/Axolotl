import { afterEach, expect, it, vi } from 'vitest'
import { h, nextTick } from 'vue'

import { mountThemed, waitFor } from '@/test/visual-harness'

import ContextMenu from './ContextMenu.vue'

vi.mock('@modrinth/ui', async () => ({
    PointMenu: (await import('../../../../../packages/ui/src/components/base/PointMenu.vue'))
        .default,
}))

const menuRoot = () => ({
    element: document.querySelector<HTMLElement>('[role="menu"]')!,
    isVisible: () => !!document.querySelector('[role="menu"]'),
})
const globalItem = (selector: string) => ({
    element: document.querySelector<HTMLElement>(selector)!,
    trigger: async (name: string) => {
        document
            .querySelector<HTMLElement>(selector)
            ?.dispatchEvent(new MouseEvent(name, { bubbles: true }))
        await nextTick()
    },
})
const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

async function menu(item: unknown = null) {
    const action = vi.fn()
    const wrapper = await mountThemed(ContextMenu, {}, 'dark', {
        slots: { inspect: () => h('span', 'Nested label') },
        global: { directives: { tooltip: {} } },
    })
    cleanup.push(() => wrapper.unmount())
    const vm = wrapper.vm as unknown as {
        showMenu: (event: MouseEvent, item: unknown, options: unknown[]) => void
        close: () => void
    }
    vm.showMenu(new MouseEvent('contextmenu', { clientX: 40, clientY: 40 }), item, [
        { name: 'inspect', label: 'Inspect' },
        { type: 'divider' },
        { name: 'disabled', label: 'Disabled', disabled: true, action },
        { name: 'action', label: 'Action', action },
        {
            name: 'add_content',
            label: 'Add content',
            shown: !(item as { instance?: { link?: boolean } } | null)?.instance?.link,
        },
    ])
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    return { wrapper, vm, action }
}

it('keeps padding, separators and disabled items inert while actions target the correct item', async () => {
    const item = { id: 'instance-a' }
    const { wrapper, vm, action } = await menu(item)
    const root = menuRoot()
    root.element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await globalItem('[role="separator"]').trigger('click')
    document
        .querySelectorAll<HTMLElement>('[role="menuitem"]')[1]
        .dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(root.isVisible()).toBe(true)
    expect(action).not.toHaveBeenCalled()
    expect(wrapper.emitted('option-clicked')).toBeUndefined()
    expect(wrapper.emitted('menu-closed')).toBeUndefined()
    await globalItem('[role="menuitem"] span').trigger('click')
    expect(wrapper.emitted('option-clicked')?.[0]).toEqual([{ item, option: 'inspect' }])
    expect(wrapper.emitted('menu-closed')).toHaveLength(1)
    vm.close()
    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }))
    expect(wrapper.emitted('menu-closed')).toHaveLength(1)
})

it('closes on outside click and Escape, but respects consumed Escape', async () => {
    const { wrapper } = await menu()
    const root = menuRoot()
    const consumed = new KeyboardEvent('keydown', {
        key: 'Escape',
        bubbles: true,
        cancelable: true,
    })
    consumed.preventDefault()
    root.element.dispatchEvent(consumed)
    expect(root.isVisible()).toBe(true)
    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }))
    await waitFor(() => !root.isVisible())
    expect(wrapper.emitted('menu-closed')).toHaveLength(1)
    const second = await menu()
    document
        .querySelector<HTMLElement>('[role="menu"]')!
        .dispatchEvent(
            new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
        )
    await waitFor(() => !menuRoot().isVisible())
    expect(second.wrapper.emitted('menu-closed')).toHaveLength(1)
})

it('closes other menus once and releases listeners when unmounted', async () => {
    const first = await menu()
    const second = await menu()
    expect(first.wrapper.emitted('menu-closed')).toHaveLength(1)
    expect(menuRoot().isVisible()).toBe(true)
    second.wrapper.unmount()
    const before = second.wrapper.emitted('menu-closed')?.length ?? 0
    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }))
    window.dispatchEvent(new CustomEvent('close-all-context-menus'))
    window.dispatchEvent(new Event('resize'))
    expect(second.wrapper.emitted('menu-closed')?.length ?? 0).toBe(before)
    expect(first.wrapper.emitted('menu-closed')).toHaveLength(1)
})

it('does not render a clickable placeholder for hidden linked-instance actions', async () => {
    const { wrapper, action } = await menu({ instance: { link: true } })
    expect(document.querySelectorAll('[role="menuitem"]')).toHaveLength(3)
    document.querySelectorAll<HTMLElement>('[role="menuitem"]')[2].click()
    await nextTick()
    expect(action).toHaveBeenCalledOnce()
    expect(wrapper.emitted('option-clicked')).toBeUndefined()
})
