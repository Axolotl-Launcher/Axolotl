import { expect, it } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import Combobox, { type ComboboxOption } from '../components/base/Combobox.vue'
import { mountThemed, waitFor } from './visual-harness'

it('emits the current searchable query after the input model updates', async () => {
    const teleports = installTeleportTarget()
    const wrapper = await mountThemed(
        Combobox,
        {
            searchable: true,
            syncWithSelection: false,
            minSearchLengthToOpen: 2,
            options: [{ value: 'alpha', label: 'Alpha' }],
        },
        'dark',
    )
    try {
        await wrapper.get('input').setValue('a')
        expect(wrapper.emitted('searchInput')?.[0]).toEqual(['a'])
        expect(document.querySelector('[role="listbox"]')).toBeNull()
        await wrapper.get('input').setValue('al')
        expect(wrapper.emitted('searchInput')?.[1]).toEqual(['al'])
        await waitFor(() => !!document.querySelector('[role="listbox"]'))
    } finally {
        wrapper.unmount()
        teleports.remove()
    }
})

for (const searchable of [false, true]) {
    it(`fills a form region and respects externally sized ${searchable ? 'searchable' : 'standard'} controls`, async () => {
        const teleports = installTeleportTarget()
        const host = document.createElement('div')
        host.style.width = '360px'
        document.body.append(host)
        const wrapper = await mountThemed(
            Combobox,
            {
                options: [{ value: 'all', label: 'All tools' }],
                modelValue: 'all',
                searchable,
            },
            'dark',
            { attachTo: host },
        )
        try {
            expect(wrapper.element.getBoundingClientRect().width).toBe(360)
            await wrapper.setProps({ width: 'custom', class: 'w-48' })
            expect(wrapper.element.getBoundingClientRect().width).toBe(192)
            host.style.width = '140px'
            expect(wrapper.element.getBoundingClientRect().width).toBe(140)
            const trigger = wrapper.get(searchable ? 'input' : '[aria-haspopup="listbox"]').element
            expect(trigger.getBoundingClientRect().width).toBeLessThanOrEqual(140)
        } finally {
            wrapper.unmount()
            host.remove()
            teleports.remove()
        }
    })
}

it('fits toolbar content and shrinks long labels without squeezing adjacent actions', async () => {
    const teleports = installTeleportTarget()
    const host = document.createElement('div')
    host.style.cssText = 'width:360px;display:flex;gap:8px'
    document.body.append(host)
    const action = document.createElement('button')
    action.textContent = 'Action'
    action.style.cssText = 'width:48px;flex-shrink:0'
    const wrapper = await mountThemed(
        Combobox,
        {
            width: 'content',
            modelValue: 'name',
            options: [
                { value: 'name', label: 'Name' },
                { value: 'long', label: 'An unusually long translated sorting option'.repeat(4) },
            ],
        },
        'dark',
        { attachTo: host },
    )
    wrapper.element.parentElement!.style.display = 'contents'
    host.append(action)
    try {
        const shortWidth = wrapper.element.getBoundingClientRect().width
        expect(shortWidth).toBeGreaterThan(40)
        expect(shortWidth).toBeLessThan(300)
        await wrapper.setProps({ modelValue: 'long' })
        host.style.width = '220px'
        expect(wrapper.element.getBoundingClientRect().width).toBeLessThanOrEqual(164)
        expect(action.getBoundingClientRect().width).toBe(48)
        expect(host.scrollWidth).toBe(220)
        activate(wrapper.get('[aria-haspopup="listbox"]').element as HTMLElement)
        await waitFor(() => !!document.querySelector('[role="listbox"]'))
        const dropdown = document.querySelector('[role="listbox"]') as HTMLElement
        expect(dropdown.getBoundingClientRect().width).toBeCloseTo(
            wrapper.element.getBoundingClientRect().width,
            0,
        )
    } finally {
        wrapper.unmount()
        host.remove()
        teleports.remove()
    }
})

function installTeleportTarget() {
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    return teleports
}

function activate(element: HTMLElement) {
    element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }))
    element.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }))
    element.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, button: 0 }))
    element.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }))
}

it('preserves scalar values while showing formatted option labels', async () => {
    const teleports = installTeleportTarget()
    const options: ComboboxOption<number>[] = [
        { value: 10, label: '10 results' },
        { value: 25, label: '25 results' },
    ]
    const Harness = defineComponent({
        setup() {
            const selected = ref(10)
            return () =>
                h(Combobox, {
                    modelValue: selected.value,
                    options,
                    'onUpdate:modelValue': (value: number) => {
                        selected.value = value
                    },
                })
        },
    })
    const wrapper = await mountThemed(Harness, {}, 'dark', { attachTo: document.body })

    const trigger = wrapper.get('[aria-haspopup="listbox"]')
    expect(trigger.text()).toContain('10 results')
    activate(trigger.element as HTMLElement)
    await waitFor(() => document.querySelectorAll('[role="option"]').length === 2, {
        label: 'number options to open',
    })

    const nextOption = Array.from(document.querySelectorAll<HTMLElement>('[role="option"]')).find(
        (option) => option.textContent?.includes('25 results'),
    )
    expect(nextOption).toBeDefined()
    nextOption?.click()
    await waitFor(() => trigger.text().includes('25 results'), {
        label: 'number option selection to update the model',
    })
    expect(trigger.text()).toContain('25 results')
    expect(trigger.text()).not.toContain('[object Object]')

    wrapper.unmount()
    teleports.remove()
})

it('passes the formatted selected label through the selected slot', async () => {
    const teleports = installTeleportTarget()
    const wrapper = await mountThemed(
        Combobox,
        {
            modelValue: 'date_modified',
            options: [{ value: 'date_modified', label: 'Date updated' }],
        },
        'dark',
        {
            attachTo: document.body,
            slots: {
                selected: ({ label }: { label: string }) =>
                    h('span', { 'data-testid': 'selected-label' }, `Sorted by ${label}`),
            },
        },
    )

    expect(wrapper.get('[data-testid="selected-label"]').text()).toBe('Sorted by Date updated')

    wrapper.unmount()
    teleports.remove()
})

it('keeps disabled listboxes closed and does not emit a selection', async () => {
    const teleports = installTeleportTarget()
    const wrapper = await mountThemed(
        Combobox,
        {
            modelValue: 'relevance',
            options: [
                { value: 'relevance', label: 'Relevance' },
                { value: 'downloads', label: 'Downloads' },
            ],
            disabled: true,
        },
        'dark',
        { attachTo: document.body },
    )

    const trigger = wrapper.get('[aria-haspopup="listbox"]')
    expect(trigger.attributes('aria-disabled')).toBe('true')
    activate(trigger.element as HTMLElement)
    await wrapper.vm.$nextTick()
    expect(document.querySelector('[role="listbox"]')).toBeNull()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()

    wrapper.unmount()
    teleports.remove()
})

it('honors an explicit upward direction for floating dropdowns', async () => {
    const teleports = installTeleportTarget()
    const host = document.createElement('div')
    host.style.cssText = 'position:fixed; top:200px; left:16px; width:180px'
    document.body.append(host)
    const wrapper = await mountThemed(
        Combobox,
        {
            modelValue: 'relevance',
            options: [{ value: 'relevance', label: 'Relevance' }],
            forceDirection: 'up',
        },
        'dark',
        { attachTo: host },
    )

    const trigger = wrapper.get('[aria-haspopup="listbox"]')
    activate(trigger.element as HTMLElement)
    await waitFor(() => !!document.querySelector('[role="listbox"]'), {
        label: 'forced-up listbox to open',
    })

    const dropdown = document.querySelector('[role="listbox"]') as HTMLElement
    expect(dropdown.getBoundingClientRect().bottom).toBeLessThanOrEqual(
        (trigger.element as HTMLElement).getBoundingClientRect().top,
    )

    wrapper.unmount()
    host.remove()
    teleports.remove()
})

it('automatically floats upward when the trigger is near the viewport bottom', async () => {
    const teleports = installTeleportTarget()
    const host = document.createElement('div')
    host.style.position = 'fixed'
    host.style.left = '16px'
    host.style.bottom = '4px'
    host.style.width = '180px'
    document.body.append(host)
    const options: ComboboxOption<string>[] = Array.from({ length: 8 }, (_, index) => ({
        value: `option-${index}`,
        label: `Option ${index}`,
    }))
    const wrapper = await mountThemed(Combobox, { modelValue: 'option-0', options }, 'dark', {
        attachTo: host,
    })

    const trigger = wrapper.get('[aria-haspopup="listbox"]')
    activate(trigger.element as HTMLElement)
    await waitFor(
        () => {
            const dropdown = document.querySelector('[role="listbox"]') as HTMLElement | null
            return !!dropdown && dropdown.getBoundingClientRect().height > 0
        },
        { label: 'auto-positioned listbox to open' },
    )

    const dropdown = document.querySelector('[role="listbox"]') as HTMLElement
    expect(dropdown.getBoundingClientRect().bottom).toBeLessThanOrEqual(
        (trigger.element as HTMLElement).getBoundingClientRect().top,
    )

    wrapper.unmount()
    host.remove()
    teleports.remove()
})
