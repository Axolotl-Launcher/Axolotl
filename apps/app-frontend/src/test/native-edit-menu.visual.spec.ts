import { afterEach, expect, it, vi } from 'vitest'

import { installNativeEditMenuBoundary } from '@/helpers/native-edit-menu'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

it('keeps the native edit event uncancelled inside business right-click containers', () => {
    const host = document.createElement('div')
    host.innerHTML =
        '<input type="search"><input type="text"><textarea></textarea><div contenteditable="true"><span>Editable</span></div><button>Object</button>'
    document.body.append(host)
    cleanup.push(() => host.remove())
    const businessMenu = vi.fn((event: Event) => event.preventDefault())
    host.addEventListener('contextmenu', businessMenu)
    const stop = installNativeEditMenuBoundary(true)
    cleanup.push(stop)
    for (const field of host.querySelectorAll('input,textarea,[contenteditable] span')) {
        const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
        field.dispatchEvent(event)
        expect(event.defaultPrevented).toBe(false)
    }
    expect(businessMenu).not.toHaveBeenCalled()
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    host.querySelector('button')!.dispatchEvent(event)
    expect(businessMenu).toHaveBeenCalledOnce()
    expect(event.defaultPrevented).toBe(true)
    stop()
    host.querySelector('input')!.dispatchEvent(
        new MouseEvent('contextmenu', { bubbles: true, cancelable: true }),
    )
    expect(businessMenu).toHaveBeenCalledTimes(2)
})

it('allows selected page text and preserves development page menus', () => {
    const label = document.createElement('p')
    label.textContent = 'Selectable text'
    document.body.append(label)
    cleanup.push(() => {
        window.getSelection()?.removeAllRanges()
        label.remove()
    })
    const stop = installNativeEditMenuBoundary(true)
    cleanup.push(stop)
    const range = document.createRange()
    range.selectNodeContents(label)
    window.getSelection()!.addRange(range)
    const selected = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    label.dispatchEvent(selected)
    expect(selected.defaultPrevented).toBe(false)
    window.getSelection()!.removeAllRanges()
    const page = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    label.dispatchEvent(page)
    expect(page.defaultPrevented).toBe(true)
    stop()
    cleanup.push(installNativeEditMenuBoundary(false))
    const dev = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    label.dispatchEvent(dev)
    expect(dev.defaultPrevented).toBe(false)
})
