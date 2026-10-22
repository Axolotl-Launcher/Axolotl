import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import ConfirmDeleteInstanceModal from './ConfirmDeleteInstanceModal.vue'

vi.mock('@/helpers/instance-backup', () => ({ getBackupDeleteSummary: async () => null }))
vi.mock('@/components/ui/InstanceIcon.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['instanceId'],
            setup: (props) => () => h('img', { 'data-instance-icon': props.instanceId }),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const container = defineComponent({
        setup: (_, { slots, expose }) => {
            expose({ show: () => {} })
            return () => h('div', slots.default?.())
        },
    })
    return {
        ConfirmActionModal: container,
        Admonition: container,
        defineMessages: (value: unknown) => value,
        useFormatBytes: () => String,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('shows full names, icons and distinguishing paths for same-name targets without substituting internal IDs', () => {
    const name = '同名实例'.repeat(20)
    const wrapper = mount(ConfirmDeleteInstanceModal, {
        props: {
            count: 2,
            deleteAction: vi.fn(),
            instances: [
                {
                    id: 'opaque-one',
                    name,
                    loader: 'fabric',
                    game_version: '1.21.1',
                    path: 'C:/Games/One',
                    icon_path: 'one.png',
                },
                {
                    id: 'opaque-two',
                    name,
                    loader: 'forge',
                    game_version: '1.20.1',
                    path: 'C:/Games/Two',
                    icon_path: 'two.png',
                },
            ],
        },
    })
    try {
        const rows = wrapper.findAll('li')
        expect(rows).toHaveLength(2)
        expect(rows[0].text()).toContain(name)
        expect(rows[0].text()).toContain('fabric 1.21.1')
        expect(rows[0].text()).toContain('C:/Games/One')
        expect(rows[1].text()).toContain('C:/Games/Two')
        expect(wrapper.text()).not.toContain('opaque-one')
        expect(wrapper.findAll('[data-instance-icon]')).toHaveLength(2)
        expect(rows[0].get('.font-semibold').classes()).not.toContain('truncate')
    } finally {
        wrapper.unmount()
    }
})
