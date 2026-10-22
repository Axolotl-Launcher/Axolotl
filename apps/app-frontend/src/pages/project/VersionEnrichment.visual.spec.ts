import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, reactive, Suspense } from 'vue'

import { get_project_many } from '@/helpers/cache.js'

import Version from './Version.vue'

const route = reactive({ params: { id: 'project', version: 'initial' }, query: {} })
vi.mock('vue-router', () => ({ useRoute: () => route }))
vi.mock('@/helpers/cache.js', () => ({ get_project_many: vi.fn(), get_version_many: vi.fn() }))
vi.mock('@/store/breadcrumbs', () => ({ useBreadcrumbs: () => ({ setName: vi.fn() }) }))
vi.mock('@/components/ui/ResourceReleaseChangelog.vue', () => ({
    default: defineComponent({ setup: () => () => h('p') }),
}))
vi.mock('@modrinth/ui', () => ({
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    Button: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('button', slots.default?.()),
    }),
    ButtonLink: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('a', slots.default?.()),
    }),
    OverflowMenu: defineComponent({ setup: () => () => h('span') }),
    VersionPage: defineComponent({
        props: ['version', 'enrichment', 'enrichmentLoading'],
        setup: (props) => () =>
            h(
                'p',
                JSON.stringify({
                    version: props.version.id,
                    enrichment: props.enrichment,
                    loading: props.enrichmentLoading,
                }),
            ),
    }),
    defineMessage: (value: unknown) => value,
    defineMessages: (value: unknown) => value,
    commonMessages: {},
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('switching release routes preserves the new version’s dependency details when an older response arrives late', async () => {
    let complete!: (value: unknown[]) => void
    vi.mocked(get_project_many)
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    complete = resolve
                }),
        )
        .mockResolvedValueOnce([{ id: 'new-dependency' }])
    const versions = [
        { id: 'initial', name: 'Initial', dependencies: [] },
        { id: 'old', name: 'Old', dependencies: [{ project_id: 'old-dependency' }] },
        { id: 'new', name: 'New', dependencies: [{ project_id: 'new-dependency' }] },
    ]
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(Suspense, null, {
                    default: () =>
                        h(Version, {
                            project: { id: 'project' },
                            versions,
                            members: [],
                            install: () => {},
                            installed: false,
                            installing: false,
                            installedVersion: '',
                        } as never),
                }),
        }),
        { global: { stubs: { RouterLink: true }, directives: { tooltip: {} } } },
    )
    try {
        await flushPromises()
        route.params.version = 'old'
        await flushPromises()
        route.params.version = 'new'
        await flushPromises()
        expect(wrapper.text()).toContain('new-dependency')
        complete([{ id: 'old-dependency' }])
        await flushPromises()
        expect(wrapper.text()).toContain('new-dependency')
        expect(wrapper.text()).not.toContain('old-dependency')
        expect(wrapper.text()).toContain('"loading":false')
    } finally {
        wrapper.unmount()
    }
})
