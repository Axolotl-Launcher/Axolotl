import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import Version from './Version.vue'

vi.mock('vue-router', () => ({ useRoute: () => ({ params: { version: 'version-a' }, query: {} }) }))
vi.mock('@/store/breadcrumbs', () => ({ useBreadcrumbs: () => ({ setName: vi.fn() }) }))
vi.mock('@/helpers/cache.js', () => ({ get_project_many: vi.fn(), get_version_many: vi.fn() }))
vi.mock('@modrinth/ui', () => ({
    ProjectPageSkeleton: { render: () => null },
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    Button: { render: () => null },
    OverflowMenu: { render: () => null },
    ButtonLink: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('a', attrs, slots.default?.()),
    }),
    VersionPage: defineComponent({
        props: ['version'],
        setup:
            (props, { slots }) =>
            () =>
                h(
                    'div',
                    props.version.files.map((file: unknown) =>
                        slots.supplementaryResourceActions?.({ file }),
                    ),
                ),
    }),
    commonMessages: {},
    defineMessages: (messages: unknown) => messages,
    defineMessage: (message: unknown) => message,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('browser download links use the current CDN when a cached version contains retired URLs', async () => {
    const old = 'https://cdn-alt.modrinth.com/data/%e9%87%91/file%2B.jar?download=1#file'
    const version = {
        id: 'version-a',
        name: 'A',
        dependencies: [],
        files: [
            { url: old, filename: 'file.jar' },
            { url: 'https://cdn.modrinth.com/data/current.jar', filename: 'current.jar' },
            { url: 'https://mod.tianpao.top/files/1/2/mirror.jar', filename: 'mirror.jar' },
            { url: 'https://mod.tianpao.top/unknown', filename: 'unknown.jar' },
        ],
    }
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(
                    Suspense,
                    {},
                    {
                        default: () =>
                            h(Version, {
                                project: { id: 'project-a' },
                                versions: [version],
                                members: [],
                                install: vi.fn(),
                                installed: false,
                                installing: false,
                                installedVersion: '',
                            } as unknown as InstanceType<typeof Version>['$props']),
                    },
                ),
        }),
        { global: { stubs: { RouterLink: true }, directives: { tooltip: () => {} } } },
    )
    try {
        await flushPromises()
        const links = wrapper.findAll('a')
        expect(links[0].attributes('href')).toBe(
            'https://cdn.modrinth.com/data/%e9%87%91/file%2B.jar?download=1#file',
        )
        expect(links[0].attributes('download')).toBe('file.jar')
        expect(links[1].attributes('href')).toBe(version.files[1].url)
        expect(links[2].attributes('href')).toBe('https://edge.forgecdn.net/files/1/2/mirror.jar')
        expect(links[3].attributes('href')).toBeUndefined()
        expect(links[3].attributes('disabled')).toBeDefined()
        expect(version.files[0].url).toBe(old)
    } finally {
        wrapper.unmount()
    }
})
