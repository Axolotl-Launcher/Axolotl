import { flushPromises } from '@vue/test-utils'
import { expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import ProjectPageVersions from '../components/project/ProjectPageVersions.vue'
import { mountThemed } from './visual-harness'

it('clamps stale pages to available versions and follows browser pagination changes', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/project/:id/versions', component: { template: '<div />' } }],
    })
    await router.push('/project/test/versions?page=8')
    const versions = Array.from({ length: 25 }, (_, index) => ({
        id: `${index}`,
        name: `Release ${index}`,
        version_number: `Release ${index}`,
        date_published: '2026-10-14T00:00:00Z',
        downloads: 1,
        version_type: 'release',
        game_versions: ['1.21.1'],
        loaders: ['neoforge'],
        files: [],
        displayUrlEnding: `${index}`,
    }))
    const wrapper = await mountThemed(
        ProjectPageVersions,
        {
            project: { id: 'test', project_type: 'mod' },
            versions,
            loaders: [],
            gameVersions: [{ version: '1.21.1', version_type: 'release' }],
        },
        'dark',
        { global: { plugins: [router] } },
    )
    await flushPromises()
    expect(wrapper.text()).toContain('Release 20')
    expect(wrapper.text()).not.toContain('No data available.')
    await router.replace('/project/test/versions?page=1')
    await flushPromises()
    expect(wrapper.text()).toContain('Release 0')
    await wrapper.setProps({ versions: versions.slice(0, 1) })
    await router.replace('/project/test/versions?page=99')
    await flushPromises()
    expect(wrapper.text()).toContain('Release 0')
    wrapper.unmount()
})

it('explains unmatched filters in both the table and the narrow window list', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/project/:id/versions', component: { template: '<div />' } }],
    })
    await router.push('/project/test/versions?g=1.20.1')
    const wrapper = await mountThemed(
        ProjectPageVersions,
        {
            project: { id: 'test', project_type: 'mod' },
            versions: [
                {
                    id: 'new',
                    name: 'New',
                    version_number: 'New',
                    date_published: '2026-10-14T00:00:00Z',
                    downloads: 1,
                    version_type: 'release',
                    game_versions: ['1.21.1'],
                    loaders: ['neoforge'],
                    files: [],
                    displayUrlEnding: 'new',
                },
            ],
            loaders: [],
            gameVersions: [{ version: '1.21.1', version_type: 'release' }],
        },
        'dark',
        { global: { plugins: [router] } },
    )
    await flushPromises()
    const emptyStates = wrapper
        .findAll('p')
        .filter((paragraph) => paragraph.text() === 'No versions match the selected filters.')
    expect(emptyStates).toHaveLength(2)
    expect(wrapper.text()).not.toContain('No data available.')
    wrapper.unmount()
})
