import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import JavaSelector from './JavaSelector.vue'

const selection = vi.hoisted(() => ({ confirm: vi.fn(), error: vi.fn() }))
vi.mock('@/helpers/java-selection', () => ({ confirmJavaSelection: selection.confirm }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/components/ui/JavaDetectionModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@/composables/useJavaTest', () => ({
    default: () => ({
        testingJava: ref(false),
        javaTestResult: ref(null),
        testJavaInstallation: vi.fn(),
        testJavaInstallationDebounced: vi.fn(),
    }),
}))
vi.mock('@modrinth/ui', () => ({
    Button: { render: () => null },
    IconButton: { render: () => null },
    StyledInput: defineComponent({
        props: ['modelValue', 'disabled'],
        emits: ['update:modelValue'],
        setup:
            (props, { attrs, emit }) =>
            () =>
                h('input', {
                    ...attrs,
                    value: props.modelValue,
                    disabled: props.disabled,
                    onInput: (event: Event) =>
                        emit('update:modelValue', (event.target as HTMLInputElement).value),
                }),
    }),
    commonMessages: {},
    defineMessages: (value: unknown) => value,
    injectNotificationManager: () => ({ handleError: selection.error }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => vi.resetAllMocks())
afterEach(() => wrappers.splice(0).forEach((wrapper) => wrapper.unmount()))
function mounted() {
    const wrapper = mount(JavaSelector, {
        props: { modelValue: { path: 'C:/local/javaw.exe' } },
        global: { directives: { tooltip: {} } },
    })
    wrappers.push(wrapper)
    return wrapper
}

it('keeps an edited path uncommitted until the external selection is approved', async () => {
    let finish!: (runtime: { path: string }) => void
    selection.confirm.mockReturnValue(new Promise((resolve) => (finish = resolve)))
    const wrapper = mounted()
    await wrapper.get('input').setValue('E:/portable/javaw.exe')
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    await wrapper.get('input').trigger('focusout')
    expect(wrapper.emitted('commit')).toBeUndefined()
    expect(wrapper.get('input').attributes('disabled')).toBeDefined()
    finish({ path: 'E:/portable/javaw.exe' })
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')).toEqual([[{ path: 'E:/portable/javaw.exe' }]])
    expect(wrapper.emitted('commit')).toEqual([[{ path: 'E:/portable/javaw.exe' }]])
})

it('restores the previous path without persisting a declined choice', async () => {
    selection.confirm.mockResolvedValue(null)
    const wrapper = mounted()
    await wrapper.get('input').setValue('E:/portable/javaw.exe')
    await wrapper.get('input').trigger('focusout')
    await flushPromises()
    expect((wrapper.get('input').element as HTMLInputElement).value).toBe('C:/local/javaw.exe')
    expect(wrapper.emitted('commit')).toBeUndefined()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
})

it('discards a pending choice when the required Java version changes', async () => {
    let finish!: (runtime: { path: string }) => void
    selection.confirm.mockReturnValue(new Promise((resolve) => (finish = resolve)))
    const wrapper = mounted()
    await wrapper.get('input').setValue('E:/portable/javaw.exe')
    await wrapper.get('input').trigger('focusout')
    await wrapper.setProps({ version: 21 })
    finish({ path: 'E:/portable/javaw.exe' })
    await flushPromises()
    expect(wrapper.emitted('commit')).toBeUndefined()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
})

it('preserves the previous selection when storage changes before approval is saved', async () => {
    selection.confirm.mockRejectedValue(new Error('volume changed'))
    const wrapper = mounted()
    await wrapper.get('input').setValue('E:/portable/javaw.exe')
    await wrapper.get('input').trigger('focusout')
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    expect(selection.error).toHaveBeenCalled()
})
