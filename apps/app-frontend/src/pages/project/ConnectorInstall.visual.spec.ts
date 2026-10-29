import { open } from '@tauri-apps/plugin-dialog'
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, reactive, ref } from 'vue'

import { getMcArchiveModBySlug } from '@/helpers/mcarchive'
import { getPlanetMinecraftProject } from '@/helpers/planet-minecraft'

import McArchive from './McArchive.vue'
import PlanetMinecraft from './PlanetMinecraft.vue'

const route = reactive({
    path: '/project/mcarchive/old',
    params: { slug: 'old', id: 'old' },
    query: {},
})
const target = ref({ id: 'instance', name: 'Target' })
const showInstall = vi.fn()
const cancelInstall = vi.fn<() => Promise<void>>()
const trackJob = vi.fn()
vi.mock('vue-router', () => ({ useRoute: () => route }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))
vi.mock('@/helpers/mcarchive', () => ({ getMcArchiveModBySlug: vi.fn() }))
vi.mock('@/helpers/planet-minecraft', () => ({ getPlanetMinecraftProject: vi.fn() }))
vi.mock('@/store/breadcrumbs', () => ({
    useBreadcrumbs: () => ({ setName: vi.fn(), setNameIcon: vi.fn() }),
}))
vi.mock('@/providers/content-selection', () => ({
    injectContentSelection: () => ({
        targetInstance: target,
        instances: ref([target.value]),
        refreshInstances: vi.fn().mockResolvedValue(null),
        setTarget: (instance: typeof target.value) => {
            target.value = instance
        },
    }),
}))
vi.mock('@/providers/download-manager', () => ({ injectDownloadManager: () => ({ trackJob }) }))
vi.mock('@/components/ui/ResourceOperationPreviewModal.vue', () => ({
    default: defineComponent({
        setup: (_, { expose }) => {
            expose({ showInstall, cancel: cancelInstall })
            return () => h('div')
        },
    }),
}))
vi.mock('@/components/ui/ResourceDescription.vue', () => ({
    default: defineComponent({ setup: () => () => h('p') }),
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({
    default: defineComponent({ setup: () => () => h('span') }),
}))
vi.mock('@/components/browse/BrowseInstanceSelector.vue', () => ({
    default: defineComponent({ setup: () => () => h('span') }),
}))
vi.mock('@modrinth/ui', () => ({
    ProjectPageSkeleton: defineComponent({ setup: () => () => h('div', { 'data-skeleton': '' }) }),
    commonMessages: { retryButton: { defaultMessage: 'Retry' } },
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    Card: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('article', slots.default?.()),
    }),
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    injectNotificationManager: () => ({ handleError: vi.fn() }),
}))
const archive = (slug: string, manual = false) => ({
    uuid: '12345678-1234-5678-1234-567812345678',
    slug,
    name: slug,
    modVersions: [
        {
            uuid: 'release',
            name: 'Release',
            gameVersions: [],
            files: [
                {
                    uuid: 'file',
                    name: 'mod.jar',
                    sha256: 'a'.repeat(64),
                    archiveUrl: manual ? null : 'https://example.org/mod.jar',
                    pageUrl: 'https://example.org/download',
                },
            ],
        },
    ],
})
const planet = (id: string, manual = false) => ({
    id,
    title: id,
    pageUrl: 'https://example.org/project',
    versions: [
        {
            id: 'release',
            name: 'Release',
            gameVersions: [],
            download: {
                fileName: 'mod.jar',
                sha256: 'a'.repeat(64),
                directUrl: manual ? null : 'https://example.org/mod.jar',
                pageUrl: 'https://example.org/download',
            },
        },
    ],
})
beforeEach(() => {
    vi.clearAllMocks()
    target.value = { id: 'instance', name: 'Target' }
    route.params.slug = 'old'
    route.params.id = 'old'
    showInstall.mockResolvedValue({ job_id: 'job' })
    cancelInstall.mockResolvedValue(undefined)
    vi.mocked(getMcArchiveModBySlug).mockResolvedValue(archive('old') as never)
    vi.mocked(getPlanetMinecraftProject).mockResolvedValue(planet('old') as never)
})
for (const [name, component, loader, field, data] of [
    ['MCArchive', McArchive, getMcArchiveModBySlug, 'slug', archive],
    ['Planet Minecraft', PlanetMinecraft, getPlanetMinecraftProject, 'id', planet],
] as const) {
    it(`${name} uses a frozen resource review and tracks its accepted job`, async () => {
        const wrapper = mount(component)
        try {
            await flushPromises()
            await wrapper
                .findAll('button')
                .find((button) => button.text() === 'Install')!
                .trigger('click')
            await flushPromises()
            expect(showInstall).toHaveBeenCalledWith(
                expect.objectContaining({
                    instanceId: 'instance',
                    selections: [
                        expect.objectContaining({
                            release: expect.objectContaining({
                                kind: name === 'MCArchive' ? 'artifact' : 'release',
                            }),
                        }),
                    ],
                }),
            )
            expect(trackJob).toHaveBeenCalledExactlyOnceWith({ job_id: 'job' })
        } finally {
            wrapper.unmount()
        }
    })
    it(`${name} shows source download guidance instead of an unverifiable import action when no hash exists`, async () => {
        const value = data('old', true)
        if ('modVersions' in value) value.modVersions[0].files[0].sha256 = ''
        else value.versions[0].download.sha256 = ''
        vi.mocked(loader).mockResolvedValue(value as never)
        const wrapper = mount(component)
        try {
            await flushPromises()
            await wrapper
                .findAll('button')
                .find((button) => button.text() === 'Install')!
                .trigger('click')
            await flushPromises()
            expect(wrapper.text()).toContain('This source provides no file hash')
            expect(
                wrapper
                    .findAll('button')
                    .some((button) => button.text() === 'Import downloaded file'),
            ).toBe(false)
            expect(showInstall).not.toHaveBeenCalled()
        } finally {
            wrapper.unmount()
        }
    })
    it(`${name} aborts old detail requests and cannot import into a newly selected instance`, async () => {
        let finish!: (value: unknown) => void
        let signal: AbortSignal | undefined
        vi.mocked(loader).mockImplementationOnce(((_id: string, currentSignal?: AbortSignal) => {
            signal = currentSignal
            return new Promise((resolve) => {
                finish = resolve
            })
        }) as never)
        const wrapper = mount(component)
        try {
            route.params[field] = 'new'
            vi.mocked(loader).mockResolvedValue(data('new', true) as never)
            await flushPromises()
            finish(data('late'))
            await flushPromises()
            expect(signal?.aborted).toBe(true)
            expect(cancelInstall).toHaveBeenCalledTimes(1)
            expect(wrapper.find('h1').text()).toBe('new')
            await wrapper
                .findAll('button')
                .find((button) => button.text() === 'Install')!
                .trigger('click')
            await flushPromises()
            let pick!: (path: string) => void
            vi.mocked(open).mockImplementationOnce(
                () =>
                    new Promise((resolve) => {
                        pick = resolve
                    }) as never,
            )
            await wrapper
                .findAll('button')
                .find((button) => button.text() === 'Import downloaded file')!
                .trigger('click')
            target.value = { id: 'other', name: 'Other' }
            await flushPromises()
            expect(cancelInstall).toHaveBeenCalledTimes(2)
            pick('C:/Downloads/mod.jar')
            await flushPromises()
            expect(showInstall).not.toHaveBeenCalled()
        } finally {
            wrapper.unmount()
        }
    })
}

it.each([
    ['MCArchive', McArchive, getMcArchiveModBySlug],
    ['Planet Minecraft', PlanetMinecraft, getPlanetMinecraftProject],
] as const)(
    '%s keeps the navigation title through loading, failure and retry',
    async (_name, component, loader) => {
        let fail!: (error: Error) => void
        vi.mocked(loader).mockImplementationOnce(
            () =>
                new Promise((_, reject) => {
                    fail = reject
                }),
        )
        window.history.replaceState(
            { projectPreview: { targetPath: route.path, title: 'Known archive' } },
            '',
        )
        const wrapper = mount(component)
        try {
            expect(wrapper.get('h1').text()).toBe('Known archive')
            fail(new Error('offline'))
            await flushPromises()
            expect(wrapper.get('h1').text()).toBe('Known archive')
            expect(wrapper.find('[role="alert"]').exists()).toBe(true)
            expect(wrapper.find('[aria-busy="true"]').exists()).toBe(false)
            await wrapper
                .findAll('button')
                .find((button) => button.text() === 'Retry')!
                .trigger('click')
            await flushPromises()
            expect(wrapper.get('h1').text()).toBe('old')
            expect(loader).toHaveBeenCalledTimes(2)
        } finally {
            wrapper.unmount()
            window.history.replaceState({}, '')
        }
    },
)

it.each([McArchive, PlanetMinecraft])(
    'shows a title placeholder on a cold direct visit',
    (component) => {
        vi.mocked(getMcArchiveModBySlug).mockReturnValue(new Promise(() => {}))
        vi.mocked(getPlanetMinecraftProject).mockReturnValue(new Promise(() => {}))
        window.history.replaceState({}, '')
        const wrapper = mount(component)
        try {
            expect(wrapper.find('h1').exists()).toBe(false)
            expect(wrapper.find('[aria-busy="true"]').exists()).toBe(true)
            expect(wrapper.find('[data-skeleton]').exists()).toBe(true)
        } finally {
            wrapper.unmount()
        }
    },
)
