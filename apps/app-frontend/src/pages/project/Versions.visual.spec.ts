import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import { get_game_versions, get_loaders } from '@/helpers/tags.js'

import Versions from './Versions.vue'

vi.mock('vue-router', () => ({ useRoute: () => ({ params: {}, query: {} }) }))
vi.mock('@/helpers/tags.js', () => ({ get_game_versions: vi.fn(), get_loaders: vi.fn() }))
vi.mock('@/store/theme.ts', () => ({ useTheming: () => ({ featureFlags: {} }) }))
vi.mock('@/assets/icons/index.js', () => ({ SwapIcon: defineComponent(() => () => h('i')) }))
vi.mock('@modrinth/assets', () => {
    const icon = defineComponent(() => () => h('i'))
    return { CheckIcon: icon, DownloadIcon: icon, ExternalIcon: icon, ServerIcon: icon }
})
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
    ProjectPageVersions: defineComponent({
        props: ['loaders', 'gameVersions', 'versions'],
        setup: (props) => () =>
            h(
                'p',
                JSON.stringify({
                    versions: props.versions.map((version: { id: string }) => version.id),
                    loaders: props.loaders,
                    gameVersions: props.gameVersions,
                }),
            ),
    }),
    commonMessages: {},
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

function mountVersions() {
    return mount(
        defineComponent({
            setup: () => () =>
                h(Suspense, null, {
                    default: () =>
                        h(Versions, {
                            project: { id: 'project' },
                            versions: [{ id: 'release' }],
                            install: () => {},
                            installed: false,
                            installing: false,
                            installedVersion: '',
                        } as never),
                    fallback: () => h('span', 'suspended'),
                }),
        }),
        { global: { directives: { tooltip: {} } } },
    )
}

it('lists versions before the loader and game version tags arrive', async () => {
    let finish!: () => void
    const tags = new Promise<void>((resolve) => {
        finish = resolve
    })
    vi.mocked(get_loaders).mockReturnValue(tags.then(() => [{ name: 'fabric' }]) as never)
    vi.mocked(get_game_versions).mockReturnValue(tags.then(() => [{ version: '1.21' }]) as never)
    const wrapper = mountVersions()
    try {
        await flushPromises()
        expect(wrapper.text()).not.toContain('suspended')
        expect(JSON.parse(wrapper.get('p').text())).toEqual({
            versions: ['release'],
            loaders: [],
            gameVersions: [],
        })

        finish()
        await flushPromises()
        expect(JSON.parse(wrapper.get('p').text())).toEqual({
            versions: ['release'],
            loaders: [{ name: 'fabric' }],
            gameVersions: [{ version: '1.21' }],
        })
    } finally {
        wrapper.unmount()
    }
})

it('keeps empty tag lists when loading the tags fails', async () => {
    vi.mocked(get_loaders).mockRejectedValue(new Error('offline'))
    vi.mocked(get_game_versions).mockRejectedValue(new Error('offline'))
    const wrapper = mountVersions()
    try {
        await flushPromises()
        expect(JSON.parse(wrapper.get('p').text())).toEqual({
            versions: ['release'],
            loaders: [],
            gameVersions: [],
        })
    } finally {
        wrapper.unmount()
    }
})
