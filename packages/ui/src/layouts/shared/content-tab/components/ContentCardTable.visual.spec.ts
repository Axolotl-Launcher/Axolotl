import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { h, nextTick, ref } from 'vue'

import { mountThemed, waitFor } from '../../../../test/visual-harness'
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

it('keeps the loading status below the filters in the sticky header and allows it to hide and reappear', async () => {
    const host = document.createElement('div')
    host.style.cssText = 'height:240px;width:900px;overflow-y:auto'
    document.body.append(host)
    const loading = ref(true)
    const wrapper = await mountThemed(
        ContentCardTable,
        {
            items: Array.from({ length: 30 }, (_, index) => ({
                id: String(index),
                project: { id: String(index), title: `Project ${index}` },
            })),
            virtualized: false,
        },
        'dark',
        {
            attachTo: host,
            slots: {
                'header-project': () => h('span', 'All'),
                'header-status': () =>
                    loading.value
                        ? h(
                              'div',
                              { role: 'status', class: 'px-3 pb-2' },
                              'Loading online information',
                          )
                        : null,
            },
            global: { stubs: { RouterLink: true }, directives: { tooltip: {} } },
        },
    )
    try {
        const status = () => wrapper.get('[role="status"]').element.getBoundingClientRect()
        const header = wrapper.get('[role="rowgroup"]').element
        const filters = header.querySelector('[role="row"]')!
        expect(status().top).toBeGreaterThanOrEqual(filters.getBoundingClientRect().bottom)
        expect(status().bottom).toBeLessThanOrEqual(header.getBoundingClientRect().bottom)
        expect(
            wrapper.get('[data-content-card-item]').element.getBoundingClientRect().top,
        ).toBeGreaterThanOrEqual(status().bottom)

        host.scrollTop = 300
        host.dispatchEvent(new Event('scroll'))
        await waitFor(
            () =>
                Math.abs(header.getBoundingClientRect().top - host.getBoundingClientRect().top) <=
                1,
        )
        expect(status().top).toBeGreaterThanOrEqual(filters.getBoundingClientRect().bottom)
        expect(status().bottom).toBeLessThanOrEqual(header.getBoundingClientRect().bottom)

        loading.value = false
        await nextTick()
        expect(wrapper.find('[role="status"]').exists()).toBe(false)
        loading.value = true
        await nextTick()
        expect(wrapper.find('[role="status"]').exists()).toBe(true)
    } finally {
        wrapper.unmount()
        host.remove()
    }
})

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
