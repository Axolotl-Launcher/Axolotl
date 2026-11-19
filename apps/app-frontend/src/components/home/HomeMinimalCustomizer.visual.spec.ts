import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { type HomeMinimalOptions, setHomeMinimalPreset } from './home-minimal'
import HomeMinimalCustomizer from './HomeMinimalCustomizer.vue'

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Toggle: defineComponent({
            props: ['modelValue'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('button', {
                        role: 'switch',
                        'aria-checked': String(props.modelValue),
                        onClick: () => emit('update:modelValue', !props.modelValue),
                    }),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

function mountPanel(options: HomeMinimalOptions | null) {
    const wrapper = mount(HomeMinimalCustomizer, {
        attachTo: document.body,
        props: { options, canChangeInstance: true },
        global: { directives: { tooltip: () => {} } },
    })
    wrapper.element.setAttribute('style', 'position:static')
    return wrapper
}

it('picks a preset from drawn thumbnails, as a keyboard radio group', async () => {
    const wrapper = mountPanel(null)
    try {
        const group = wrapper.get('[data-group="preset"]')
        expect(group.attributes('role')).toBe('radiogroup')
        const presets = group.findAll('[role="radio"]')
        expect(presets.map((preset) => preset.attributes('data-preset'))).toEqual([
            'pure',
            'classic',
            'standard',
            'custom',
        ])
        // Every preset has a drawing and a one-word label, not a paragraph.
        for (const preset of presets) {
            expect(preset.find('.home-minimal-thumb').exists()).toBe(true)
            expect(preset.get('.home-minimal-customizer-preset-label').text().length).toBeLessThan(
                10,
            )
        }
        // Only the checked preset is a tab stop, and it carries the check mark.
        const standard = group.get('[data-preset="standard"]')
        expect(standard.attributes('aria-checked')).toBe('true')
        expect(standard.attributes('tabindex')).toBe('0')
        expect(standard.find('.home-minimal-customizer-check').exists()).toBe(true)
        expect(group.get('[data-preset="pure"]').attributes('tabindex')).toBe('-1')
        expect(getComputedStyle(standard.get('.home-minimal-thumb').element).boxShadow).not.toBe(
            'none',
        )

        // Arrow keys move the choice along the row.
        await standard.trigger('keydown', { key: 'ArrowRight' })
        await standard.trigger('keydown', { key: 'ArrowLeft' })
        const updates = wrapper.emitted('update') as [HomeMinimalOptions][]
        expect(updates.map(([options]) => options.preset)).toEqual(['custom', 'classic'])
    } finally {
        wrapper.unmount()
    }
})

it('shows the three arrangements as drawn options, the strip among them', async () => {
    const wrapper = mountPanel(setHomeMinimalPreset(null, 'classic'))
    try {
        const group = wrapper.get('[data-group="arrangement"]')
        const options = group.findAll('[role="radio"]')
        expect(options.map((option) => option.attributes('data-value'))).toEqual([
            'row',
            'stacked',
            'bar',
        ])
        for (const option of options) {
            expect(option.find('.home-minimal-glyph').exists()).toBe(true)
        }
        expect(group.get('[data-value="bar"]').attributes('aria-checked')).toBe('true')
        await group.get('[data-value="row"]').trigger('click')
        const [[changed]] = wrapper.emitted('update') as [HomeMinimalOptions][]
        expect(changed).toMatchObject({ preset: 'custom', arrangement: 'row' })
        // The panel stays inside its width.
        const panel = wrapper.element as HTMLElement
        expect(panel.scrollWidth).toBeLessThanOrEqual(panel.clientWidth)
    } finally {
        wrapper.unmount()
    }
})
