import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { resourceChangelog } from '@/helpers/resources'
import { getTranslationSettings, translate } from '@/helpers/translation'

import ResourceReleaseChangelog from './ResourceReleaseChangelog.vue'

const locale = ref('zh-CN')
const visible = ref(false)
vi.mock('@vueuse/core', async (original) => ({
    ...(await original<object>()),
    useElementVisibility: () => visible,
}))
vi.mock('@modrinth/ui', () => ({
    ProjectPageSkeleton: { render: () => null },
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    defineMessages: (value: unknown) => value,
    commonMessages: {
        loadingLabel: { defaultMessage: 'Loading' },
        retryButton: { defaultMessage: 'Retry' },
        cancelButton: { defaultMessage: 'Cancel' },
    },
    useVIntl: () => ({
        locale,
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
vi.mock('@/helpers/page-transition', () => ({
    runAfterPageTransitionSettle: (task: () => void) => task(),
}))
vi.mock('@/helpers/resources', () => ({ resourceChangelog: vi.fn() }))
vi.mock('@/helpers/translation', async (original) => ({
    ...(await original<typeof import('@/helpers/translation')>()),
    getTranslationSettings: vi.fn(),
    translate: vi.fn(),
}))
const release = { project: { source: 'curseforge' as const, projectId: '42' }, releaseId: '100' }
beforeEach(() => {
    visible.value = false
    vi.mocked(resourceChangelog).mockReset()
    vi.mocked(translate).mockReset()
    locale.value = 'zh-CN'
    vi.mocked(getTranslationSettings).mockResolvedValue({
        auto_translate: false,
        target_language: '',
        mode: 'bilingual',
        style: 'weakened',
    } as Awaited<ReturnType<typeof getTranslationSettings>>)
})

it('timeline bodies request once when visible and unvisited bodies never request', async () => {
    vi.mocked(resourceChangelog).mockResolvedValue({
        source: 'curseforge',
        format: 'plain',
        body: 'Changes',
    })
    const wrapper = mount(ResourceReleaseChangelog, { props: { release, lazy: true } })
    try {
        await flushPromises()
        expect(resourceChangelog).not.toHaveBeenCalled()
        visible.value = true
        await flushPromises()
        expect(resourceChangelog).toHaveBeenCalledOnce()
        visible.value = false
        await flushPromises()
        visible.value = true
        await flushPromises()
        expect(resourceChangelog).toHaveBeenCalledOnce()
    } finally {
        wrapper.unmount()
    }
    vi.mocked(resourceChangelog).mockClear()
    visible.value = false
    const hidden = mount(ResourceReleaseChangelog, { props: { release, lazy: true } })
    hidden.unmount()
    await flushPromises()
    expect(resourceChangelog).not.toHaveBeenCalled()
})
const click = async (wrapper: ReturnType<typeof mount>, text: string) => {
    await wrapper
        .findAll('button')
        .find((button) => button.text() === text)!
        .trigger('click')
    await flushPromises()
}

it('renders exact CF HTML safely, distinguishes empty logs and supports fetch retry', async () => {
    vi.mocked(resourceChangelog)
        .mockRejectedValueOnce(new Error('network'))
        .mockResolvedValueOnce({
            source: 'curseforge',
            format: 'html',
            body: '<p><strong>Fixed</strong><a href="https://example.org">details</a><script>alert(1)</script></p>',
        })
        .mockResolvedValueOnce({ source: 'curseforge', format: 'html', body: '' })
    const wrapper = mount(ResourceReleaseChangelog, { props: { release } })
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('could not be loaded')
        await click(wrapper, 'Retry')
        expect(wrapper.find('strong').text()).toBe('Fixed')
        expect(wrapper.find('a').attributes('href')).toBe('https://example.org')
        expect(wrapper.find('script').exists()).toBe(false)
        vi.mocked(translate).mockImplementation(async (request) => ({
            segments: request.segments.map((segment) => ({
                id: segment.id,
                text: segment.text.replace('Fixed', 'Translated'),
            })),
        }))
        await click(wrapper, 'Translate changelog')
        expect(wrapper.findAll('strong').map((element) => element.text())).toEqual([
            'Fixed',
            'Translated',
        ])
        expect(
            wrapper
                .findAll('a')
                .every((element) => element.attributes('href') === 'https://example.org'),
        ).toBe(true)
        await wrapper.setProps({ release: { ...release, releaseId: '101' } })
        await flushPromises()
        expect(wrapper.text()).toContain('No changelog was provided')
        expect(wrapper.find('button').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('aborts old release requests and never displays their late results', async () => {
    let complete!: (value: Awaited<ReturnType<typeof resourceChangelog>>) => void
    let oldSignal: AbortSignal | undefined
    vi.mocked(resourceChangelog)
        .mockImplementationOnce((_release, signal) => {
            oldSignal = signal
            return new Promise((resolve) => {
                complete = resolve
            })
        })
        .mockResolvedValueOnce({
            source: 'modrinth',
            format: 'markdown',
            body: 'Current **changes**',
        })
    const wrapper = mount(ResourceReleaseChangelog, { props: { release } })
    try {
        await wrapper.setProps({
            release: { project: { source: 'modrinth', projectId: 'project' }, releaseId: 'new' },
        })
        await flushPromises()
        complete({ source: 'curseforge', format: 'html', body: 'Stale changes' })
        await flushPromises()
        expect(oldSignal?.aborted).toBe(true)
        expect(wrapper.text()).toContain('Current changes')
        expect(wrapper.text()).not.toContain('Stale changes')
    } finally {
        wrapper.unmount()
    }
})

it('keeps original logs on translation failure and discards cancelled and language-stale translations', async () => {
    vi.mocked(resourceChangelog).mockResolvedValue({
        source: 'curseforge',
        format: 'plain',
        body: 'Original changes',
    })
    vi.mocked(translate).mockRejectedValue(new Error('translator offline'))
    const wrapper = mount(ResourceReleaseChangelog, { props: { release } })
    try {
        await flushPromises()
        await click(wrapper, 'Translate changelog')
        expect(wrapper.text()).toContain('Translation failed')
        expect(wrapper.text()).toContain('Original changes')
        let complete!: (value: Awaited<ReturnType<typeof translate>>) => void
        vi.mocked(translate).mockImplementation(
            () =>
                new Promise((resolve) => {
                    complete = resolve
                }),
        )
        await click(wrapper, 'Translate changelog')
        await click(wrapper, 'Cancel')
        complete({ segments: [{ id: 'body', text: 'Cancelled translation' }] })
        await flushPromises()
        expect(wrapper.text()).not.toContain('Cancelled translation')
        await click(wrapper, 'Translate changelog')
        locale.value = 'ja-JP'
        await flushPromises()
        complete({ segments: [{ id: 'body', text: 'Wrong language' }] })
        await flushPromises()
        expect(wrapper.text()).not.toContain('Wrong language')
        vi.mocked(translate).mockResolvedValue({
            segments: [{ id: 'body', text: 'Translated changes' }],
        })
        await click(wrapper, 'Translate changelog')
        expect(wrapper.text()).toContain('Translated changes')
        expect(translate).toHaveBeenLastCalledWith(
            expect.objectContaining({
                target_language: 'ja-JP',
                context: expect.objectContaining({
                    title: JSON.stringify(['changelog', 'curseforge', '42', '100']),
                }),
            }),
        )
        await click(wrapper, 'Show original')
        expect(wrapper.text()).not.toContain('Translated changes')
    } finally {
        wrapper.unmount()
    }
})
