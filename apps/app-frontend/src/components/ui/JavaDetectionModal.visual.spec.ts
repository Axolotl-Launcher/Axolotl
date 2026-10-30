import { config, flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { java_discovery_listener } from '@/helpers/events'
import { find_filtered_jres } from '@/helpers/jre.js'

import JavaDetectionModal from './JavaDetectionModal.vue'

config.global.directives.tooltip = {}

const handleError = vi.fn()
vi.mock('@/helpers/jre.js', () => ({ find_filtered_jres: vi.fn() }))
vi.mock('@/helpers/events', () => ({ java_discovery_listener: vi.fn(async () => () => {}) }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@modrinth/assets', () => ({
    CheckIcon: { render: () => null },
    PlusIcon: { render: () => null },
    SpinnerIcon: { render: () => null },
    XIcon: { render: () => null },
}))
vi.mock('@modrinth/ui', () => ({
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    NewModal: defineComponent({
        setup(_, { slots, expose }) {
            const visible = ref(false)
            expose({ show: () => (visible.value = true), hide: () => (visible.value = false) })
            return () => (visible.value ? h('section', slots.default?.()) : null)
        },
    }),
    Table: defineComponent({
        props: ['data'],
        setup:
            (props, { slots }) =>
            () =>
                h(
                    'div',
                    props.data.length
                        ? props.data.map((row: { path: string }) => h('p', row.path))
                        : slots['empty-state']?.(),
                ),
    }),
    commonMessages: {
        selectedLabel: { defaultMessage: 'Selected' },
        cancelButton: { defaultMessage: 'Cancel' },
    },
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

it('opens at once while Java installations are still being found', async () => {
    const lookup = deferred<{ path: string; version: string }[]>()
    vi.mocked(find_filtered_jres).mockReturnValueOnce(lookup.promise)
    const wrapper = mount(JavaDetectionModal)
    try {
        void wrapper.vm.show(17, null)
        await flushPromises()
        expect(wrapper.find('section').exists()).toBe(true)
        expect(wrapper.text()).toContain('Looking for Java installations')
        lookup.resolve([{ path: 'C:/java17/bin/javaw.exe', version: '17' }])
        await flushPromises()
        expect(wrapper.text()).toContain('C:/java17/bin/javaw.exe')
        expect(wrapper.text()).not.toContain('Looking for Java installations')
    } finally {
        wrapper.unmount()
    }
})

it('does not let a slower lookup for an earlier version replace the current one', async () => {
    const older = deferred<{ path: string; version: string }[]>()
    vi.mocked(find_filtered_jres).mockImplementation((version) =>
        version === 8
            ? older.promise
            : Promise.resolve([{ path: 'C:/java21/bin/javaw.exe', version: '21' }]),
    )
    const wrapper = mount(JavaDetectionModal)
    try {
        void wrapper.vm.show(8, null)
        await flushPromises()
        expect(find_filtered_jres).toHaveBeenLastCalledWith(8, false, false)
        void wrapper.vm.show(21, null)
        await flushPromises()
        older.resolve([{ path: 'C:/java8/bin/javaw.exe', version: '8' }])
        await flushPromises()
        expect(wrapper.text()).toContain('C:/java21/bin/javaw.exe')
        expect(wrapper.text()).not.toContain('C:/java8/bin/javaw.exe')
    } finally {
        wrapper.unmount()
    }
})

it('lets a discovery event wait for the lookup the user started instead of replacing it', async () => {
    let discovered!: () => void
    vi.mocked(java_discovery_listener).mockImplementationOnce(async (listener) => {
        discovered = listener as () => void
        return () => {}
    })
    const lookup = deferred<{ path: string; version: string }[]>()
    vi.mocked(find_filtered_jres)
        .mockReset()
        .mockReturnValueOnce(lookup.promise.then(() => Promise.reject(new Error('scan failed'))))
        .mockResolvedValueOnce([{ path: 'C:/java17/bin/javaw.exe', version: '17' }])
    handleError.mockClear()
    const wrapper = mount(JavaDetectionModal)
    try {
        void wrapper.vm.show(17, null)
        await flushPromises()
        discovered()
        await flushPromises()
        expect(find_filtered_jres).toHaveBeenCalledOnce()
        lookup.resolve([])
        await flushPromises()
        expect(handleError).toHaveBeenCalledOnce()
        expect(find_filtered_jres).toHaveBeenCalledTimes(2)
        expect(wrapper.text()).toContain('C:/java17/bin/javaw.exe')
    } finally {
        wrapper.unmount()
    }
})
