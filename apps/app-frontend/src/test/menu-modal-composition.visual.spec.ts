import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import { mountThemed, waitFor } from '@/test/visual-harness'

import NewModal from '../../../../packages/ui/src/components/modal/NewModal.vue'
import { I18N_INJECTION_KEY } from '../../../../packages/ui/src/providers/i18n'

vi.mock('@modrinth/ui', async () => ({
    PointMenu: (await import('../../../../packages/ui/src/components/base/PointMenu.vue')).default,
}))
const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((stop) => stop()),
)

async function setup() {
    const originalOverflow = document.body.style.cssText
    document.body.style.setProperty('overflow', 'scroll', 'important')
    cleanup.push(() => {
        document.body.style.cssText = originalOverflow
    })
    const host = document.createElement('div')
    host.id = 'teleports'
    document.body.append(host)
    cleanup.push(() => host.remove())
    const menu = ref<InstanceType<typeof ContextMenu>>()
    const target = { id: 'world-one' }
    const selected = vi.fn()
    const Child = defineComponent({
        setup: () => () =>
            h('div', [
                h(
                    'button',
                    {
                        id: 'object-trigger',
                        onContextmenu: (event: MouseEvent) => {
                            event.preventDefault()
                            menu.value?.showMenu(event, target, [
                                { name: 'inspect', label: 'Inspect' },
                            ])
                        },
                    },
                    'World',
                ),
                h(ContextMenu, { ref: menu, onOptionClicked: selected }),
            ]),
    })
    const wrapper = await mountThemed(NewModal, { header: 'World tools' }, 'dark', {
        slots: { default: () => h(Child) },
        global: {
            directives: { tooltip: {} },
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => undefined,
                },
            },
        },
    })
    cleanup.push(() => wrapper.unmount())
    wrapper.vm.show()
    await waitFor(() => !!document.querySelector('.modal-container.shown'))
    await waitFor(() => getComputedStyle(document.querySelector('.modal-body')!).opacity === '1')
    const trigger = document.querySelector<HTMLElement>('#object-trigger')!
    trigger.focus()
    trigger.dispatchEvent(
        new MouseEvent('contextmenu', {
            bubbles: true,
            cancelable: true,
            clientX: 200,
            clientY: 200,
        }),
    )
    await waitFor(() => document.activeElement?.getAttribute('role') === 'menuitem')
    return { wrapper, trigger, target, selected }
}

it('dismisses only the business menu with Escape and keeps the parent focus and scroll lock', async () => {
    const { trigger } = await setup()
    document.activeElement!.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    )
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(document.querySelector('.modal-container.shown')).not.toBeNull()
    await waitFor(() => document.activeElement === trigger)
    expect(document.body.style.overflow).toBe('hidden')
    trigger.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    )
    await waitFor(() => !document.querySelector('[role="dialog"]'))
    expect(document.body.style.overflow).toBe('scroll')
    expect(document.body.style.getPropertyPriority('overflow')).toBe('important')
})

it('targets the current business object once and releases its menu when the parent unmounts', async () => {
    const { wrapper, trigger, target, selected } = await setup()
    document.querySelector<HTMLElement>('[role="menuitem"]')!.click()
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(selected).toHaveBeenCalledExactlyOnceWith({ item: target, option: 'inspect' })
    expect(document.body.style.overflow).toBe('hidden')
    trigger.focus()
    trigger.dispatchEvent(
        new MouseEvent('contextmenu', {
            bubbles: true,
            cancelable: true,
            clientX: 200,
            clientY: 200,
        }),
    )
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    wrapper.unmount()
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    expect(document.body.style.overflow).toBe('scroll')
})
