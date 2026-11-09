import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import JavaSettings from './JavaSettings.vue'

const fixture = vi.hoisted(() => ({
    edit: vi.fn(async () => {}),
    memory: null as null | { maximum: number; automatic: boolean },
}))

vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Checkbox: defineComponent({
            props: ['modelValue', 'label'],
            emits: ['update:modelValue'],
            setup: (props, { emit }) => {
                return () =>
                    h(
                        'button',
                        {
                            'data-label': props.label,
                            'data-checked': String(!!props.modelValue),
                            onClick: () => emit('update:modelValue', !props.modelValue),
                        },
                        props.label,
                    )
            },
        }),
        Slider: defineComponent({
            props: ['disabled'],
            setup: (props) => () => h('input', { disabled: props.disabled }),
        }),
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})
vi.mock('@tanstack/vue-query', async () => {
    const { ref } = await import('vue')
    return { useQuery: () => ({ data: ref(null), error: ref(null) }) }
})
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => 'linux' }))
vi.mock('@/components/ui/EnvironmentVariablesInput.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/ui/JavaArgumentsInput.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/JavaSelector.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/MemoryAllocationDisplay.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/composables/useMemorySlider', async () => {
    const { ref } = await import('vue')
    return { default: () => ({ maxMemory: ref(16384), snapPoints: ref([]) }) }
})
vi.mock('@/helpers/gc/context', () => ({
    collectGcContext: async () => null,
    extractJavaMajorVersion: () => 21,
}))
vi.mock('@/helpers/instance', () => ({
    edit: fixture.edit,
    get_content_snapshot: async () => ({ items: [] }),
    get_optimal_jre_key: async () => null,
}))
vi.mock('@/helpers/settings', () => ({
    get: async () => ({ memory: { maximum: 4096, automatic: true } }),
}))
vi.mock('@/providers/instance-settings', () => ({
    injectInstanceSettings: () => ({
        instance: ref({ id: 'instance-1', loader: 'fabric', memory: fixture.memory }),
    }),
}))

const CUSTOM = 'Custom memory allocation'
const AUTOMATIC = 'Automatically allocate memory at launch'

const cleanup: (() => void)[] = []
beforeEach(() => {
    fixture.edit.mockClear()
    fixture.memory = null
})
afterEach(() => cleanup.splice(0).forEach((stop) => stop()))

async function mountSettings() {
    const wrapper = mount(
        defineComponent({
            setup: () => () => h(Suspense, null, { default: () => h(JavaSettings) }),
        }),
    )
    cleanup.push(() => wrapper.unmount())
    await vi.waitFor(() => expect(wrapper.find('#max-memory').exists()).toBe(true))
    return wrapper
}

it('unlocks the slider as soon as custom memory allocation is turned on', async () => {
    const wrapper = await mountSettings()
    expect(wrapper.find('#max-memory').attributes('disabled')).toBeDefined()

    await wrapper.find(`[data-label="${CUSTOM}"]`).trigger('click')
    await flushPromises()

    expect(wrapper.find(`[data-label="${AUTOMATIC}"]`).attributes('data-checked')).toBe('false')
    expect(wrapper.find('#max-memory').attributes('disabled')).toBeUndefined()
    expect(fixture.edit).toHaveBeenLastCalledWith(
        'instance-1',
        expect.objectContaining({ memory: expect.objectContaining({ automatic: false }) }),
    )
})

it('keeps a saved automatic override when the settings are reopened', async () => {
    fixture.memory = { maximum: 4096, automatic: true }
    const wrapper = await mountSettings()

    expect(wrapper.find(`[data-label="${AUTOMATIC}"]`).attributes('data-checked')).toBe('true')
    expect(wrapper.find('#max-memory').attributes('disabled')).toBeDefined()
    expect(fixture.edit).not.toHaveBeenCalled()
})

it('lets automatic allocation be turned back on under a custom override', async () => {
    const wrapper = await mountSettings()
    await wrapper.find(`[data-label="${CUSTOM}"]`).trigger('click')
    await flushPromises()

    await wrapper.find(`[data-label="${AUTOMATIC}"]`).trigger('click')
    await flushPromises()

    expect(wrapper.find('#max-memory').attributes('disabled')).toBeDefined()
})
