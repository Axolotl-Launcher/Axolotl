import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import ContentEmptyState from './ContentEmptyState.vue'
import ContentToolbar from './ContentToolbar.vue'

vi.mock('#ui/composables/i18n', async (original) => ({
    ...(await original<object>()),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it.each([ContentToolbar, ContentEmptyState])(
    'shows live refresh progress inside the existing action',
    async (component) => {
        const wrapper = mount(component, {
            props: {
                searchableItemCount: 4,
                searchQuery: '',
                contentTypeLabel: 'mod',
                refreshing: true,
                refreshStatus: '刷新…… 2/4',
            },
            global: { directives: { tooltip: {} } },
        })
        const button = () =>
            wrapper.findAll('button').find((item) => item.text().startsWith('刷新……'))!
        expect(button().text()).toBe('刷新…… 2/4')
        expect(button().attributes('disabled')).toBeDefined()
        expect(button().attributes('aria-busy')).toBe('true')
        await wrapper.setProps({ refreshStatus: '刷新…… 3/4' })
        expect(button().text()).toBe('刷新…… 3/4')
        await wrapper.setProps({ refreshing: false, refreshStatus: null })
        const refresh = wrapper.findAll('button').find((item) => item.text() === 'Refresh')!
        expect(refresh.attributes('disabled')).toBeUndefined()
        await refresh.trigger('click')
        expect(wrapper.emitted('refresh')).toHaveLength(1)
        wrapper.unmount()
    },
)
