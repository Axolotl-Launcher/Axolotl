import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { resourceDescription, resourceGet } from '@/helpers/resources'

import ResourceDescription from './ResourceDescription.vue'

function queryPlugin() {
    return { plugins: [[VueQueryPlugin, { queryClient: new QueryClient() }]] }
}

vi.mock('@/helpers/resources', () => ({
    sourceResource: (source: string, projectId: string) => ({
        kind: 'source',
        project: { source, projectId },
    }),
    resourceGet: vi.fn(),
    resourceDescription: vi.fn(),
}))
vi.mock('@/components/ui/TranslatedProjectDescription.vue', () => ({
    default: defineComponent({
        props: ['description'],
        setup: (props) => () => h('p', props.description),
    }),
}))
vi.mock('@modrinth/ui', () => ({
    ProjectPageSkeleton: defineComponent({ setup: () => () => h('div', { 'data-skeleton': '' }) }),
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, string>) =>
            message.defaultMessage.replace('{source}', values?.source ?? ''),
    }),
}))

const project = (source: string, projectId: string) => ({ project: { source, projectId } })
function ready() {
    vi.mocked(resourceGet).mockResolvedValue({
        id: 'confirmed',
        defaultProject: { source: 'curseforge', projectId: '1' },
        projects: [project('modrinth', 'abcdefgh'), project('curseforge', '1')],
    } as Awaited<ReturnType<typeof resourceGet>>)
    vi.mocked(resourceDescription).mockImplementation(async (_reference, source) => ({
        source,
        format: 'plain',
        body: `${source} description`,
    }))
}

it('reads the preferred description while keeping the original installation reference', async () => {
    ready()
    const wrapper = mount(ResourceDescription, {
        global: queryPlugin(),
        props: { source: 'modrinth', projectId: 'abcdefgh' },
        slots: { default: '<p>Original translated description</p>' },
    })
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('curseforge description')
        expect(resourceDescription).toHaveBeenLastCalledWith(
            { kind: 'source', project: { source: 'modrinth', projectId: 'abcdefgh' } },
            'curseforge',
            undefined,
            expect.any(AbortSignal),
        )
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Modrinth')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Original translated description')
        expect(wrapper.props('projectId')).toBe('abcdefgh')
    } finally {
        wrapper.unmount()
    }
})

it('retries the failed source while preserving the last successful description', async () => {
    ready()
    const wrapper = mount(ResourceDescription, {
        global: queryPlugin(),
        props: { source: 'modrinth', projectId: 'abcdefgh' },
    })
    try {
        await flushPromises()
        vi.mocked(resourceDescription).mockRejectedValueOnce(new Error('unavailable'))
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Modrinth')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('curseforge description')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry description')!
            .trigger('click')
        await flushPromises()
        expect(resourceDescription).toHaveBeenLastCalledWith(
            expect.any(Object),
            'modrinth',
            undefined,
            expect.any(AbortSignal),
        )
        expect(wrapper.text()).toContain('modrinth description')
    } finally {
        wrapper.unmount()
    }
})

it('keeps a completed description on failure and aborts old subscriptions on source change', async () => {
    ready()
    const wrapper = mount(ResourceDescription, {
        global: queryPlugin(),
        props: { source: 'modrinth', projectId: 'abcdefgh' },
    })
    try {
        await flushPromises()
        let oldSignal: AbortSignal | undefined
        let resolveOld!: (value: Awaited<ReturnType<typeof resourceDescription>>) => void
        vi.mocked(resourceDescription).mockImplementationOnce(
            (_reference, _source, _locator, signal) => {
                oldSignal = signal
                return new Promise((resolve) => {
                    resolveOld = resolve
                })
            },
        )
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Modrinth')!
            .trigger('click')
        vi.mocked(resourceDescription).mockRejectedValueOnce(new Error('unavailable'))
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'CurseForge')!
            .trigger('click')
        await flushPromises()
        expect(oldSignal?.aborted).toBe(true)
        expect(wrapper.text()).toContain('curseforge description')
        expect(wrapper.text()).toContain('The description could not be loaded.')
        resolveOld({ source: 'modrinth', format: 'plain', body: 'late response' })
        await flushPromises()
        expect(wrapper.text()).not.toContain('late response')
    } finally {
        wrapper.unmount()
    }
})

it('shows a local skeleton only when no description is available', async () => {
    ready()
    vi.mocked(resourceGet).mockReturnValueOnce(new Promise(() => {}))
    let resolve!: (value: Awaited<ReturnType<typeof resourceDescription>>) => void
    vi.mocked(resourceDescription).mockImplementationOnce(
        () =>
            new Promise((done) => {
                resolve = done
            }),
    )
    const wrapper = mount(ResourceDescription, {
        global: queryPlugin(),
        props: { source: 'planet_minecraft', projectId: 'one' },
    })
    try {
        expect(wrapper.find('[data-skeleton]').exists()).toBe(true)
        resolve({
            source: 'planet_minecraft',
            format: 'plain',
            body: 'planet_minecraft description',
        })
        await flushPromises()
        expect(wrapper.text()).toContain('planet_minecraft description')
        expect(wrapper.find('[data-skeleton]').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }

    vi.mocked(resourceGet).mockReturnValueOnce(new Promise(() => {}))
    const original = mount(ResourceDescription, {
        global: queryPlugin(),
        props: { source: 'mcarchive', projectId: 'two' },
        slots: { default: '<p>Existing description</p>' },
    })
    try {
        expect(original.text()).toContain('Existing description')
        expect(original.find('[data-skeleton]').exists()).toBe(false)
    } finally {
        original.unmount()
    }
})

it('renders the original source before aggregate discovery completes', async () => {
    ready()
    vi.mocked(resourceGet).mockImplementationOnce(() => new Promise(() => {}))
    const wrapper = mount(ResourceDescription, {
        global: queryPlugin(),
        props: { source: 'modrinth', projectId: 'abcdefgh' },
    })
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('modrinth description')
        expect(wrapper.attributes('aria-busy')).toBe('false')
    } finally {
        wrapper.unmount()
    }
})
