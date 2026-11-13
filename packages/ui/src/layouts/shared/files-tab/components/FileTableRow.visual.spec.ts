import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import FileTableRow from './FileTableRow.vue'

vi.mock('#ui/composables/i18n', () => ({
    defineMessage: (value: unknown) => value,
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (
            message: { id: string; defaultMessage: string },
            values?: { count: number },
        ) =>
            message.id === 'files.row.item-count'
                ? `${values?.count} items`
                : message.defaultMessage,
    }),
}))
vi.mock('#ui/composables', () => ({ useFormatBytes: () => (size: number) => `${size} bytes` }))
vi.mock('#ui/composables/format-date-time', () => ({
    useFormatDateTime: () => () => 'date',
}))
vi.mock('#ui/providers/web-notifications', () => ({
    injectNotificationManager: () => ({ addNotification: vi.fn() }),
}))
vi.mock('../providers/file-manager', () => ({ injectFileManager: () => ({}) }))

it('leaves an unknown directory count blank and still displays a known empty directory', async () => {
    const wrapper = mount(FileTableRow, {
        props: {
            name: 'schematics',
            path: 'schematics',
            type: 'directory',
            created: 0,
            modified: 0,
            index: 0,
            isLast: true,
            selected: false,
        },
        global: {
            stubs: { Checkbox: true, TeleportOverflowMenu: true },
        },
    })
    try {
        expect(wrapper.text()).not.toContain('0 items')
        await wrapper.setProps({ count: 0 })
        expect(wrapper.text()).toContain('0 items')
        await wrapper.setProps({ count: 5000 })
        expect(wrapper.text()).toContain('5000 items')
    } finally {
        wrapper.unmount()
    }
})
