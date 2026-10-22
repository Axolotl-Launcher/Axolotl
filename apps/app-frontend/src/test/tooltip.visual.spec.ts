import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { afterEach, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, ref, withDirectives } from 'vue'

import { applyTheme, assertTokensLoaded, waitFor } from './visual-harness'

/**
 * The tooltip directive's behaviour, pinned.
 *
 * Written before migrating the component-level overlays, because those are the
 * changes a diff cannot check: a tooltip that renders in the wrong place still
 * type-checks and still builds. The `position`/`z-index` omissions this file
 * guards against were exactly that -- they shipped, and `vite build` passed.
 *
 * The directive is mounted on a real app instance, the same way `main.js` and
 * the website plugin register it, rather than through a component wrapper.
 */

const TRIGGER_STYLE = 'position:absolute;top:200px;left:200px;width:80px;height:32px'

function mountTrigger(
    value: ReturnType<typeof ref<unknown>>,
    modifiers: Record<string, boolean> = {},
    parent: HTMLElement = document.body,
): { trigger: HTMLElement; unmount: () => void } {
    const host = document.createElement('div')
    parent.appendChild(host)

    const app = createApp(
        defineComponent({
            setup: () => () =>
                withDirectives(
                    h('button', { id: 'tooltip-trigger', style: TRIGGER_STYLE }, 'Trigger'),
                    [[tooltipDirective, value.value, undefined, modifiers]],
                ),
        }),
    )
    app.mount(host)

    let disposed = false
    const unmount = () => {
        if (disposed) return
        disposed = true
        app.unmount()
        host.remove()
    }
    cleanup.push(unmount)
    return {
        trigger: host.querySelector('button') as HTMLElement,
        unmount,
    }
}

const popper = () => document.querySelector('.tooltip-popper') as HTMLElement | null

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn())
    vi.restoreAllMocks()
})

function mountGroup() {
    const group = document.createElement('div')
    group.dataset.tooltipGroup = 'toolbar'
    document.body.append(group)
    cleanup.push(() => group.remove())
    const items = ['First', 'Second', 'Third'].map((label, index) => {
        const item = mountTrigger(ref(label), {}, group)
        item.trigger.style.left = `${100 + index * 180}px`
        cleanup.push(item.unmount)
        return item
    })
    return { group, items }
}

const enter = (trigger: HTMLElement) => trigger.dispatchEvent(new MouseEvent('mouseenter'))
const leave = (trigger: HTMLElement) => trigger.dispatchEvent(new MouseEvent('mouseleave'))

it('does not let an older pending hover replace a newer focused target in the group', async () => {
    const { items } = mountGroup()
    enter(items[0].trigger)
    items[1].trigger.focus()
    await waitFor(() => popper()?.parentElement?.dataset.state === 'open')
    const surface = popper()!
    await new Promise((resolve) => setTimeout(resolve, 300))
    expect(surface.textContent).toBe('Second')
    expect(items[0].trigger.hasAttribute('aria-describedby')).toBe(false)
    expect(items[1].trigger.getAttribute('aria-describedby')).toBe(surface.id)
})

it('preserves the first hover delay while truncation observers and values refresh', async () => {
    const value = ref<unknown>({ content: 'A long filename', onlyWhenTruncated: true })
    const { trigger } = mountTrigger(value)
    trigger.style.cssText += ';width:20px;overflow:hidden;white-space:nowrap'
    enter(trigger)
    trigger.style.width = '24px'
    value.value = { content: 'Another long filename', onlyWhenTruncated: true }
    await nextTick()
    await new Promise((resolve) => setTimeout(resolve, 80))
    expect(popper()).toBeNull()
    await waitFor(() => popper()?.textContent === 'Another long filename')
})

it('delays first hover, then moves the same surface immediately to the latest grouped trigger', async () => {
    applyTheme('dark')
    const { items } = mountGroup()
    const [a, b, c] = items.map((item) => item.trigger)
    enter(a)
    await new Promise((resolve) => setTimeout(resolve, 80))
    expect(popper()).toBeNull()
    await waitFor(() => popper()?.parentElement?.dataset.state === 'open')
    const surface = popper()!
    const positioner = surface.parentElement!
    await waitFor(() => getComputedStyle(surface).opacity === '1')
    const from = positioner.getBoundingClientRect().left
    leave(a)
    enter(b)
    expect(popper()).toBe(surface)
    expect(surface.textContent).toBe('Second')
    await waitFor(() => positioner.getBoundingClientRect().left > from + 1)
    expect(positioner.getBoundingClientRect().left).toBeLessThan(b.getBoundingClientRect().left)
    leave(b)
    enter(c)
    leave(c)
    enter(a)
    expect(surface.textContent).toBe('First')
    await waitFor(() => Math.abs(positioner.getBoundingClientRect().left - from) < 1)
    expect(document.querySelectorAll('[role="tooltip"]')).toHaveLength(1)
    leave(a)
    await waitFor(() => positioner.dataset.state === 'closed')
    expect(positioner.isConnected).toBe(true)
    await waitFor(() => !positioner.isConnected)
})

it('transfers descriptions on keyboard focus without deleting existing descriptions', async () => {
    const { items } = mountGroup()
    const [a, b] = items.map((item) => item.trigger)
    a.setAttribute('aria-describedby', 'existing-help')
    a.focus()
    await waitFor(() => !!popper())
    const surface = popper()!
    expect(a.getAttribute('aria-describedby')).toBe(`existing-help ${surface.id}`)
    b.focus()
    expect(popper()).toBe(surface)
    expect(surface.textContent).toBe('Second')
    expect(a.getAttribute('aria-describedby')).toBe('existing-help')
    expect(b.getAttribute('aria-describedby')).toBe(surface.id)
    b.blur()
    await waitFor(() => !popper())
    expect(b.hasAttribute('aria-describedby')).toBe(false)
})

it('keeps nested toolbar, page and dialog groups isolated', async () => {
    const { group, items } = mountGroup()
    const page = items[0].trigger
    page.focus()
    await waitFor(() => !!popper())
    const pageSurface = popper()!
    for (const boundary of ['toolbar', 'dialog']) {
        const container = document.createElement('div')
        if (boundary === 'toolbar') container.dataset.tooltipGroup = 'toolbar'
        else container.setAttribute('role', 'dialog')
        group.append(container)
        const item = mountTrigger(ref(boundary), {}, container)
        cleanup.push(item.unmount)
        enter(item.trigger)
        expect(document.querySelectorAll('[role="tooltip"]')).toHaveLength(1)
        await waitFor(() => document.querySelectorAll('[role="tooltip"]').length === 2)
        expect(pageSurface.textContent).toBe('First')
        item.unmount()
    }
})

it('remeasures multiline content and flips the arrow when the grouped target reaches an edge', async () => {
    applyTheme('dark')
    const { group, items } = mountGroup()
    const value = ref('A long description '.repeat(40))
    const item = mountTrigger(value, {}, group)
    cleanup.push(item.unmount)
    item.trigger.style.cssText =
        'position:fixed;left:calc(100vw - 40px);top:0;width:32px;height:32px'
    items[0].trigger.focus()
    await waitFor(() => popper()?.parentElement?.dataset.state === 'open')
    const surface = popper()!
    item.trigger.focus()
    await waitFor(
        () =>
            surface.querySelector<HTMLElement>('.tooltip-popper-arrow')?.dataset.side === 'bottom',
    )
    await waitFor(() => surface.parentElement!.getBoundingClientRect().top >= 32)
    const box = surface.getBoundingClientRect()
    expect(box.height).toBeGreaterThan(50)
    expect(box.right).toBeLessThanOrEqual(innerWidth - 7)
    expect(box.left).toBeGreaterThanOrEqual(7)
    value.value = 'Short'
    await nextTick()
    await waitFor(() => surface.getBoundingClientRect().height < 50)
    expect(surface.textContent).toBe('Short')
})

it('cancels exit on reentry and clears pending show and exit tasks on unmount', async () => {
    const { items } = mountGroup()
    const [a, b, c] = items.map((item) => item.trigger)
    a.focus()
    await waitFor(() => popper()?.parentElement?.dataset.state === 'open')
    const surface = popper()!
    a.blur()
    await waitFor(() => surface.parentElement!.dataset.state === 'closed')
    enter(b)
    expect(popper()).toBe(surface)
    expect(surface.textContent).toBe('Second')
    items[0].unmount()
    expect(popper()).toBe(surface)
    items[1].unmount()
    expect(popper()).toBeNull()
    enter(c)
    items[2].unmount()
    await new Promise((resolve) => setTimeout(resolve, 300))
    expect(document.querySelector('.tooltip-positioner')).toBeNull()
})

it('removes a closed tooltip immediately when reduced motion is requested', async () => {
    const original = window.matchMedia.bind(window)
    vi.spyOn(window, 'matchMedia').mockImplementation((query) => {
        const result = original(query)
        if (query.includes('prefers-reduced-motion'))
            Object.defineProperty(result, 'matches', { value: true })
        return result
    })
    const { items } = mountGroup()
    items[0].trigger.focus()
    await waitFor(() => popper()?.parentElement?.dataset.state === 'open')
    const positioner = popper()!.parentElement!
    items[0].trigger.blur()
    await waitFor(() => positioner.dataset.state === 'closed')
    expect(positioner.isConnected).toBe(false)
})

it('loads the token layer', () => {
    assertTokensLoaded()
})

it('does not render a popper for a suppressed value', async () => {
    const value = ref<unknown>(undefined)
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    // Give any incorrectly-scheduled show a chance to run.
    await new Promise((resolve) => setTimeout(resolve, 400))

    expect(popper()).toBeNull()
    unmount()
})

it('renders the popper with working position and stacking', async () => {
    // The two properties whose omission made every tooltip land in the wrong
    // place and paint under the app's own layers.
    const value = ref<unknown>('Hello')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip to appear' })

    const el = popper() as HTMLElement
    const style = getComputedStyle(el.parentElement!)
    expect(style.position, 'out of flow, or the transform is relative to the page').toBe('fixed')
    expect(Number(style.zIndex), 'above the app chrome').toBeGreaterThanOrEqual(1000)
    expect(el.textContent).toContain('Hello')

    unmount()
})

it('renders an outlined arrow for the settled tooltip placement', async () => {
    const value = ref<unknown>('Arrow')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper()?.querySelector('.tooltip-popper-arrow'), {
        label: 'the tooltip arrow to render',
    })

    const arrow = popper()?.querySelector('.tooltip-popper-arrow') as HTMLElement
    expect(arrow.getAttribute('aria-hidden')).toBe('true')
    expect(arrow.dataset.side).toBe('top')
    expect(getComputedStyle(arrow, '::before').borderTopWidth).toBe('7px')

    unmount()
})

it('positions the popper adjacent to its trigger', async () => {
    const value = ref<unknown>('Adjacent')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip' })
    // The transform is applied asynchronously after the position is computed.
    await waitFor(() => (popper() as HTMLElement).parentElement!.style.transform !== '', {
        label: 'positioning to settle',
    })

    const triggerBox = trigger.getBoundingClientRect()
    const popperBox = (popper() as HTMLElement).getBoundingClientRect()

    // The default placement is `top`, so the popper must sit above the trigger
    // and overlap it horizontally. A popper left in normal flow would land at
    // the document's bottom instead, which is the failure this catches.
    expect(popperBox.bottom).toBeLessThanOrEqual(triggerBox.top + 1)
    expect(popperBox.top).toBeGreaterThan(0)
    expect(popperBox.right).toBeGreaterThan(triggerBox.left)
    expect(popperBox.left).toBeLessThan(triggerBox.right)

    unmount()
})

it('honours a placement modifier', async () => {
    const value = ref<unknown>('To the right')
    const { trigger, unmount } = mountTrigger(value, { right: true })
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip' })
    await waitFor(() => (popper() as HTMLElement).parentElement!.style.transform !== '', {
        label: 'positioning to settle',
    })

    const triggerBox = trigger.getBoundingClientRect()
    const popperBox = (popper() as HTMLElement).getBoundingClientRect()
    expect(
        popperBox.left,
        'a `right` tooltip sits to the right of its trigger',
    ).toBeGreaterThanOrEqual(triggerBox.right - 1)

    unmount()
})

it('announces the tooltip only while it is shown', async () => {
    const value = ref<unknown>('Described')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    expect(trigger.hasAttribute('aria-describedby')).toBe(false)

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip' })

    const describedBy = trigger.getAttribute('aria-describedby')
    expect(describedBy, 'a shown tooltip should describe its trigger').toBeTruthy()
    expect(document.getElementById(describedBy as string), 'the id must resolve').toBeTruthy()

    trigger.dispatchEvent(new MouseEvent('mouseleave'))
    await waitFor(() => !popper(), { label: 'the tooltip to hide' })
    expect(trigger.hasAttribute('aria-describedby')).toBe(false)

    unmount()
})

it('renders text as text and markup only for the html option', async () => {
    const value = ref<unknown>('<b>Bold</b>')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the text tooltip' })

    // Without `html`, markup must stay inert: this is the property that keeps an
    // untrusted string from becoming HTML.
    expect((popper() as HTMLElement).querySelector('b')).toBeNull()
    expect((popper() as HTMLElement).textContent).toContain('<b>Bold</b>')

    trigger.dispatchEvent(new MouseEvent('mouseleave'))
    await waitFor(() => !popper(), { label: 'the tooltip to hide' })

    value.value = { content: '<b>Bold</b>', html: true }
    await nextTick()
    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the html tooltip' })
    expect((popper() as HTMLElement).querySelector('b'), 'html: true renders markup').toBeTruthy()

    unmount()
})

it('applies a popperClass to the popper', async () => {
    const value = ref<unknown>({ content: 'Styled', popperClass: 'storage-tooltip' })
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip' })

    // Several components restyle the popper through this class; losing it would
    // silently change their appearance.
    expect((popper() as HTMLElement).classList.contains('storage-tooltip')).toBe(true)

    unmount()
})

it('honours a hover-only trigger configuration', async () => {
    const value = ref<unknown>({ content: 'Hover only', triggers: ['hover'] })
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.focus()
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(popper(), 'focus must not activate a hover-only tooltip').toBeNull()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the hover-only tooltip' })

    unmount()
})

it('keeps a tooltip open while another enabled trigger remains active', async () => {
    const value = ref<unknown>('Persistent while focused')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.focus()
    await waitFor(() => !!popper(), { label: 'the focused tooltip' })
    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    trigger.dispatchEvent(new MouseEvent('mouseleave'))
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(popper(), 'hover ending must not hide a focused tooltip').not.toBeNull()

    trigger.blur()
    await waitFor(() => !popper(), { label: 'the tooltip to hide after focus leaves' })
    unmount()
})

it('re-reads its content when the value changes', async () => {
    // The value is deliberately not captured from the binding object passed to
    // `mounted`: Vue replaces that object on re-render, so a captured one would
    // keep serving the first render's text.
    const value = ref<unknown>('first')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip' })
    expect((popper() as HTMLElement).textContent).toContain('first')

    value.value = 'second'
    await nextTick()
    trigger.dispatchEvent(new MouseEvent('mouseleave'))
    await waitFor(() => !popper(), { label: 'the tooltip to hide' })

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(
        () => (popper() as HTMLElement | null)?.textContent?.includes('second') ?? false,
        {
            label: 'the updated content',
        },
    )

    unmount()
})

it('removes its popper when the trigger unmounts', async () => {
    const value = ref<unknown>('Transient')
    const { trigger, unmount } = mountTrigger(value)
    applyTheme('dark')
    await nextTick()

    trigger.dispatchEvent(new MouseEvent('mouseenter'))
    await waitFor(() => !!popper(), { label: 'the tooltip' })

    // The popper lives under the teleport target, not inside the trigger's
    // subtree, so nothing would clean it up implicitly.
    unmount()
    await waitFor(() => !popper(), { label: 'the tooltip to be cleaned up' })
})
