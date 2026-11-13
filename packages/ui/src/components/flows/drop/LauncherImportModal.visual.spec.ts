import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import LauncherImportModal from './LauncherImportModal.vue'

vi.mock('#ui/components/modal/NewModal.vue', () => ({
    default: defineComponent({
        setup: (_, { slots, expose }) => {
            expose({ show: vi.fn(), hide: vi.fn() })
            return () => h('section', [slots.default?.(), slots.actions?.()])
        },
    }),
}))
vi.mock('#ui/composables/i18n', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('selects one concrete shared-root version when names repeat across game directories', async () => {
    const shared = 'D:/Games/.minecraft'
    const instances = [
        {
            name: '1.20.1',
            path: shared,
            versionPath: `${shared}/versions/1.20.1`,
            compatibleMode: true,
            version: '1.20.1',
            loader: '',
        },
        {
            name: '1.21.1',
            path: shared,
            versionPath: `${shared}/versions/1.21.1`,
            compatibleMode: true,
            version: '1.21.1',
            loader: '',
        },
        {
            name: '1.20.1',
            path: 'E:/Other/.minecraft',
            versionPath: 'E:/Other/.minecraft/versions/1.20.1',
            compatibleMode: true,
            version: '1.20.1',
            loader: '',
        },
    ]
    const wrapper = mount(LauncherImportModal, {
        global: {
            directives: { tooltip: {} },
            stubs: {
                InstanceRowCard: defineComponent({
                    props: ['name'],
                    emits: ['select'],
                    setup:
                        (props, { slots, emit }) =>
                        () =>
                            h('div', { class: 'instance-row', onClick: () => emit('select') }, [
                                props.name,
                                slots.prepend?.(),
                            ]),
                }),
                Checkbox: defineComponent({
                    props: ['modelValue'],
                    setup: (props) => () =>
                        h('input', { type: 'checkbox', checked: props.modelValue }),
                }),
            },
        },
    })
    try {
        wrapper.vm.show([{ launcherName: 'PCL CE', launcherType: 'PCL2CE', instances }])
        await flushPromises()
        await wrapper.findAll('.instance-row')[0].trigger('click')
        expect(wrapper.findAll('input').map((input) => input.element.checked)).toEqual([
            true,
            false,
            false,
        ])
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Import ({n})')!
            .trigger('click')
        await vi.waitFor(() => expect(wrapper.emitted('confirm')).toHaveLength(1))
        expect(wrapper.emitted('confirm')![0]).toEqual([
            [
                {
                    launcherName: 'PCL CE',
                    launcherType: 'PCL2CE',
                    instances: [
                        {
                            name: instances[0].name,
                            path: instances[0].path,
                            versionPath: instances[0].versionPath,
                            compatibleMode: true,
                        },
                    ],
                },
            ],
        ])
    } finally {
        wrapper.unmount()
    }
})
