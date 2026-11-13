import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, ref } from 'vue'

import ArmorPreviewControls from '../components/skin/ArmorPreviewControls.vue'
import ArmorTrimPanel from '../components/skin/ArmorTrimPanel.vue'
import {
    type ArmorPreviewConfig,
    createDefaultArmorPreviewConfig,
} from '../composables/skin-rendering/armor-preview-types'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn())
    vi.useRealTimers()
})

async function panel(ignorePhysicalPointer = false) {
    const config = ref(createDefaultArmorPreviewConfig())
    config.value.helmet = { material: 'diamond', trimPattern: 'bolt', trimMaterial: 'gold' }
    const Harness = defineComponent({
        setup: () => () =>
            h(
                'div',
                {
                    style: `width:360px;position:fixed;left:16px;top:16px;${ignorePhysicalPointer ? 'pointer-events:none' : ''}`,
                },
                [
                    h(ArmorTrimPanel, {
                        modelValue: config.value,
                        'onUpdate:modelValue': (value: ArmorPreviewConfig) => {
                            config.value = value
                        },
                    }),
                ],
            ),
    })
    const wrapper = await mountThemed(Harness, {}, 'dark')
    cleanup.push(() => wrapper.unmount())
    return { wrapper, config }
}

it('uses the available region width to wrap fixed-size options without stretching them', async () => {
    const { wrapper } = await panel()
    const section = wrapper.get('[data-tooltip-group="armor-trim-patterns"]').element as HTMLElement
    const buttons = Array.from(section.querySelectorAll('button'))
    const countFirstRow = () =>
        buttons.filter((button) => button.offsetTop === buttons[0].offsetTop).length
    const wideCount = countFirstRow()
    expect(wideCount).toBeGreaterThan(6)
    expect(section.getBoundingClientRect().width).toBe(360)
    expect(buttons[0].getBoundingClientRect().width).toBe(40)
    expect(buttons[0].getBoundingClientRect().height).toBe(40)
    ;(wrapper.element as HTMLElement).style.width = '200px'
    expect(countFirstRow()).toBeLessThan(wideCount)
    expect(section.scrollWidth).toBe(200)
    expect(buttons[0].getBoundingClientRect().width).toBe(40)
})

it('keeps the popover compact while its parent owns scrolling and the panel fits inside it', async () => {
    const wrapper = await mountThemed(
        ArmorPreviewControls,
        { modelValue: createDefaultArmorPreviewConfig() },
        'dark',
    )
    cleanup.push(() => wrapper.unmount())
    await wrapper.get('button').trigger('click')
    await waitFor(() => !!document.querySelector('.armor-preview-popover'))
    const popover = document.querySelector('.armor-preview-popover') as HTMLElement
    const options = popover.querySelector('[data-tooltip-group="armor-materials"]') as HTMLElement
    const region = options.parentElement!.parentElement!
    const scrollOwner = region.parentElement!
    expect(getComputedStyle(region).overflowY).toBe('visible')
    expect(getComputedStyle(scrollOwner).overflowY).toBe('auto')
    expect(region.getBoundingClientRect().width).toBeLessThanOrEqual(304)
    expect(options.scrollWidth).toBe(options.clientWidth)
    expect(popover.getBoundingClientRect().width).toBeLessThanOrEqual(innerWidth - 16)
    await wrapper.get('button').trigger('click')
    await waitFor(() => !document.querySelector('.armor-preview-popover'))
})

it('reuses the tooltip immediately between peer options and keeps their selections intact', async () => {
    // Synthetic events own this test's hover state. Native pointer events from
    // an earlier browser action must not activate a different option.
    const { wrapper, config } = await panel(true)
    await page
        .elementLocator(wrapper.get('button[aria-label="Netherite Helmet"]').element)
        .hover({ force: true })
    expect(wrapper.get('button[aria-label="Netherite Helmet"]').element.matches(':hover')).toBe(
        false,
    )
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    for (const section of wrapper.findAll('[data-tooltip-group]')) {
        const [first, second] = section
            .findAll('button')
            .map((button) => button.element as HTMLButtonElement)
        first.dispatchEvent(new MouseEvent('mouseenter'))
        await vi.advanceTimersByTimeAsync(199)
        expect(document.querySelector('[role="tooltip"]')).toBeNull()
        await vi.advanceTimersByTimeAsync(1)
        const tooltip = document.querySelector('[role="tooltip"]')!
        expect(tooltip).not.toBeNull()
        first.dispatchEvent(new MouseEvent('mouseleave'))
        second.dispatchEvent(new MouseEvent('mouseenter'))
        expect(document.querySelector('[role="tooltip"]')).toBe(tooltip)
        expect(tooltip.textContent).toBe(second.getAttribute('aria-label'))
        expect(second.getAttribute('aria-describedby')).toBe(tooltip.id)
        expect(first.hasAttribute('aria-describedby')).toBe(false)
        second.dispatchEvent(new MouseEvent('mouseleave'))
        await vi.advanceTimersByTimeAsync(1000)
        expect(document.querySelector('[role="tooltip"]')).toBeNull()
        expect(second.hasAttribute('aria-describedby')).toBe(false)
    }
    await wrapper.get('button[aria-label="Diamond Helmet"]').trigger('click')
    await wrapper.get('button[aria-label="Coast Armor Trim"]').trigger('click')
    expect(config.value.helmet.material).toBe('diamond')
    expect(config.value.helmet.trimPattern).toBe('coast')
    wrapper.get<HTMLButtonElement>('button[aria-label="Coast Armor Trim"]').element.focus()
    expect(document.querySelector('[role="tooltip"]')).not.toBeNull()
    wrapper.unmount()
    expect(document.querySelector('[role="tooltip"]')).toBeNull()
})
