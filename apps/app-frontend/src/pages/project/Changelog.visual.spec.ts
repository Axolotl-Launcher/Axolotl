import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { resourceReleases } from '@/helpers/resources'

import Changelog from './Changelog.vue'

vi.mock('@modrinth/ui', () => ({
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
    commonMessages: {
        loadingLabel: { defaultMessage: 'Loading' },
        retryButton: { defaultMessage: 'Retry' },
    },
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    useFormatDateTime: () => (date: Date) => date.toISOString().slice(0, 10),
}))
vi.mock('@/helpers/resources', () => ({
    resourceReleases: vi.fn(),
    sourceResource: (source: string, projectId: string) => ({
        kind: 'source',
        project: { source, projectId },
    }),
}))
vi.mock('@/components/ui/ResourceReleaseChangelog.vue', () => ({
    default: defineComponent({
        name: 'ResourceReleaseChangelog',
        props: { release: { type: Object, required: true }, lazy: Boolean },
        setup: (props) => () => h('p', `Body ${props.release.releaseId}`),
    }),
}))
const release = (id: string) => ({
    reference: { project: { source: 'curseforge', projectId: '42' }, releaseId: id },
    version: `Version ${id}`,
    publishedAt: '2026-10-13T00:00:00Z',
})
const page = (ids: string[], cursor: string | null = null) =>
    ({
        entries: ids.map(release),
        cursor,
        hasMore: cursor !== null,
        exhaustedSources: [],
    }) as unknown as Awaited<ReturnType<typeof resourceReleases>>
beforeEach(() => vi.mocked(resourceReleases).mockReset())

it('lists releases without eagerly loading bodies, preserves pages on failure and retries the same cursor', async () => {
    vi.mocked(resourceReleases)
        .mockResolvedValueOnce(page(['100'], 'next'))
        .mockRejectedValueOnce(new Error('offline'))
        .mockResolvedValueOnce(page(['100', '101']))
    const wrapper = mount(Changelog, { props: { project: { id: '42' }, source: 'curseforge' } })
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('CurseForge · 2026-10-13')
        expect(wrapper.text()).toContain('Body 100')
        const bodies = wrapper.findAllComponents({ name: 'ResourceReleaseChangelog' })
        expect(bodies).toHaveLength(1)
        expect(bodies.every((component) => component.props('lazy'))).toBe(true)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Load more releases')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Version 100')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.findAll('article article')).toHaveLength(2)
        expect(resourceReleases).toHaveBeenLastCalledWith(
            { kind: 'source', project: { source: 'curseforge', projectId: '42' } },
            undefined,
            'next',
            undefined,
            expect.any(AbortSignal),
        )
    } finally {
        wrapper.unmount()
    }
})

it('source and project changes cancel old pages and clear selected logs', async () => {
    let oldSignal: AbortSignal | undefined
    let complete!: (value: Awaited<ReturnType<typeof resourceReleases>>) => void
    vi.mocked(resourceReleases)
        .mockImplementationOnce((_reference, _target, _cursor, _locator, signal) => {
            oldSignal = signal
            return new Promise((resolve) => {
                complete = resolve
            })
        })
        .mockResolvedValueOnce(page(['new']))
    const wrapper = mount(Changelog, { props: { project: { id: '42' }, source: 'curseforge' } })
    try {
        await wrapper.setProps({ project: { id: 'other' }, source: 'modrinth' })
        await flushPromises()
        complete(page(['old']))
        await flushPromises()
        expect(oldSignal?.aborted).toBe(true)
        expect(wrapper.text()).toContain('Version new')
        expect(wrapper.text()).not.toContain('Version old')
    } finally {
        wrapper.unmount()
    }
})
