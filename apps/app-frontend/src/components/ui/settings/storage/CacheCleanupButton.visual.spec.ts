import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { purgeLauncherCache } from '@/helpers/cache.js'
import type { CacheCleanupResult } from '@/helpers/storage'

import CacheCleanupButton from './CacheCleanupButton.vue'
const preferences = vi.hoisted(() => ({ skipWarnings: false, locale: 'en' }))
const zhMessages: Record<string, string> = {
    'app.settings.storage.unknownError': '未知错误',
}
vi.mock('@/helpers/cache.js', () => ({ purgeLauncherCache: vi.fn() }))
vi.mock('@/store/theme', () => ({
    useTheming: () => ({ getFeatureFlag: () => preferences.skipWarnings }),
}))
vi.mock('@modrinth/ui', () => ({
    Admonition: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    StyledInput: defineComponent({ setup: () => () => h('input') }),
    Combobox: defineComponent({ props: ['options', 'modelValue'], setup: () => () => h('select') }),
    useFormatBytes: () => (size: number) => `${size} bytes`,
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    Checkbox: defineComponent({
        props: ['modelValue', 'disabled', 'description'],
        emits: ['update:modelValue'],
        setup:
            (props, { emit }) =>
            () =>
                h('button', {
                    disabled: props.disabled,
                    role: 'checkbox',
                    'aria-label': props.description,
                    'aria-checked': props.modelValue,
                    onClick: () => emit('update:modelValue', !props.modelValue),
                }),
    }),
    NewModal: defineComponent({
        name: 'NewModal',
        props: ['onHide', 'disableClose'],
        setup(props, { slots, expose }) {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
                hide: () => {
                    if (!props.disableClose) {
                        visible.value = false
                        props.onHide?.()
                    }
                },
            })
            return () =>
                visible.value ? h('section', [slots.default?.(), slots.actions?.()]) : null
        },
    }),
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (
            message: { id: string; defaultMessage: string },
            values?: Record<string, number>,
        ) => {
            const template =
                (preferences.locale === 'zh-CN' ? zhMessages[message.id] : undefined) ??
                message.defaultMessage
            return new IntlMessageFormat(template, preferences.locale).format(values)
        },
    }),
}))

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    preferences.skipWarnings = false
    preferences.locale = 'en'
    vi.resetAllMocks()
})
it('keeps cache cleanup pending until the shared cleanup operation completes', async () => {
    const wrapper = mount(CacheCleanupButton, { props: { compact: true } })
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    expect(purgeLauncherCache).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('Installed games, saves and backups are kept')
    let complete!: (result: CacheCleanupResult) => void
    vi.mocked(purgeLauncherCache).mockReturnValueOnce(
        new Promise<CacheCleanupResult>((resolve) => {
            complete = resolve
        }),
    )
    const confirm = wrapper.findAll('button').filter((button) => button.text() === 'Clear cache')[1]
    await confirm.trigger('click')
    await flushPromises()
    expect(wrapper.getComponent({ name: 'NewModal' }).props('disableClose')).toBe(true)
    complete({ removed_bytes: 100, remaining_bytes: 30, failures: [] })
    await flushPromises()
    expect(wrapper.find('section').exists()).toBe(true)
    expect(wrapper.emitted('cleared')).toHaveLength(1)
    expect(wrapper.text()).toContain('Freed 100 bytes. Remaining cache: 30 bytes.')
    expect(wrapper.get('button').text()).toBe('Clear cache')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Close')!
        .trigger('click')
    expect(wrapper.find('section').exists()).toBe(false)
})
it('leaves the cache confirmation available for retry after failure', async () => {
    const wrapper = mount(CacheCleanupButton)
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    vi.mocked(purgeLauncherCache).mockRejectedValueOnce(new Error('cache unavailable'))
    await wrapper
        .findAll('button')
        .filter((button) => button.text() === 'Clear cache')[1]
        .trigger('click')
    await flushPromises()
    expect(wrapper.find('section').exists()).toBe(true)
    expect(wrapper.text()).toContain('Could not clear the cache')
    expect(wrapper.emitted('cleared')).toBeUndefined()
})
it('localizes the fallback for a value that cannot be formatted', async () => {
    // `formatMessage` runs while the rejection is handled, so the fallback text
    // follows the current locale instead of the helper's English default.
    const wrapper = mount(CacheCleanupButton)
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    preferences.locale = 'zh-CN'
    vi.mocked(purgeLauncherCache).mockRejectedValueOnce(
        // A plain object (safe to log) whose JSON serialization throws.
        Object.defineProperty({}, 'toJSON', {
            value() {
                throw new Error('serialization rejected')
            },
        }),
    )
    await wrapper
        .findAll('button')
        .filter((button) => button.text() === 'Clear cache')[1]
        .trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('未知错误')
    expect(wrapper.text()).not.toContain('Unknown error')
})
it('shows the message of a serialized Tauri rejection in the failure details', async () => {
    // A rejected `invoke` arrives as a plain serialized object, not an `Error`;
    // the component must surface its message instead of `[object Object]`.
    const wrapper = mount(CacheCleanupButton)
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    vi.mocked(purgeLauncherCache).mockRejectedValueOnce({
        field_name: 'Theseus',
        message: 'cache in use by a running instance',
    })
    await wrapper
        .findAll('button')
        .filter((button) => button.text() === 'Clear cache')[1]
        .trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('Could not clear the cache')
    expect(wrapper.text()).toContain('cache in use by a running instance')
    expect(wrapper.text()).not.toContain('[object Object]')
})
it('shows structured file details when a cleanup rejection has no message', async () => {
    const wrapper = mount(CacheCleanupButton)
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    vi.mocked(purgeLauncherCache).mockRejectedValueOnce({
        path: 'downloads/pack.zip',
        reason: 'permission denied',
    })
    await wrapper
        .findAll('button')
        .filter((button) => button.text() === 'Clear cache')[1]
        .trigger('click')
    await flushPromises()
    expect(wrapper.get('details').text()).toContain('downloads/pack.zip')
    expect(wrapper.get('details').text()).toContain('permission denied')
    expect(wrapper.text()).not.toContain('[object Object]')
    expect(wrapper.emitted('cleared')).toBeUndefined()
})
it('respects the preference to skip optional warnings', async () => {
    preferences.skipWarnings = true
    vi.mocked(purgeLauncherCache).mockResolvedValueOnce({
        removed_bytes: 0,
        remaining_bytes: 30,
        failures: [],
    })
    const wrapper = mount(CacheCleanupButton)
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    await flushPromises()
    expect(purgeLauncherCache).toHaveBeenCalledTimes(1)
    expect(wrapper.text()).toContain('Freed 0 bytes. Remaining cache: 30 bytes.')
    expect(wrapper.emitted('cleared')).toHaveLength(1)
})

it('reports partial deletion without claiming the entire cache was cleared', async () => {
    vi.mocked(purgeLauncherCache).mockResolvedValueOnce({
        removed_bytes: 10,
        remaining_bytes: 80,
        failures: ['content/locked.jar: access denied'],
    })
    const wrapper = mount(CacheCleanupButton, { props: { compact: true } })
    mounted.push(wrapper)
    await wrapper.get('button').trigger('click')
    await wrapper
        .findAll('button')
        .filter((button) => button.text() === 'Clear cache')[1]
        .trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('Some cache files could not be cleared')
    expect(wrapper.text()).toContain('access denied')
    expect(wrapper.emitted('cleared')).toHaveLength(1)
    expect(wrapper.text()).not.toContain('Cache cleared')
})
