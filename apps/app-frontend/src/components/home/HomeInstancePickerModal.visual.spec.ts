import { CheckIcon } from '@modrinth/assets'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import HomeInstancePickerModal from './HomeInstancePickerModal.vue'
import { useHomeInstanceSelection } from './use-home-instance-selection'

const backend = vi.hoisted(() => ({
    theme: { minimalHomeInstanceId: null as string | null },
    read: vi.fn(),
    update: vi.fn(),
    hide: vi.fn(),
    handleError: vi.fn(),
}))

vi.mock('@/helpers/settings', () => ({
    appSettingsKeys: { all: ['app-settings'], update: ['app-settings', 'update'] },
    get: backend.read,
    update: backend.update,
}))
vi.mock('@/store/state', async () => {
    const { reactive } = await import('vue')
    const theme = reactive(backend.theme)
    return { useTheming: () => theme }
})
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        commonMessages: { savingButton: { defaultMessage: 'Saving…' } },
        defineMessages: (messages: unknown) => messages,
        EmptyState: defineComponent({ render: () => null }),
        StyledInput: defineComponent({ render: () => h('input') }),
        NewModal: defineComponent({
            props: { disableClose: Boolean },
            setup: (props, { expose, slots }) => {
                expose({ show: () => {}, hide: backend.hide })
                return () => h('section', { 'data-busy': props.disableClose }, slots.default?.())
            },
        }),
        useVIntl: () => ({
            locale: { value: 'en-US' },
            formatMessage: (message: { defaultMessage: string }, values?: Record<string, string>) =>
                new IntlMessageFormat(message.defaultMessage, 'en-US').format(values) as string,
        }),
    }
})

const first = { id: 'one', name: 'One', loader: 'vanilla', game_version: '1.21' } as GameInstance
const second = { ...first, id: 'two', name: 'Two' }
const third = { ...first, id: 'three', name: 'Three' }

beforeEach(() => {
    backend.theme.minimalHomeInstanceId = first.id
    backend.read.mockReset()
    backend.update.mockReset()
    backend.hide.mockReset()
    backend.handleError.mockReset()
})

function mountPicker() {
    const instances = ref([first, second, third])
    const queryClient = new QueryClient()
    let selection!: ReturnType<typeof useHomeInstanceSelection>
    const wrapper = mount(
        defineComponent({
            setup() {
                selection = useHomeInstanceSelection(instances, backend.handleError)
                return () =>
                    h(HomeInstancePickerModal, {
                        instances: instances.value,
                        selectedInstanceId: selection.selectedInstanceId.value,
                        selectionPending: selection.selecting.value,
                        saveSelection: selection.select,
                    })
            },
        }),
        { global: { plugins: [[VueQueryPlugin, { queryClient }]] } },
    )
    return {
        wrapper,
        selection,
        instances,
        cleanup: () => {
            wrapper.unmount()
            queryClient.clear()
        },
    }
}

it('holds selection controls while saving and only closes after applying the confirmed instance', async () => {
    const saving = Promise.withResolvers<{ minimal_home_instance_id: string }>()
    backend.update.mockReturnValueOnce(saving.promise)
    const { wrapper, selection, cleanup } = mountPicker()
    try {
        await wrapper.get('button[aria-label="Choose Two"]').trigger('click')
        await flushPromises()
        expect(backend.update).toHaveBeenCalledExactlyOnceWith({ minimal_home_instance_id: 'two' })
        expect(wrapper.get('fieldset').attributes()).toHaveProperty('disabled')
        expect(wrapper.get('section').attributes('data-busy')).toBe('true')
        expect(selection.selectedInstanceId.value).toBe('one')
        expect(backend.hide).not.toHaveBeenCalled()

        saving.resolve({ minimal_home_instance_id: 'two' })
        await flushPromises()
        expect(selection.selectedInstanceId.value).toBe('two')
        expect(
            wrapper.get('button[aria-label="Choose Two"]').findComponent(CheckIcon).exists(),
        ).toBe(true)
        expect(wrapper.get('section').attributes('data-busy')).toBe('false')
        expect(backend.hide).toHaveBeenCalledOnce()
    } finally {
        cleanup()
    }
})

it('keeps the picker open and restores the confirmed selection when saving fails', async () => {
    const failure = new Error('could not save')
    backend.update.mockRejectedValueOnce(failure)
    backend.read.mockResolvedValueOnce({ minimal_home_instance_id: 'one' })
    const { wrapper, selection, cleanup } = mountPicker()
    try {
        await wrapper.get('button[aria-label="Choose Two"]').trigger('click')
        await flushPromises()
        expect(backend.handleError).toHaveBeenCalledWith(failure)
        expect(selection.selectedInstanceId.value).toBe('one')
        expect(
            wrapper.get('button[aria-label="Choose One"]').findComponent(CheckIcon).exists(),
        ).toBe(true)
        expect(wrapper.get('fieldset').attributes()).not.toHaveProperty('disabled')
        expect(backend.hide).not.toHaveBeenCalled()
    } finally {
        cleanup()
    }
})

it('serializes writes and does not let cleanup for a vanished prior choice overwrite a new selection', async () => {
    const saving = Promise.withResolvers<{ minimal_home_instance_id: string }>()
    backend.update
        .mockReturnValueOnce(saving.promise)
        .mockResolvedValueOnce({ minimal_home_instance_id: 'three' })
    const { instances, selection, cleanup } = mountPicker()
    try {
        const chooseSecond = selection.select(second)
        const chooseThird = selection.select(third)
        instances.value = [second, third]
        await flushPromises()
        expect(backend.update).toHaveBeenCalledExactlyOnceWith({ minimal_home_instance_id: 'two' })

        saving.resolve({ minimal_home_instance_id: 'two' })
        await Promise.all([chooseSecond, chooseThird])
        await flushPromises()
        expect(backend.update.mock.calls).toEqual([
            [{ minimal_home_instance_id: 'two' }],
            [{ minimal_home_instance_id: 'three' }],
        ])
        expect(selection.selectedInstanceId.value).toBe('three')
    } finally {
        cleanup()
    }
})
