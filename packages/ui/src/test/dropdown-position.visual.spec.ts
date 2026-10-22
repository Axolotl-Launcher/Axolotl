import { afterEach, expect, it } from 'vitest'
import { h, nextTick, ref } from 'vue'

import Combobox from '../components/base/Combobox.vue'
import MultiSelect from '../components/base/MultiSelect.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed, waitFor } from './visual-harness'

const cleanups: (() => void)[] = []
afterEach(() =>
    cleanups
        .splice(0)
        .reverse()
        .forEach((cleanup) => cleanup()),
)

for (const component of [Combobox, MultiSelect]) {
    it(`${component.__name} preserves CSS width keywords while bounding oversized minimum widths`, async () => {
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        document.body.append(teleports)
        cleanups.push(() => teleports.remove())
        const wrapper = await mountThemed(
            component,
            {
                options: [{ value: 'a', label: 'A' }],
                modelValue: component === Combobox ? 'a' : [],
                dropdownWidth: 'max-content',
                dropdownMinWidth: 'min-content',
            },
            'dark',
            {
                global: {
                    provide: {
                        [I18N_INJECTION_KEY as symbol]: {
                            locale: ref('en-US'),
                            t: (key: string) => key,
                            setLocale: () => undefined,
                        },
                    },
                },
            },
        )
        cleanups.push(() => wrapper.unmount())
        wrapper
            .get('[aria-haspopup="listbox"]')
            .element.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }))
        const panel = () => document.querySelector<HTMLElement>('[role="listbox"]')!
        await waitFor(() => !!panel() && getComputedStyle(panel()).visibility !== 'hidden')
        expect(panel().style.width).toBe('max-content')
        expect(panel().style.minWidth).toBe('min-content')
        await wrapper.setProps({ dropdownWidth: 'auto', dropdownMinWidth: 2000 })
        await waitFor(
            () =>
                panel().style.width === 'auto' &&
                panel().getBoundingClientRect().width <= window.innerWidth - 15,
        )
    })

    it(`${component.__name} constrains a long list when neither side fits and keeps the last option reachable`, async () => {
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        const host = document.createElement('div')
        host.style.cssText = 'position:fixed; top:45vh; right:4px; width:180px'
        document.body.append(teleports, host)
        cleanups.push(() => {
            host.remove()
            teleports.remove()
        })
        const options = Array.from({ length: 30 }, (_, index) => ({
            value: index,
            label: `Option ${index}`,
        }))
        const wrapper = await mountThemed(
            component,
            {
                options,
                modelValue: component === Combobox ? 0 : [],
                maxHeight: 1000,
                dropdownWidth: 600,
                searchable: component === MultiSelect,
            },
            'dark',
            {
                attachTo: host,
                slots:
                    component === MultiSelect
                        ? {
                              bottom: () =>
                                  h(
                                      'div',
                                      { style: 'height:400px; flex-shrink:0' },
                                      'Additional filters',
                                  ),
                          }
                        : {},
                global: {
                    provide: {
                        [I18N_INJECTION_KEY as symbol]: {
                            locale: ref('en-US'),
                            t: (key: string) => key,
                            setLocale: () => undefined,
                        },
                    },
                },
            },
        )
        cleanups.push(() => wrapper.unmount())
        const trigger = wrapper.get('[aria-haspopup="listbox"]').element as HTMLElement
        trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }))
        const panel = () => document.querySelector<HTMLElement>('[role="listbox"]')
        await waitFor(
            () =>
                !!panel() &&
                getComputedStyle(panel()!).visibility !== 'hidden' &&
                panel()!.getBoundingClientRect().height > 0,
        )
        const assertBounds = () => {
            const bounds = panel()!.getBoundingClientRect()
            expect(bounds.top).toBeGreaterThanOrEqual(7)
            expect(bounds.bottom).toBeLessThanOrEqual(window.innerHeight - 7)
            expect(bounds.left).toBeGreaterThanOrEqual(7)
            expect(bounds.right).toBeLessThanOrEqual(window.innerWidth - 7)
        }
        assertBounds()
        const scroll = panel()!.querySelector<HTMLElement>('[data-overlayscrollbars-viewport]')!
        expect(scroll.scrollHeight).toBeGreaterThan(scroll.clientHeight)
        scroll.scrollTop = scroll.scrollHeight
        const last = Array.from(panel()!.querySelectorAll<HTMLElement>('[role="option"]')).at(-1)!
        last.scrollIntoView({ block: 'nearest' })
        await waitFor(
            () =>
                last.getBoundingClientRect().bottom <= panel()!.getBoundingClientRect().bottom + 1,
        )
        last.click()
        await nextTick()
        expect(wrapper.emitted('update:modelValue')?.at(-1)?.[0]).toEqual(
            component === Combobox ? 29 : [29],
        )
        if (component === Combobox)
            trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }))
        await waitFor(() => !!panel() && getComputedStyle(panel()!).visibility !== 'hidden')
        host.style.top = 'calc(100vh - 60px)'
        await waitFor(
            () => panel()!.getBoundingClientRect().bottom <= trigger.getBoundingClientRect().top,
        )
        assertBounds()
        trigger.dispatchEvent(
            new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
        )
        await waitFor(() => !panel())
    })

    it(`${component.__name} ignores positioning work after closing and unmounting`, async () => {
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        document.body.append(teleports)
        cleanups.push(() => teleports.remove())
        const wrapper = await mountThemed(
            component,
            {
                options: [{ value: 'a', label: 'A' }],
                modelValue: component === Combobox ? 'a' : [],
            },
            'dark',
            {
                global: {
                    provide: {
                        [I18N_INJECTION_KEY as symbol]: {
                            locale: ref('en-US'),
                            t: (key: string) => key,
                            setLocale: () => undefined,
                        },
                    },
                },
            },
        )
        const trigger = wrapper.get('[aria-haspopup="listbox"]').element as HTMLElement
        trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }))
        trigger.dispatchEvent(
            new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
        )
        wrapper.unmount()
        await nextTick()
        await new Promise((resolve) => setTimeout(resolve, 100))
        expect(document.querySelector('[role="listbox"]')).toBeNull()
    })
}
