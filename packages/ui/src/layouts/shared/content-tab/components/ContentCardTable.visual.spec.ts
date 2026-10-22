import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { ref } from 'vue'

import type { ContentCardTableItem } from '../types'
import ContentCardTable from './ContentCardTable.vue'

vi.mock('#ui/composables/i18n', () => ({
    defineMessage: (value: unknown) => value,
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        locale: ref('en-US'),
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
vi.mock('#ui/composables/how-ago', () => ({ useRelativeTime: () => () => 'today' }))

it('keeps the collapse control available without row actions and shows copy paths, sources and versions after expansion', async () => {
    const project = { id: 'verified', slug: null, title: 'Resource', icon_url: null }
    const group: ContentCardTableItem = {
        id: 'duplicate:verified',
        group: 'duplicate:verified',
        isGroupHeader: true,
        groupKind: 'duplicate',
        groupItemCount: 2,
        groupChildIds: ['a', 'b'],
        groupStatus: '2 matching · 1 disabled',
        project,
    }
    const child: ContentCardTableItem = {
        id: 'b',
        group: group.group,
        groupKind: 'duplicate',
        project,
        localPath: 'mods/custom-b.jar.disabled',
        installationSource: 'CurseForge',
        version: { id: 'release-b', version_number: '1.2', file_name: 'custom-b.jar.disabled' },
    }
    const wrapper = mount(ContentCardTable, {
        props: { items: [group], virtualized: false },
        global: { stubs: { RouterLink: true }, directives: { tooltip: {} } },
    })
    try {
        expect(wrapper.text()).toContain('(2)')
        expect(wrapper.text()).toContain('1 disabled')
        await wrapper
            .find('[data-content-card-item-id="duplicate:verified"] button')
            .trigger('click')
        expect(wrapper.emitted('toggleExpand')).toEqual([[group.group]])
        await wrapper.setProps({ items: [group, child], expandedGroups: new Set([group.group!]) })
        expect(wrapper.text()).toContain('mods/custom-b.jar.disabled')
        expect(wrapper.text()).toContain('CurseForge')
        expect(wrapper.text()).toContain('1.2')
        await wrapper.setProps({
            items: [{ ...group, hasUpdate: true, updateLabel: 'Review all 2 copies' }],
            onUpdate: () => {},
        })
        expect(wrapper.find('[aria-label="Review all 2 copies"]').exists()).toBe(true)
        await wrapper.find('[aria-label="Review all 2 copies"]').trigger('click')
        expect(wrapper.emitted('update')).toEqual([[group.id]])
    } finally {
        wrapper.unmount()
    }
})
