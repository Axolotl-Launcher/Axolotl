import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import StorageBackupSettings from './StorageBackupSettings.vue'

vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
}))
vi.mock('./ResourceManagementSettings.vue', () => ({
    default: defineComponent({ setup: () => () => h('div', { 'data-test': 'resources' }) }),
}))
vi.mock('./BackupRepositorySettings.vue', () => ({
    default: defineComponent({ setup: () => () => h('div', { 'data-test': 'backups' }) }),
}))
vi.mock('./StorageSettings.vue', () => ({
    default: defineComponent({ setup: () => () => h('div', { 'data-test': 'analysis' }) }),
}))
vi.mock('./SettingsRow.vue', () => ({
    default: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', [slots.label?.(), slots.description?.(), slots.control?.()]),
    }),
}))
vi.mock('./SettingsSection.vue', () => ({
    default: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('section', slots.default?.()),
    }),
}))

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))
async function page(url: string) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/settings', component: StorageBackupSettings }],
    })
    await router.push(url)
    await router.isReady()
    const wrapper = mount(StorageBackupSettings, { global: { plugins: [router] } })
    mounted.push(wrapper)
    await flushPromises()
    return { wrapper, router }
}

it('shows storage analysis alongside storage and backup settings', async () => {
    const { wrapper, router } = await page('/settings?source=test#storage-backups')
    await vi.waitFor(() => expect(wrapper.find('[data-test="analysis"]').exists()).toBe(true))
    expect(wrapper.find('[data-test="backups"]').exists()).toBe(true)
    expect(router.currentRoute.value.query).toEqual({ source: 'test' })
    expect(router.currentRoute.value.hash).toBe('#storage-backups')
})

it('keeps old analysis links on the complete storage and backups page', async () => {
    const { wrapper } = await page('/settings?page=storage-analysis#storage-backups')
    await vi.waitFor(() => expect(wrapper.find('[data-test="analysis"]').exists()).toBe(true))
    expect(wrapper.find('[data-test="resources"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="backups"]').exists()).toBe(true)
    expect(wrapper.text()).not.toContain('Back to storage & backups')
})
