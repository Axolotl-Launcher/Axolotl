import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import ResourceRoute from './ResourceRoute.vue'

const api = vi.hoisted(() => ({ get: vi.fn(), replace: vi.fn() }))
vi.mock('@/helpers/resources', () => ({ resourceGet: api.get }))
vi.mock('vue-router', () => ({
    useRoute: () => ({
        path: '/resource/known',
        params: { id: 'known' },
        query: { i: 'instance' },
        hash: '',
    }),
    useRouter: () => ({ replace: api.replace }),
}))
vi.mock('@modrinth/ui', async () => {
    const { h } = await import('vue')
    return {
        Button: { render: () => null },
        defineMessages: (value: unknown) => value,
        useVIntl: () => ({ formatMessage: () => 'Failed' }),
        ProjectPageSkeleton: {
            props: ['title'],
            setup: (props: { title?: string }) => () => h('h1', props.title),
        },
    }
})

it('keeps the title through resource resolution and forwards the authoritative destination title', async () => {
    let resolve!: (value: unknown) => void
    api.get.mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    window.history.replaceState(
        {
            ...window.history.state,
            projectPreview: { targetPath: '/resource/known', title: 'Known project' },
        },
        '',
    )
    const wrapper = mount(ResourceRoute)
    try {
        expect(wrapper.get('h1').text()).toBe('Known project')
        resolve({
            defaultProject: { source: 'curseforge', projectId: '42' },
            projects: [
                {
                    project: { source: 'curseforge', projectId: '42' },
                    title: 'Authoritative project',
                },
            ],
        })
        await flushPromises()
        expect(api.replace).toHaveBeenCalledWith({
            path: '/project/curseforge/42',
            query: { i: 'instance' },
            hash: '',
            state: {
                projectPreview: {
                    targetPath: '/project/curseforge/42',
                    title: 'Authoritative project',
                },
            },
        })
    } finally {
        wrapper.unmount()
        window.history.replaceState({}, '')
    }
})
