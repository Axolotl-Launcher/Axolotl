import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import ConfirmDeleteInstanceModal from './ConfirmDeleteInstanceModal.vue'

const { handleError } = vi.hoisted(() => ({ handleError: vi.fn() }))

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
        name: 'ConfirmActionModal',
        props: ['confirmAction'],
        setup: (_, { slots, expose }) => {
            expose({ show: () => {} })
            return () => h('div', slots.default?.())
        },
    })
    return {
        ConfirmActionModal: container,
        Admonition: container,
        defineMessages: (value: unknown) => value,
        injectNotificationManager: () => ({ handleError }),
        useFormatBytes: () => String,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('reports a deletion failure once and preserves the rejection so the dialog can retry', async () => {
    handleError.mockClear()
    const failure = { field_name: 'Theseus', message: 'Directory not empty (os error 145)' }
    const deleteAction = vi.fn().mockRejectedValueOnce(failure).mockResolvedValueOnce(undefined)
    const wrapper = mount(ConfirmDeleteInstanceModal, { props: { deleteAction } })
    try {
        const confirm = wrapper.findComponent({ name: 'ConfirmActionModal' })
        const action = confirm.props('confirmAction') as () => Promise<void>
        await expect(action()).rejects.toBe(failure)
        expect(handleError).toHaveBeenCalledExactlyOnceWith(failure)
        await expect(action()).resolves.toBeUndefined()
        expect(deleteAction).toHaveBeenCalledTimes(2)
        expect(handleError).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
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
