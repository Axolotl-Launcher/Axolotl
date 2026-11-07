import { userEvent } from '@vitest/browser/context'
import type { mount, VueWrapper } from '@vue/test-utils'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { mountThemed, waitFor } from '../../test/visual-harness'
import ColorPicker from './ColorPicker.vue'

const mounted: ReturnType<typeof mount>[] = []

afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

beforeEach(() => {
    localStorage.clear()
})

function panel() {
    return document.querySelector('[role="dialog"]') as HTMLElement | null
}

function hueInput() {
    return panel()!.querySelector('input[type="range"]') as HTMLInputElement
}

function area() {
    return panel()!.querySelector('[role="slider"]') as HTMLElement
}

function presetGroups() {
    return [...panel()!.querySelectorAll('[role="group"]')] as HTMLElement[]
}

function hexInput() {
    return panel()!.querySelector('input[type="text"]') as HTMLInputElement
}

function channels(hex: string) {
    return [
        Number.parseInt(hex.slice(1, 3), 16),
        Number.parseInt(hex.slice(3, 5), 16),
        Number.parseInt(hex.slice(5, 7), 16),
    ]
}

/** Reads the alpha out of a computed colour, which omits it when it is 1. */
function alphaOf(color: string) {
    const match = /\/(\s*[\d.]+)\s*\)/.exec(color)
    return match ? Number(match[1]) : 1
}

type PickerWrapper = VueWrapper

async function openPicker(props: Record<string, unknown> = {}) {
    const wrapper = await mountThemed(ColorPicker, { modelValue: '#ff0000', ...props }, 'dark')
    mounted.push(wrapper)
    await openTrigger(wrapper)
    return wrapper
}

/** Reopens the same instance, which keeps the state the component holds in memory. */
async function openTrigger(wrapper: PickerWrapper) {
    await wrapper.get('button').trigger('click')
    await flushPromises()
    await waitFor(() => panel() !== null && getComputedStyle(panel()!).visibility === 'visible', {
        label: 'the picker panel',
    })
    // The entrance animation scales the panel, so measuring or clicking it
    // mid-flight would read a transformed box.
    await waitFor(() => getComputedStyle(panel()!).transform === 'none', {
        label: 'the panel entrance animation',
    })
}

async function closePicker() {
    area().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()
}

it('replaces the native colour dialog with a themed panel', async () => {
    await openPicker()

    expect(document.querySelector('input[type="color"]')).toBeNull()
    expect(panel()?.getAttribute('role')).toBe('dialog')
    expect(panel()?.classList.contains('bg-surface-4')).toBe(true)
})

it('applies a caller class to the trigger, not to a phantom root', async () => {
    const wrapper = await mountThemed(
        ColorPicker,
        { modelValue: '#ff0000', class: 'shrink-0' },
        'dark',
    )
    mounted.push(wrapper)

    expect(wrapper.get('span').classes()).toContain('shrink-0')
})

it('renders the sixteen dye presets in one group', async () => {
    await openPicker()

    const [dyes] = presetGroups()
    expect(dyes.querySelectorAll('button')).toHaveLength(16)
})

it('animates the panel in once it is positioned', async () => {
    await openPicker()

    const styles = getComputedStyle(panel()!)
    expect(styles.animationName).toContain('color-picker-panel-in')
    expect(styles.animationDuration).toBe('0.16s')
})

it('marks swatch hover with a border instead of lifting the swatch', async () => {
    await openPicker()

    const swatch = presetGroups()[0].querySelectorAll('button')[0] as HTMLElement
    await userEvent.unhover(swatch)
    const restingBorder = getComputedStyle(swatch).borderTopColor
    expect(getComputedStyle(swatch).transform).toBe('none')

    await userEvent.hover(swatch)
    await waitFor(() => getComputedStyle(swatch).borderTopColor !== restingBorder, {
        label: 'the hover border',
    })
    // Read again once the border transition has settled, otherwise the
    // intermediate colour would satisfy the comparison on its own.
    await new Promise((resolve) => setTimeout(resolve, 250))

    expect(getComputedStyle(swatch).transform).toBe('none')
    expect(alphaOf(getComputedStyle(swatch).borderTopColor)).toBeGreaterThan(alphaOf(restingBorder))
})

it('emits the dye hex when a preset is picked', async () => {
    const wrapper = await openPicker()

    presetGroups()[0].querySelectorAll('button')[2].click()
    await flushPromises()

    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['#c74ebd'])
    expect(wrapper.emitted('change')?.at(-1)).toEqual(['#c74ebd'])
})

it('keeps the hue when the value is dragged down to black', async () => {
    const wrapper = await openPicker({ modelValue: '#00ff00' })

    expect(Number(hueInput().value)).toBe(120)

    const box = area().getBoundingClientRect()
    await userEvent.click(area(), { position: { x: 4, y: box.height - 4 } })
    await flushPromises()

    const emitted = wrapper.emitted('update:modelValue')?.at(-1)?.[0] as string
    for (const channel of channels(emitted)) expect(channel).toBeLessThanOrEqual(12)
    expect(Number(hueInput().value)).toBe(120)
})

it('commits a typed hex value on blur', async () => {
    const wrapper = await openPicker()

    hexInput().value = '#3366cc'
    hexInput().dispatchEvent(new Event('input', { bubbles: true }))
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['#3366cc'])

    hexInput().dispatchEvent(new Event('blur'))
    await flushPromises()
    expect(wrapper.emitted('change')?.at(-1)).toEqual(['#3366cc'])
})

it('restores the last valid colour when the hex draft is incomplete', async () => {
    const wrapper = await openPicker()

    hexInput().value = '#12'
    hexInput().dispatchEvent(new Event('input', { bubbles: true }))
    hexInput().dispatchEvent(new Event('blur'))
    await flushPromises()

    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    expect(hexInput().value).toBe('#ff0000')
})

it('adjusts saturation and brightness with the arrow keys', async () => {
    const wrapper = await openPicker()

    area().dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }))
    await flushPromises()

    const emitted = wrapper.emitted('update:modelValue')?.at(-1)?.[0] as string
    expect(emitted).not.toBe('#ff0000')
    expect(channels(emitted)[0]).toBe(255)
    expect(Number(hueInput().value)).toBe(0)
})

it('closes on escape and returns focus to the trigger', async () => {
    const wrapper = await openPicker()

    await closePicker()

    expect(panel()).toBeNull()
    expect(document.activeElement).toBe(wrapper.get('button').element)
})

it('leaves the field that dismissed the picker with its focus', async () => {
    const wrapper = await openPicker()
    const outside = document.createElement('input')
    document.body.append(outside)

    try {
        outside.focus()
        expect(document.activeElement).toBe(outside)

        outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))
        await flushPromises()

        expect(panel()).toBeNull()
        // Not merely "focus is not on the trigger": a handler that blurred the
        // field and left focus on the body would pass that weaker check while
        // still losing the caret the user just placed.
        expect(document.activeElement).toBe(outside)
        expect(document.activeElement).not.toBe(wrapper.get('button').element)
    } finally {
        outside.remove()
    }
})

it('reports a colour once when the panel closes after the change', async () => {
    const wrapper = await openPicker()

    presetGroups()[0].querySelectorAll('button')[12].click()
    await flushPromises()
    await closePicker()

    expect(wrapper.emitted('change')).toEqual([['#835432']])
})

it('reports nothing when the panel is dismissed without a change', async () => {
    const wrapper = await openPicker()

    await closePicker()

    expect(wrapper.emitted('change')).toBeUndefined()
})

it('stops painting the colour once a drag is interrupted', async () => {
    const wrapper = await openPicker()
    const box = area().getBoundingClientRect()

    // Escape closes the panel before the pointer is released, so the drag never
    // ends through its own pointerup.
    area().dispatchEvent(
        new PointerEvent('pointerdown', {
            bubbles: true,
            pointerId: 1,
            clientX: box.left + box.width * 0.9,
            clientY: box.top + 8,
        }),
    )
    await flushPromises()
    area().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()
    const dragged = wrapper.emitted('update:modelValue')?.at(-1)?.[0]
    expect(dragged).not.toBe('#ff0000')

    await openTrigger(wrapper)
    area().dispatchEvent(
        new PointerEvent('pointermove', {
            bubbles: true,
            pointerId: 1,
            clientX: box.left + 4,
            clientY: box.top + 8,
        }),
    )
    await flushPromises()

    expect(wrapper.emitted('update:modelValue')?.at(-1)?.[0]).toBe(dragged)
})

it('lists a committed colour under recent colours on the next open', async () => {
    await openPicker()
    presetGroups()[0].querySelectorAll('button')[12].click()
    await flushPromises()
    await closePicker()

    await openPicker()

    const groups = presetGroups()
    expect(groups).toHaveLength(2)
    const recent = groups[1].querySelectorAll('button')
    expect(recent).toHaveLength(1)
    expect(recent[0].getAttribute('aria-label')).toBe('#835432')
})

it('keeps recent colours after storage rejects the write', async () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('QuotaExceededError')
    })

    const wrapper = await openPicker()
    presetGroups()[0].querySelectorAll('button')[12].click()
    await flushPromises()
    await closePicker()
    await openTrigger(wrapper)

    setItem.mockRestore()

    const groups = presetGroups()
    expect(groups).toHaveLength(2)
    expect(groups[1].querySelectorAll('button')[0].getAttribute('aria-label')).toBe('#835432')
})

it('keeps recent colours out of the panel when the caller opts out', async () => {
    await openPicker({ showRecentColors: false })
    presetGroups()[0].querySelectorAll('button')[12].click()
    await flushPromises()
    await closePicker()

    await openPicker({ showRecentColors: false })

    expect(presetGroups()).toHaveLength(1)
    expect(localStorage.getItem('axolotl:color-picker:recent')).toBeNull()
})
