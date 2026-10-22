import { expect, it } from 'vitest'

import FileTreeSelect from '../components/base/FileTreeSelect.vue'
import { mountThemed, waitFor } from './visual-harness'

const items = [
    { path: 'mods', type: 'directory' as const },
    { path: 'mods/deep', type: 'directory' as const },
    { path: 'mods/a.jar', type: 'file' as const, size: 10 },
    { path: 'mods/deep/b.jar', type: 'file' as const, size: 20 },
    { path: 'mods/blocked.jar', type: 'file' as const, disabled: true },
    { path: 'options.txt', type: 'file' as const, size: 30 },
]

it('indexes many sibling directories without losing descendants during selection', async () => {
    const items = Array.from({ length: 1000 }, (_, index) => ({
        path: `folder-${index}/nested/file.txt`,
        type: 'file' as const,
    }))
    const wrapper = await mountThemed(FileTreeSelect, { items, modelValue: [] }, 'dark')
    try {
        await wrapper
            .find('[role="checkbox"][aria-label="Select all files in this view"]')
            .trigger('click')
        const next = wrapper.emitted('update:modelValue')!.at(-1)![0] as string[]
        expect(next).toHaveLength(1000)
        expect(new Set(next).size).toBe(1000)
    } finally {
        wrapper.unmount()
    }
})

it('renders a bounded window while selecting the complete matching snapshot', async () => {
    const files = Array.from({ length: 2000 }, (_, index) => ({
        path: `screen-${index}.png`,
        type: 'file' as const,
    }))
    const wrapper = await mountThemed(FileTreeSelect, { items: files, modelValue: [] }, 'dark')
    try {
        expect(wrapper.findAll('[role="checkbox"]').length).toBeLessThan(100)
        await wrapper
            .find('[role="checkbox"][aria-label="Select all files in this view"]')
            .trigger('click')
        expect(wrapper.emitted('update:modelValue')!.at(-1)![0] as string[]).toHaveLength(2000)
        const viewport = wrapper.find('.overflow-y-auto').element as HTMLElement
        viewport.scrollTop = viewport.scrollHeight
        viewport.dispatchEvent(new Event('scroll'))
        await waitFor(() => wrapper.text().includes('screen-1999.png'))
        await wrapper.setProps({ searchQuery: 'screen-0.png' })
        await waitFor(() => viewport.scrollTop === 0)
        expect(wrapper.text()).toContain('screen-0.png')
    } finally {
        wrapper.unmount()
    }
})

it('selects unopened descendants as exact files and updates partial folder state', async () => {
    const wrapper = await mountThemed(FileTreeSelect, { items, modelValue: [] }, 'dark')
    try {
        await wrapper.find('[role="checkbox"][aria-label="mods"]').trigger('click')
        const next = wrapper.emitted('update:modelValue')!.at(-1)![0]
        expect(next).toEqual(['mods/a.jar', 'mods/deep/b.jar'])
        await wrapper.setProps({ modelValue: ['mods/deep/b.jar'] })
        expect(
            wrapper.find('[role="checkbox"][aria-label="mods"]').attributes('aria-checked'),
        ).toBe('mixed')
        await wrapper.find('[role="checkbox"][aria-label="mods"]').trigger('click')
        expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toEqual([
            'mods/deep/b.jar',
            'mods/a.jar',
        ])
    } finally {
        wrapper.unmount()
    }
})

it('search and selected views change visible scope without discarding hidden selections', async () => {
    const wrapper = await mountThemed(
        FileTreeSelect,
        { items, modelValue: ['options.txt'], searchQuery: 'deep/b' },
        'light',
    )
    try {
        expect(wrapper.text()).toContain('mods/deep/b.jar')
        expect(wrapper.text()).not.toContain('options.txt')
        await wrapper.find('[role="checkbox"][aria-label="mods/deep/b.jar"]').trigger('click')
        expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toEqual([
            'options.txt',
            'mods/deep/b.jar',
        ])
        await wrapper.setProps({
            modelValue: ['options.txt', 'mods/deep/b.jar'],
            searchQuery: '',
            selectedOnly: true,
        })
        await wrapper.find('[role="checkbox"][aria-label="mods"]').trigger('click')
        expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toEqual(['options.txt'])
        await wrapper.setProps({ currentPath: 'mods/deep', selectedOnly: false })
        expect(wrapper.text()).toContain('b.jar')
        expect(wrapper.text()).not.toContain('options.txt')
    } finally {
        wrapper.unmount()
    }
})
