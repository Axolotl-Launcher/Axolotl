import { flushPromises } from '@vue/test-utils'
import { expect, it } from 'vitest'

import { mountThemed } from '../../test/visual-harness'
import FileTreeSelect from './FileTreeSelect.vue'

it('navigates without a controlled path and follows subsequent external path updates', async () => {
    const wrapper = await mountThemed(
        FileTreeSelect,
        {
            items: [
                { path: 'mods', type: 'directory' },
                { path: 'mods/nested', type: 'directory' },
                { path: 'mods/nested/example.jar', type: 'file' },
            ],
            modelValue: [],
        },
        'dark',
    )
    try {
        await wrapper
            .findAll('[role="button"]')
            .find((row) => row.text().includes('mods'))!
            .trigger('click')
        await flushPromises()
        expect(wrapper.emitted('navigate')).toEqual([['mods']])
        expect(wrapper.text()).toContain('nested')
        expect(wrapper.text()).toContain('Parent folder')
        await wrapper.setProps({ currentPath: 'mods/nested' })
        expect(wrapper.text()).toContain('example.jar')
        await wrapper
            .findAll('[role="button"]')
            .find((row) => row.text().includes('Parent folder'))!
            .trigger('click')
        await flushPromises()
        expect(wrapper.emitted('navigate')?.at(-1)).toEqual(['mods'])
        expect(wrapper.text()).toContain('nested')
    } finally {
        wrapper.unmount()
    }
})
