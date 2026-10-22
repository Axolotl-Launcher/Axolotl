import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import StyledInput from '../components/base/StyledInput.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

for (const multiline of [false, true]) {
    it(`routes field attributes and listeners to ${multiline ? 'textarea' : 'input'} while preserving wrapper layout`, async () => {
        const focus = vi.fn()
        const blur = vi.fn()
        const input = vi.fn()
        const wrapper = await mountThemed(
            StyledInput,
            {
                modelValue: '',
                multiline,
                id: 'field',
                class: 'flex-1',
                style: { width: '180px' },
                wrapperAttrs: { 'data-region': 'search' },
                'aria-label': 'Search servers',
                required: true,
                minlength: 3,
                inputAttrs: { 'aria-describedby': 'help' },
                onFocus: focus,
                onBlur: blur,
                onInput: input,
            },
            'dark',
        )
        cleanup.push(() => wrapper.unmount())
        const field = wrapper.get<HTMLInputElement | HTMLTextAreaElement>(
            multiline ? 'textarea' : 'input',
        )
        expect(wrapper.attributes('aria-label')).toBeUndefined()
        expect(wrapper.attributes('data-region')).toBe('search')
        expect(wrapper.classes()).toContain('flex-1')
        expect(wrapper.element.getBoundingClientRect().width).toBe(180)
        expect(field.attributes('id')).toBe('field')
        expect(field.attributes('aria-label')).toBe('Search servers')
        expect(field.attributes('aria-describedby')).toBe('help')
        expect(field.element.required).toBe(true)
        expect(field.element.minLength).toBe(3)
        field.element.focus()
        await field.setValue('Updated')
        field.element.blur()
        expect(focus).toHaveBeenCalledTimes(1)
        expect(blur).toHaveBeenCalledTimes(1)
        expect(input).toHaveBeenCalledTimes(1)
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['Updated'])
        await wrapper.setProps({
            'aria-label': 'Changed',
            inputAttrs: { 'aria-describedby': 'new-help' },
        })
        expect(field.attributes('aria-label')).toBe('Changed')
        expect(field.attributes('aria-describedby')).toBe('new-help')
    })
}

for (const variant of ['filled', 'outlined'] as const) {
    it(`uses the public clear label and clears once in ${variant} inputs`, async () => {
        const inputClick = vi.fn()
        const wrapper = await mountThemed(
            StyledInput,
            {
                variant,
                modelValue: 'Find me',
                clearable: true,
                clearLabel: 'Clear server search',
                onClick: inputClick,
            },
            'dark',
        )
        cleanup.push(() => wrapper.unmount())
        await wrapper.get('button[aria-label="Clear server search"]').trigger('click')
        expect(wrapper.emitted('clear')).toHaveLength(1)
        expect(wrapper.emitted('update:modelValue')).toEqual([['']])
        expect(inputClick).not.toHaveBeenCalled()
        await wrapper.setProps({ modelValue: 'Locked', readonly: true })
        if (variant === 'filled') expect(wrapper.find('button').exists()).toBe(false)
        else {
            expect(wrapper.get<HTMLButtonElement>('button').element.disabled).toBe(true)
            wrapper.get<HTMLButtonElement>('button').element.click()
            expect(wrapper.emitted('clear')).toHaveLength(1)
        }
    })
}

it('localizes the default clear label and follows the supported sizes', async () => {
    const locale = ref('zh-CN')
    const wrapper = await mountThemed(
        StyledInput,
        { modelValue: 'Search', clearable: true },
        'dark',
        {
            global: {
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale,
                        t: (key: string) => (key === 'button.clear' ? '清除' : key),
                        setLocale: () => {},
                    },
                },
            },
        },
    )
    cleanup.push(() => wrapper.unmount())
    expect(wrapper.get('button').attributes('aria-label')).toBe('清除')
    expect(wrapper.get('input').element.getBoundingClientRect().height).toBe(36)
    await wrapper.setProps({ size: 'small' })
    expect(wrapper.get('input').element.getBoundingClientRect().height).toBe(32)
})

it('applies numeric constraints and exposes select on the actual field', async () => {
    const wrapper = await mountThemed(
        StyledInput,
        { type: 'number', min: 0, max: 10, step: 0.05 },
        'dark',
    )
    cleanup.push(() => wrapper.unmount())
    const field = wrapper.get<HTMLInputElement>('input')
    expect(field.element.type).toBe('number')
    expect(field.element.min).toBe('0')
    expect(field.element.step).toBe('0.05')
    await field.setValue('1.25')
    expect(wrapper.emitted('update:modelValue')).toEqual([[1.25]])
    await field.setValue('')
    expect(wrapper.emitted('update:modelValue')?.[1]).toEqual([undefined])
    await wrapper.setProps({ type: 'text', modelValue: 'Select me' })
    wrapper.vm.focus()
    wrapper.vm.select()
    expect(document.activeElement).toBe(field.element)
    expect(field.element.selectionStart).toBe(0)
    expect(field.element.selectionEnd).toBe(9)
})
