import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { scanStorage, type StorageScanEvent, type StorageTree } from '@/helpers/storage'

import StorageSettings from '../StorageSettings.vue'

vi.mock('@/helpers/storage', () => ({ scanStorage: vi.fn(), openStoragePaths: vi.fn() }))
vi.mock('@/helpers/storage/preview-cache-usage', () => ({ previewCacheUsage: async () => null }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@modrinth/assets', () => ({
    RefreshCwIcon: defineComponent({ setup: () => () => h('svg') }),
}))
vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    useFormatBytes: () => (size: number) => `${size} bytes`,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    ProgressBar: defineComponent({
        props: ['progress', 'max'],
        setup: (props) => () => h('progress', { value: props.progress, max: props.max }),
    }),
}))
vi.mock('../SettingsSection.vue', () => ({
    default: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('section', [slots.header?.(), slots.default?.()]),
    }),
}))
vi.mock('./StorageTreeNode.vue', () => ({
    default: defineComponent({ props: ['node'], setup: (props) => () => h('div', props.node.id) }),
}))
vi.mock('./CacheCleanupButton.vue', () => ({
    default: defineComponent({ setup: () => () => h('button', 'Clear cache') }),
}))
vi.mock('./MetaStorageCleanup.vue', () => ({
    default: defineComponent({ setup: () => () => h('input', { 'data-test': 'cleanup' }) }),
}))

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.resetAllMocks()
})
const snapshot = (): StorageTree => ({
    scannedAt: '2026-10-15T00:00:00Z',
    total: { actual: 12, symlink: 0 },
    categories: [{ id: 'meta', type: 'meta', size: { actual: 12, symlink: 0 }, paths: [] }],
    rootOther: null,
    external: [],
    warnings: [],
})

it('keeps the last complete snapshot and cleanup state when refresh fails', async () => {
    vi.mocked(scanStorage).mockResolvedValueOnce(snapshot())
    const wrapper = mount(StorageSettings, { global: { directives: { tooltip: {} } } })
    mounted.push(wrapper)
    await flushPromises()
    await wrapper.get('[data-test="cleanup"]').setValue('selection context')
    vi.mocked(scanStorage).mockRejectedValueOnce(new Error('scan unavailable'))
    await wrapper
        .findAll('button')
        .find((element) => element.text() === 'Update')!
        .trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('12 bytes')
    expect(wrapper.text()).toContain('Storage could not be scanned')
    expect((wrapper.get('[data-test="cleanup"]').element as HTMLInputElement).value).toBe(
        'selection context',
    )
})

it('keeps the cleanup result surface mounted when the metadata category disappears', async () => {
    vi.mocked(scanStorage).mockResolvedValueOnce(snapshot())
    const wrapper = mount(StorageSettings, { global: { directives: { tooltip: {} } } })
    mounted.push(wrapper)
    await flushPromises()
    await wrapper.get('[data-test="cleanup"]').setValue('cleanup result')
    vi.mocked(scanStorage).mockResolvedValueOnce({
        ...snapshot(),
        total: { actual: 0, symlink: 0 },
        categories: [],
    })
    await wrapper
        .findAll('button')
        .find((element) => element.text() === 'Update')!
        .trigger('click')
    await flushPromises()
    expect((wrapper.get('[data-test="cleanup"]').element as HTMLInputElement).value).toBe(
        'cleanup result',
    )
    expect(wrapper.text()).toContain('0 bytes')
})

it('shows cached results and actual scan progress while refresh is pending', async () => {
    let notify!: (event: StorageScanEvent) => void
    let finish!: (tree: StorageTree) => void
    vi.mocked(scanStorage).mockImplementationOnce((_force, progress) => {
        notify = progress!
        return new Promise((resolve) => {
            finish = resolve
        })
    })
    const wrapper = mount(StorageSettings, { global: { directives: { tooltip: {} } } })
    mounted.push(wrapper)
    await flushPromises()
    expect(wrapper.find('.animate-pulse').exists()).toBe(false)
    notify({ kind: 'cached', tree: snapshot() })
    notify({ kind: 'progress', completed: 3, total: 16, category: 'meta' })
    await flushPromises()
    expect(wrapper.text()).toContain('12 bytes')
    expect(wrapper.text()).toContain('Showing the previous scan')
    expect(wrapper.get('progress').attributes('value')).toBe('3')
    expect(wrapper.get('progress').attributes('max')).toBe('16')
    finish({ ...snapshot(), total: { actual: 20, symlink: 0 } })
    await flushPromises()
    expect(wrapper.text()).toContain('20 bytes')
    expect(wrapper.find('progress').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Showing the previous scan')
})
