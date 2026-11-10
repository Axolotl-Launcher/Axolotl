import { MultiSelect } from '@modrinth/ui'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import { mountThemed } from '../../test/visual-harness'
import VersionFilterControl from './VersionFilterControl.vue'

let teleports: HTMLDivElement
beforeEach(() => {
    teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
})
afterEach(() => teleports.remove())

it('follows back/forward filter queries and preserves explicitly cleared game and loader axes', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/', component: { render: () => null } }],
    })
    await router.push({ path: '/', query: { g: '1.21.1', l: 'neoforge', c: 'beta' } })
    const wrapper = await mountThemed(
        VersionFilterControl,
        { versions: [], gameVersions: [], loaders: [] },
        'dark',
        { global: { plugins: [router] } },
    )
    try {
        expect(wrapper.vm.selectedGameVersions).toEqual(['1.21.1'])
        expect(wrapper.vm.selectedPlatforms).toEqual(['neoforge'])
        await router.replace({ query: { g: 'custom-version', l: 'fabric' } })
        expect(wrapper.vm.selectedGameVersions).toEqual(['custom-version'])
        expect(wrapper.vm.selectedPlatforms).toEqual(['fabric'])
        await wrapper.vm.toggleFilter('gameVersion', 'custom-version')
        const query = wrapper.emitted('update:query')!.at(-1)![0] as Record<string, unknown>
        expect(query.g).toBe('')
        await wrapper.vm.toggleFilter('platform', 'fabric')
        expect((wrapper.emitted('update:query')!.at(-1)![0] as Record<string, unknown>).l).toBe('')
    } finally {
        wrapper.unmount()
    }
})

it('channel edits keep untouched game and platform axes absent until explicitly cleared', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/', component: { render: () => null } }],
    })
    await router.push('/')
    const wrapper = await mountThemed(
        VersionFilterControl,
        { versions: [], gameVersions: [], loaders: [] },
        'dark',
        { global: { plugins: [router] } },
    )
    const vm = wrapper.vm as unknown as {
        toggleFilter: (type: string, value: string) => Promise<void>
        clearFilters: () => Promise<void>
    }
    const lastQuery = () => wrapper.emitted('update:query')!.at(-1)![0] as Record<string, unknown>
    try {
        await vm.toggleFilter('channel', 'beta')
        expect(lastQuery()).toMatchObject({ c: ['beta'], g: undefined, l: undefined })
        await router.replace({ query: { c: ['beta'] } })
        await vm.toggleFilter('gameVersion', '1.21.1')
        await vm.toggleFilter('gameVersion', '1.21.1')
        expect(lastQuery()).toMatchObject({ g: '', l: undefined })
        await router.replace({ query: { c: ['beta'], g: '' } })
        await vm.toggleFilter('channel', 'alpha')
        expect(lastQuery()).toMatchObject({ g: '', l: undefined })
        await router.replace({ query: { c: ['beta'], l: '' } })
        await vm.clearFilters()
        expect(lastQuery()).toMatchObject({ c: [], g: undefined, l: '' })
        await router.replace({ query: {} })
        await vm.toggleFilter('channel', 'beta')
        expect(lastQuery()).toMatchObject({ g: undefined, l: undefined })
    } finally {
        wrapper.unmount()
    }
})

it('restored snapshots become visible after tags are loaded while user visibility choices persist', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/', component: { render: () => null } }],
    })
    const versions = [
        { game_versions: ['1.21.1'], loaders: ['fabric'], version_type: 'release' },
        { game_versions: ['24w14a'], loaders: ['fabric'], version_type: 'beta' },
    ] as never
    await router.push({ path: '/', query: { g: '24w14a' } })
    await router.push({ path: '/', query: { g: '1.21.1' } })
    const wrapper = await mountThemed(
        VersionFilterControl,
        { versions, gameVersions: [], loaders: [] },
        'dark',
        { global: { plugins: [router] } },
    )
    const vm = wrapper.vm as unknown as {
        showSnapshots: boolean
        updateShowSnapshots: (value: boolean) => void
    }
    const includesSnapshot = () =>
        wrapper
            .findAllComponents(MultiSelect)
            .some((component) =>
                component
                    .props('options')
                    .some((option: { value: string }) => option.value === '24w14a'),
            )
    try {
        await wrapper.setProps({
            gameVersions: [
                { version: '1.21.1', version_type: 'release' },
                { version: '24w14a', version_type: 'snapshot' },
            ] as never,
        })
        expect(vm.showSnapshots).toBe(false)
        expect(includesSnapshot()).toBe(false)
        router.back()
        await vi.waitFor(() => expect(router.currentRoute.value.query.g).toBe('24w14a'))
        await nextTick()
        expect(vm.showSnapshots).toBe(true)
        expect(includesSnapshot()).toBe(true)
        router.forward()
        await vi.waitFor(() => expect(router.currentRoute.value.query.g).toBe('1.21.1'))
        await nextTick()
        expect(vm.showSnapshots).toBe(true)
        vm.updateShowSnapshots(false)
        await nextTick()
        expect(vm.showSnapshots).toBe(false)
        await wrapper.setProps({ gameVersions: [...wrapper.props('gameVersions')] })
        expect(vm.showSnapshots).toBe(false)
        router.back()
        await vi.waitFor(() => expect(router.currentRoute.value.query.g).toBe('24w14a'))
        await nextTick()
        expect(vm.showSnapshots).toBe(true)
        // The dropdown's snapshot visibility and its rendered checkbox agree.
        const gameDropdown = wrapper
            .findAllComponents(MultiSelect)
            .find((component) => component.props('searchable'))!
        expect(gameDropdown.exists()).toBe(true)
        await gameDropdown.get('[role="button"]').trigger('keydown', { key: 'Enter' })
        await vi.waitFor(() =>
            expect(
                teleports
                    .querySelector('[role="checkbox"][aria-label="Show all versions"]')
                    ?.getAttribute('aria-checked'),
            ).toBe('true'),
        )
        expect(includesSnapshot()).toBe(true)
    } finally {
        wrapper.unmount()
    }
})
