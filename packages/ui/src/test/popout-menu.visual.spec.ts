import { userEvent } from '@vitest/browser/context'
import { DropdownMenuItem } from 'reka-ui'
import { expect, it } from 'vitest'
import { h } from 'vue'

import PopoutMenu from '../components/base/PopoutMenu.vue'
import { applyTheme, assertTokensLoaded, mountThemed, waitFor } from './visual-harness'

/**
 * `PopoutMenu` is the highest-fanout wrapper in the shared package -- it backs
 * `OverflowMenu`, which is used across the app -- so its behaviour is pinned
 * here rather than discovered in the app.
 *
 * The migration from floating-vue to reka-ui changed how the menu opens,
 * positions and traps focus. None of that is visible in a diff, and all of it
 * would still type-check and build.
 */

/** reka-ui's trigger responds to pointerdown, not a synthetic click. */
function openMenu() {
    const btn = document.querySelector('button[aria-haspopup="menu"]') as HTMLElement
    btn?.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }))
    btn?.dispatchEvent(new MouseEvent('click', { bubbles: true }))
}

const content = () => document.querySelector('.menu-surface') as HTMLElement | null

async function mountPopout(props: Record<string, unknown> = {}) {
    const wrapper = await mountThemed(PopoutMenu, { menu: true, ...props }, 'dark', {
        attachTo: document.body,
        slots: {
            default: '<span id="popout-trigger">Open</span>',
            menu: () =>
                h(
                    DropdownMenuItem,
                    { asChild: true },
                    { default: () => h('button', { id: 'menu-item' }, 'Item') },
                ),
        },
    })
    return wrapper
}

it('loads the token layer', () => {
    assertTokensLoaded()
})

it('is closed until the trigger is activated', async () => {
    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    expect(content(), 'the menu should not be in the DOM before opening').toBeNull()

    wrapper.unmount()
})

it('opens on trigger click and renders its menu slot', async () => {
    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    openMenu()
    await waitFor(() => !!content(), { label: 'the menu to open' })

    expect(content()?.textContent).toContain('Item')

    wrapper.unmount()
})

it('paints the menu surface so it is not a transparent box', async () => {
    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    openMenu()
    await waitFor(() => !!content(), { label: 'the menu' })

    // Reproduces what the floating-vue theme supplied: a raised surface with a
    // stroke. Losing it would leave menu items floating over the page.
    const el = content() as HTMLElement
    const style = getComputedStyle(el)
    expect(style.backgroundColor, 'the menu has a surface background').not.toBe('rgba(0, 0, 0, 0)')
    expect(Number(style.zIndex), 'the menu stacks above app chrome').toBeGreaterThanOrEqual(1000)
    expect(style.borderTopWidth, 'the menu has a stroke').toBe('1px')

    wrapper.unmount()
})

it('closes on Escape', async () => {
    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    openMenu()
    await waitFor(() => !!content(), { label: 'the menu to open' })

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await waitFor(() => !content(), { label: 'the menu to close on Escape' })

    wrapper.unmount()
})

it('exposes show() and hide() for callers that drive it programmatically', async () => {
    // `OverflowMenu` calls both through a template ref, so the exposed API is a
    // real contract rather than a convenience.
    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    const vm = wrapper.vm as unknown as { show: () => void; hide: () => void }
    vm.show()
    await waitFor(() => !!content(), { label: 'show() to open the menu' })

    vm.hide()
    await waitFor(() => !content(), { label: 'hide() to close the menu' })

    wrapper.unmount()
})

it('accepts a dropdownClass on the content element', async () => {
    const wrapper = await mountPopout({ dropdownClass: 'seed-map-biome-popout' })
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    openMenu()
    await waitFor(() => !!content(), { label: 'the menu' })

    // Four call sites restyle the menu through this prop; landing it on the
    // wrong element would silently drop their CSS.
    expect(content()?.classList.contains('seed-map-biome-popout')).toBe(true)

    wrapper.unmount()
})

it('uses dropdownId as the menu element id', async () => {
    const wrapper = await mountPopout({ dropdownId: 'create-new-files' })
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    openMenu()
    await waitFor(() => !!content(), { label: 'the menu' })

    expect(content()?.id).toBe('create-new-files')
    expect(content()?.getAttribute('aria-label')).toBeNull()

    wrapper.unmount()
})

it('moves keyboard focus into the menu and restores it after closing', async () => {
    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    const trigger = document.querySelector('button[aria-haspopup="menu"]') as HTMLElement
    trigger.focus()
    trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    await waitFor(() => document.activeElement?.id === 'menu-item', {
        label: 'focus to move to the first menu item',
    })

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await waitFor(() => document.activeElement === trigger, {
        label: 'focus to return to the menu trigger',
    })

    wrapper.unmount()
})

it('supports a controlled open model and a custom trigger slot', async () => {
    const wrapper = await mountThemed(PopoutMenu, { open: false }, 'dark', {
        attachTo: document.body,
        slots: {
            trigger: '<button id="custom-popout-trigger">Open</button>',
            menu: '<button id="custom-popout-item">Item</button>',
        },
    })
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    await wrapper.setProps({ open: true })
    await waitFor(() => !!content(), { label: 'the controlled menu to open' })
    expect(document.querySelector('#custom-popout-trigger')?.getAttribute('aria-expanded')).toBe(
        'true',
    )

    await wrapper.setProps({ open: false })
    await waitFor(() => !content(), { label: 'the controlled menu to close' })

    wrapper.unmount()
})

it('does not move focus when opened with the pointer', async () => {
    // The menu is a dropdown, not a modal: clicking its trigger must leave the
    // caret where the user put it. Moving focus to the first item would pull it
    // out of whatever field was being edited, which is what reka does by default.
    const input = document.createElement('input')
    input.id = 'popout-outside-input'
    document.body.append(input)
    input.focus()

    const wrapper = await mountPopout()
    applyTheme('dark')
    await wrapper.vm.$nextTick()

    openMenu()
    await waitFor(() => !!content(), { label: 'the menu to open' })
    await new Promise((resolve) => setTimeout(resolve, 80))

    expect((document.activeElement as HTMLElement)?.id, 'a pointer open must not steal focus').toBe(
        'popout-outside-input',
    )

    wrapper.unmount()
    input.remove()
})

it('lets arbitrary panel controls use native focus and Tab navigation', async () => {
    const wrapper = await mountThemed(PopoutMenu, {}, 'dark', {
        slots: {
            default: 'Notifications',
            menu: '<button id="panel-first">Clear</button><button id="panel-next">Dismiss</button>',
        },
    })
    const trigger = wrapper.get('button[aria-haspopup="dialog"]').element as HTMLElement
    trigger.focus()
    await userEvent.keyboard('{Enter}')
    await waitFor(() => document.activeElement?.id === 'panel-first')
    await userEvent.keyboard('{Tab}')
    expect(document.activeElement?.id).toBe('panel-next')
    await userEvent.keyboard('{Escape}')
    await waitFor(() => !content())
    await waitFor(() => document.activeElement === trigger)
    await userEvent.keyboard('{ArrowDown}')
    await waitFor(() => document.activeElement?.id === 'panel-first')
    await userEvent.keyboard('{Escape}')
    await waitFor(() => !content())
    wrapper.unmount()
})
