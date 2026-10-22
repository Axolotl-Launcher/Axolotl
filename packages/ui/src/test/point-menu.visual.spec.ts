import { afterEach, expect, it, vi } from 'vitest'
import { h } from 'vue'

import PointMenu from '../components/base/PointMenu.vue'
import TeleportOverflowMenu from '../components/base/TeleportOverflowMenu.vue'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

it('positions a measured point menu inside the viewport and restores focus after keyboard action', async () => {
    const action = vi.fn()
    const focus = document.createElement('input')
    document.body.append(focus)
    cleanup.push(() => focus.remove())
    focus.focus()
    const wrapper = await mountThemed(
        PointMenu,
        {
            options: [
                { id: 'disabled', label: 'Disabled', disabled: true },
                { id: 'inspect', label: 'Inspect', action },
            ],
        },
        'dark',
    )
    cleanup.push(() => wrapper.unmount())
    wrapper.vm.show(
        new MouseEvent('contextmenu', { clientX: innerWidth - 2, clientY: innerHeight - 2 }),
    )
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Inspect')
    const menu = document.querySelector<HTMLElement>('[role="menu"]')!
    await waitFor(() => menu.getBoundingClientRect().right <= innerWidth - 9)
    expect(menu.getBoundingClientRect().bottom).toBeLessThanOrEqual(innerHeight - 9)
    expect(menu.getBoundingClientRect().width).toBeGreaterThan(70)
    document.activeElement!.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    )
    await waitFor(() => !document.querySelector('[role="menu"]'))
    await waitFor(() => document.activeElement === focus)
    expect(action).toHaveBeenCalledOnce()
})

it('uses the same actions and roving focus in the former teleported menu without acquiring body locks', async () => {
    const action = vi.fn()
    const wrapper = await mountThemed(
        TeleportOverflowMenu,
        {
            label: 'More',
            options: [
                { id: 'unavailable', label: 'Unavailable', action, disabled: true },
                { id: 'copy', label: 'Copy', action },
            ],
        },
        'dark',
        { slots: { default: () => h('span', 'More') } },
    )
    cleanup.push(() => wrapper.unmount())
    const previous = document.body.style.overflow
    const trigger = wrapper.get('button').element as HTMLButtonElement
    trigger.focus()
    trigger.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    )
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Copy')
    document.activeElement!.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    )
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(action).toHaveBeenCalledOnce()
    expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({ id: 'copy' })
    expect(document.body.style.overflow).toBe(previous)
    await waitFor(() => document.activeElement === trigger)
})

it('isolates trigger clicks from row actions and prevents disabled hover opens', async () => {
    const parent = document.createElement('div')
    const click = vi.fn()
    const drag = vi.fn()
    parent.addEventListener('click', click)
    parent.addEventListener('pointerdown', drag)
    document.body.append(parent)
    cleanup.push(() => parent.remove())
    const wrapper = await mountThemed(
        TeleportOverflowMenu,
        { disabled: true, hoverable: true, options: [{ id: 'Inspect' }] },
        'dark',
        { attachTo: parent, slots: { default: 'More' } },
    )
    cleanup.push(() => wrapper.unmount())
    const trigger = wrapper.get('button').element
    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await new Promise((resolve) => requestAnimationFrame(resolve))
    expect(document.querySelector('[role="menu"]')).toBeNull()
    await wrapper.setProps({ disabled: false, hoverable: false })
    trigger.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }))
    trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    expect(click).not.toHaveBeenCalled()
    expect(drag).not.toHaveBeenCalled()
})

it('closes when the original context target unmounts', async () => {
    const origin = document.createElement('button')
    document.body.append(origin)
    cleanup.push(() => origin.remove())
    const wrapper = await mountThemed(PointMenu, { options: [{ id: 'Inspect' }] }, 'dark')
    cleanup.push(() => wrapper.unmount())
    wrapper.vm.show({ clientX: 20, clientY: 20, target: origin })
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    origin.remove()
    await waitFor(() => !document.querySelector('[role="menu"]'))
})

it('keeps a replacement point menu open after the previous button menu finishes restoring focus', async () => {
    const old = await mountThemed(
        TeleportOverflowMenu,
        { options: [{ id: 'Old action' }] },
        'dark',
        { slots: { default: 'More' } },
    )
    cleanup.push(() => old.unmount())
    const trigger = old.get('button').element as HTMLElement
    trigger.focus()
    trigger.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    )
    await waitFor(() => document.activeElement?.textContent?.trim() === 'Old action')
    const next = await mountThemed(PointMenu, { options: [{ id: 'New action' }] }, 'dark')
    cleanup.push(() => next.unmount())
    next.vm.show({ clientX: 40, clientY: 40, target: document.body })
    await waitFor(() => document.activeElement?.textContent?.trim() === 'New action')
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(document.querySelectorAll('[role="menu"]')).toHaveLength(1)
    expect(document.activeElement?.textContent?.trim()).toBe('New action')
})

it('disconnects the origin observer after a menu action dismisses it', async () => {
    const origin = document.createElement('button')
    document.body.append(origin)
    cleanup.push(() => origin.remove())
    const observe = vi.spyOn(MutationObserver.prototype, 'observe')
    const disconnect = vi.spyOn(MutationObserver.prototype, 'disconnect')
    cleanup.push(() => {
        observe.mockRestore()
        disconnect.mockRestore()
    })
    const wrapper = await mountThemed(PointMenu, { options: [{ id: 'Inspect' }] }, 'dark')
    cleanup.push(() => wrapper.unmount())
    wrapper.vm.show({ clientX: 30, clientY: 30, target: origin })
    const originObserver = observe.mock.contexts.at(-1)
    expect(observe.mock.calls.at(-1)).toEqual([document.body, { childList: true, subtree: true }])
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    const before = disconnect.mock.contexts.filter((context) => context === originObserver).length
    document.querySelector<HTMLElement>('[role="menuitem"]')!.click()
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(disconnect.mock.contexts.filter((context) => context === originObserver).length).toBe(
        before + 1,
    )
})
