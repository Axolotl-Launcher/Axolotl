import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { get_project_v3_many } from '@/helpers/cache.js'
import type { GameInstance } from '@/helpers/types'

import Modpacks from './Modpacks.vue'
import Servers from './Servers.vue'

vi.mock('@/helpers/cache.js', () => ({ get_project_v3_many: vi.fn() }))
vi.mock('@/components/GridDisplay.vue', () => ({
    default: defineComponent({
        props: ['instances'],
        setup: (props) => () =>
            h(
                'div',
                props.instances.map((instance: GameInstance) =>
                    h('span', { 'data-instance': instance.id }, instance.name),
                ),
            ),
    }),
}))

const instances = [
    {
        id: 'cf',
        name: 'CurseForge pack',
        link: { type: 'curseforge_modpack', project_id: '1266680', version_id: '1' },
    },
    {
        id: 'mr',
        name: 'Modrinth pack',
        link: { type: 'modrinth_modpack', project_id: '6HvKwSky', version_id: 'abcdefgh' },
    },
    {
        id: 'legacy-server',
        name: 'Legacy server',
        link: { type: 'modrinth_modpack', project_id: 'server01', version_id: 'abcdefgh' },
    },
    { id: 'server', name: 'Server', link: { type: 'server_project', project_id: 'server02' } },
    {
        id: 'server-pack',
        name: 'Server pack',
        link: {
            type: 'server_project_modpack',
            server_project_id: 'server03',
            content_project_id: 'content1',
            content_version_id: 'abcdefgh',
        },
    },
] as GameInstance[]

it('library server detection excludes CurseForge IDs and retains provider-specific categories', async () => {
    vi.mocked(get_project_v3_many)
        .mockReset()
        .mockResolvedValue([
            { id: '6HvKwSky', minecraft_server: null },
            { id: 'server01', minecraft_server: {} },
        ])
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const root = defineComponent({
        setup: () => () => h('main', [h(Modpacks, { instances }), h(Servers, { instances })]),
    })
    const wrapper = mount(root, {
        global: { plugins: [[VueQueryPlugin, { queryClient: client }]] },
    })
    try {
        await flushPromises()
        expect(get_project_v3_many).toHaveBeenCalledTimes(1)
        expect(get_project_v3_many).toHaveBeenCalledWith(
            ['6HvKwSky', 'server01'],
            'must_revalidate',
        )
        expect(
            wrapper
                .findComponent(Modpacks)
                .findAll('[data-instance]')
                .map((node) => node.attributes('data-instance')),
        ).toEqual(['cf', 'mr'])
        expect(
            wrapper
                .findComponent(Servers)
                .findAll('[data-instance]')
                .map((node) => node.attributes('data-instance')),
        ).toEqual(['legacy-server', 'server', 'server-pack'])
    } finally {
        wrapper.unmount()
        client.clear()
    }
})

it('a CurseForge-only library requires no Modrinth metadata request', async () => {
    vi.mocked(get_project_v3_many).mockReset()
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const wrapper = mount(Modpacks, {
        props: { instances: [instances[0]] },
        global: { plugins: [[VueQueryPlugin, { queryClient: client }]] },
    })
    try {
        await flushPromises()
        expect(get_project_v3_many).not.toHaveBeenCalled()
        expect(wrapper.text()).toContain('CurseForge pack')
    } finally {
        wrapper.unmount()
        client.clear()
    }
})
