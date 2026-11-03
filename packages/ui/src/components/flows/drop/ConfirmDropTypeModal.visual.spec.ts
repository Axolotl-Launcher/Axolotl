import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import BigOptionButton from '#ui/components/base/BigOptionButton.vue'
import type { ClassificationResult } from '#ui/composables/use-global-drop'

import ConfirmDropTypeModal from './ConfirmDropTypeModal.vue'

vi.mock('#ui/components/modal/NewModal.vue', () => ({
    default: defineComponent({
        props: ['onAfterHide'],
        setup: (props, { slots, expose }) => {
            expose({ show: vi.fn(), hide: () => props.onAfterHide?.() })
            return () => h('div', [slots.title?.(), slots.default?.(), slots.actions?.()])
        },
    }),
}))
vi.mock('#ui/composables/i18n', () => ({
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it.each(['C:\\Games\\.minecraft', 'C:\\Games\\.MINECRAFT\\', '/games/.minecraft/'])(
    'asks to add the Minecraft root immediately without type choices: %s',
    async (path) => {
        const wrapper = mount(ConfirmDropTypeModal, {
            props: {
                fileName: '.minecraft',
                classification: {
                    item_type: 'launcher',
                    launcher_type: 'Generic',
                    base_path: path,
                },
            },
        })
        try {
            expect(wrapper.findAllComponents(BigOptionButton)).toHaveLength(0)
            expect(wrapper.text()).toContain(
                'This looks like a .minecraft folder. Would you like to add it directly to the launcher?',
            )
            expect(wrapper.text()).toContain(path)
            const yes = wrapper
                .findAll('button')
                .find((button) => button.text() === 'Yes, add folder')!
            await yes.trigger('click')
            expect(wrapper.emitted('confirm')).toEqual([['dot_minecraft']])
        } finally {
            wrapper.unmount()
        }
    },
)

it('continues with instance scanning when the folder-add prompt is declined', async () => {
    const wrapper = mount(ConfirmDropTypeModal, {
        props: {
            fileName: '.minecraft',
            classification: {
                item_type: 'launcher',
                launcher_type: 'Generic',
                base_path: 'D:\\Minecraft\\.minecraft',
            },
        },
    })
    try {
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'No, import instances')!
            .trigger('click')
        expect(wrapper.emitted('confirm')).toEqual([['instance']])
        expect(wrapper.emitted('cancel')).toBeUndefined()
    } finally {
        wrapper.unmount()
    }
})

it('cancels the root prompt when it is closed without a choice', async () => {
    const wrapper = mount(ConfirmDropTypeModal, {
        props: {
            fileName: '.minecraft',
            classification: {
                item_type: 'launcher',
                launcher_type: 'Generic',
                base_path: 'D:\\Minecraft\\.minecraft',
            },
        },
    })
    try {
        await wrapper.vm.hide()
        expect(wrapper.emitted('confirm')).toBeUndefined()
        expect(wrapper.emitted('cancel')).toHaveLength(1)
    } finally {
        wrapper.unmount()
    }
})

it.each<ClassificationResult>([
    { item_type: 'launcher', launcher_type: 'PCL2', base_path: 'C:\\PCL' },
    { item_type: 'launcher', launcher_type: 'Generic', base_path: 'C:\\packs\\instance.zip' },
    {
        item_type: 'launcher',
        launcher_type: 'Generic',
        base_path: 'D:\\Minecraft\\.minecraft\\versions\\1.21.1',
    },
    { item_type: 'hmcl_launcher', launcher_dir: 'C:\\HMCL' },
])(
    'keeps instance import for launcher and archive sources: $item_type $launcher_type',
    (classification) => {
        const wrapper = mount(ConfirmDropTypeModal, {
            props: { fileName: 'source', classification },
        })
        try {
            expect(
                wrapper.findAllComponents(BigOptionButton).map((option) => option.props('title')),
            ).toEqual(['Instance'])
        } finally {
            wrapper.unmount()
        }
    },
)
