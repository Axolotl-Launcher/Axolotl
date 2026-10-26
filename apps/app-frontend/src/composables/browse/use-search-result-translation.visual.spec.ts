import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { useSearchResultTranslation } from './use-search-result-translation'

it('switching to the original language restores the original snapshot and the translation toggle', async () => {
    const original = [{ title: 'Original project' }]
    const projects = ref(original),
        servers = ref<typeof original>([])
    const locale = ref('zh-CN')
    let control!: ReturnType<typeof useSearchResultTranslation>
    const wrapper = mount(
        defineComponent({
            setup: () => {
                control = useSearchResultTranslation({
                    projectHits: projects,
                    serverHits: servers,
                    projectType: ref('mod'),
                    locale,
                    onError: vi.fn(),
                    translate: (async (snapshot: typeof original, language: string) =>
                        language === 'en-US' ? snapshot : [{ title: '中文译文' }]) as never,
                })
                return () => h('div')
            },
        }),
    )
    try {
        await flushPromises()
        expect(projects.value[0].title).toBe('中文译文')
        expect(control.translationActive.value).toBe(true)
        locale.value = 'en-US'
        await flushPromises()
        expect(projects.value).toEqual(original)
        expect(control.translationActive.value).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('empty search snapshots cancel older translations without restoring old results or the wrong resource type', async () => {
    const projects = ref([{ title: 'Old project' }]),
        servers = ref([{ title: 'Old server' }])
    const projectType = ref('mod'),
        locale = ref('zh-CN')
    const pending: Array<(value: typeof projects.value) => void> = []
    const translate = vi.fn(
        () =>
            new Promise<typeof projects.value>((resolve) => {
                pending.push(resolve)
            }),
    )
    let control!: ReturnType<typeof useSearchResultTranslation>
    const wrapper = mount(
        defineComponent({
            setup: () => {
                control = useSearchResultTranslation({
                    projectHits: projects,
                    serverHits: servers,
                    projectType,
                    locale,
                    onError: vi.fn(),
                    translate: translate as never,
                })
                return () => h('div')
            },
        }),
    )
    try {
        projects.value = []
        await flushPromises()
        pending[0]([{ title: 'Stale translated project' }])
        await flushPromises()
        expect(projects.value).toEqual([])
        expect(control.translationActive.value).toBe(false)
        projectType.value = 'server'
        await flushPromises()
        projectType.value = 'mod'
        projects.value = [{ title: 'Current project' }]
        await flushPromises()
        pending[1]([{ title: 'Stale translated server' }])
        pending[2]([{ title: 'Translated current project' }])
        await flushPromises()
        expect(projects.value).toEqual([{ title: 'Translated current project' }])
        expect(servers.value).toEqual([{ title: 'Old server' }])
        expect(translate.mock.calls.at(-1)?.[0]).toEqual([{ title: 'Current project' }])
        locale.value = 'ja-JP'
        await flushPromises()
        expect(translate.mock.calls.at(-1)?.[0]).toEqual([{ title: 'Current project' }])
        pending[3]([{ title: 'Japanese current project' }])
        await flushPromises()
        control.toggle()
        expect(projects.value).toEqual([{ title: 'Current project' }])
    } finally {
        wrapper.unmount()
    }
})

it('shows raw titles before enrichment and discards enrichment for replaced snapshots', async () => {
    const projects = ref([{ title: 'First' }])
    const resolvers: Array<(value: typeof projects.value) => void> = []
    const enrichTitles = vi.fn(
        () => new Promise<typeof projects.value>((resolve) => resolvers.push(resolve)),
    )
    const translate = vi.fn(async (hits: typeof projects.value) => hits)
    const wrapper = mount(
        defineComponent({
            setup() {
                useSearchResultTranslation({
                    projectHits: projects,
                    serverHits: ref([]),
                    projectType: ref('mod'),
                    locale: ref('zh-CN'),
                    onError: vi.fn(),
                    enrichTitles,
                    translate: translate as never,
                })
                return () => h('div', projects.value[0]?.title)
            },
        }),
    )
    try {
        expect(wrapper.text()).toBe('First')
        projects.value = [{ title: 'Second' }]
        await flushPromises()
        resolvers[0]([{ title: 'Stale bilingual name' }])
        await flushPromises()
        expect(wrapper.text()).toBe('Second')
        resolvers[1]([{ title: 'Current bilingual name' }])
        await flushPromises()
        expect(wrapper.text()).toBe('Current bilingual name')
        expect(translate).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})
