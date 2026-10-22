import type { mount } from '@vue/test-utils'
import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { mountThemed } from '../../test/visual-harness'
import Slider from './Slider.vue'

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

it.each([
    { min: -64, max: 320, initial: -30, input: '0', expected: 0 },
    { min: -64, max: 320, initial: 30, input: '-12', expected: -12 },
    { min: -64, max: 320, initial: 30, input: '', expected: -64 },
    { min: -64, max: 320, initial: -64, input: '', expected: -64 },
    { min: -64, max: 320, initial: 30, input: '-100', expected: -64 },
    { min: -64, max: 320, initial: 30, input: '400', expected: 320 },
    { min: 0, max: 100, initial: 50, input: '0', expected: 0 },
    { min: 512, max: 8192, initial: 2048, input: '0', expected: 512 },
])(
    'commits $input as $expected in [$min, $max]',
    async ({ min, max, initial, input, expected }) => {
        const wrapper = await mountThemed(
            Slider,
            { modelValue: initial, min, max, step: 1 },
            'dark',
        )
        mounted.push(wrapper)
        await wrapper.get('input[type="number"]').setValue(input)
        await flushPromises()
        expect(wrapper.emitted('update:modelValue')).toEqual([[expected]])
        expect(wrapper.get<HTMLInputElement>('input[type="number"]').element.value).toBe(
            String(expected),
        )
        expect(wrapper.get<HTMLInputElement>('input[type="range"]').element.valueAsNumber).toBe(
            expected,
        )
    },
)

it('retains the current value when the number input contains an incomplete number', async () => {
    const wrapper = await mountThemed(
        Slider,
        { modelValue: 12, min: -64, max: 320, step: 1 },
        'dark',
    )
    mounted.push(wrapper)
    const input = wrapper.get<HTMLInputElement>('input[type="number"]')
    input.element.value = ''
    Object.defineProperty(input.element, 'validity', {
        configurable: true,
        value: { badInput: true },
    })
    await input.trigger('input')
    await input.trigger('change')
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    expect(input.element.value).toBe('12')
})

it.each([
    { forceStep: true, step: 64, input: '2100', expected: 2048 },
    { forceStep: true, step: 1, input: '-12.5', expected: -12 },
    { forceStep: false, step: 1, input: '-12.5', expected: -12.5 },
    { forceStep: true, step: 0.5, input: '1.5', expected: 1.5 },
    { forceStep: true, step: 0.1, input: '0.3', expected: 0.3 },
    { forceStep: true, step: 0.1, input: '-0.3', expected: -0.3 },
    { forceStep: true, step: 0.1, input: '0.35', expected: 0.3 },
    { forceStep: true, step: 0.1, input: '-0.35', expected: -0.3 },
])(
    'respects step rules without discarding valid numeric precision: %j',
    async ({ forceStep, step, input, expected }) => {
        const wrapper = await mountThemed(
            Slider,
            { modelValue: 10, min: -64, max: 8192, step, forceStep },
            'dark',
        )
        mounted.push(wrapper)
        await wrapper.get('input[type="number"]').setValue(input)
        expect(wrapper.emitted('update:modelValue')).toEqual([[expected]])
    },
)

it('snaps range input once and retains zero when crossing the origin', async () => {
    const wrapper = await mountThemed(
        Slider,
        { modelValue: -40, min: -64, max: 320, step: 1, snapPoints: [0, 64], snapRange: 5 },
        'dark',
    )
    mounted.push(wrapper)
    await wrapper.get('input[type="range"]').setValue('2')
    expect(wrapper.emitted('update:modelValue')).toEqual([[0]])
    expect(wrapper.get<HTMLInputElement>('input[type="number"]').element.value).toBe('0')
})

it('persists zero through a controlled model and restores it after reopening', async () => {
    const value = ref(-30)
    const component = defineComponent({
        setup: () => () =>
            h(Slider, {
                modelValue: value.value,
                min: -64,
                max: 320,
                step: 1,
                'onUpdate:modelValue': (next: number) => (value.value = next),
            }),
    })
    const wrapper = await mountThemed(component, {}, 'dark')
    mounted.push(wrapper)
    await wrapper.get('input[type="number"]').setValue('0')
    expect(value.value).toBe(0)
    wrapper.unmount()
    const reopened = await mountThemed(component, {}, 'dark')
    mounted.push(reopened)
    expect(reopened.get<HTMLInputElement>('input[type="number"]').element.value).toBe('0')
    expect(reopened.get<HTMLInputElement>('input[type="range"]').element.valueAsNumber).toBe(0)
})
