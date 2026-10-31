import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import ContentTypeFilter from './ContentTypeFilter.vue'

vi.mock('#ui/composables/i18n', () => ({
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('renders type filters without a redundant All option', () => {
    const wrapper = mount(ContentTypeFilter, {
        props: {
            selected: ['mod'],
            options: [
                { id: 'mod', label: 'Mods' },
                { id: 'resourcepack', label: 'Resource packs' },
            ],
            filterCounts: { mod: 4, resourcepack: 2 },
        },
    })

    try {
        const labels = wrapper
            .findAll('button')
            .map((button) => button.text().replace(/\s+/g, ' ').trim())
        expect(labels).toEqual(['Mods 4', 'Resource packs 2'])
        expect(labels.some((label) => label.startsWith('All'))).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('emits the clicked type filter', async () => {
    const wrapper = mount(ContentTypeFilter, {
        props: {
            selected: ['mod'],
            options: [
                { id: 'mod', label: 'Mods' },
                { id: 'resourcepack', label: 'Resource packs' },
            ],
            filterCounts: { mod: 4, resourcepack: 2 },
        },
    })

    try {
        await wrapper.findAll('button')[1].trigger('click')
        expect(wrapper.emitted('toggle')?.[0]?.[0]).toBe('resourcepack')
    } finally {
        wrapper.unmount()
    }
})
