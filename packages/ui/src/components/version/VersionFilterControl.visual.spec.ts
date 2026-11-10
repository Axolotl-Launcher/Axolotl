import { expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import { mountThemed } from '../../test/visual-harness'
import VersionFilterControl from './VersionFilterControl.vue'

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
