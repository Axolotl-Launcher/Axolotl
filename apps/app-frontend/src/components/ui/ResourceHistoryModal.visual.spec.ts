import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import {
    type ResourceHistoryEntry,
    resourceListHistory,
    resourceRollback,
} from '@/helpers/resources'

import ResourceHistoryModal from './ResourceHistoryModal.vue'

vi.mock('@/helpers/resources', () => ({ resourceListHistory: vi.fn(), resourceRollback: vi.fn() }))
vi.mock('@/helpers/resource-operation-logic', () => ({
    resourceSourceName: (source: string) => source,
    resourceOperationError: (error: Error) => error.message,
}))
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
    NewModal: defineComponent({
        props: ['onHide', 'disableClose'],
        setup(props, { expose, slots }) {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
                hide: () => {
                    if (props.disableClose) return
                    visible.value = false
                    props.onHide?.()
                },
            })
            return () =>
                visible.value ? h('section', [slots.default?.(), slots.actions?.()]) : null
        },
    }),
}))

const history = (id: string, backup: string | null, available = true) =>
    ({
        id,
        entryId: 'entry',
        instanceId: 'instance',
        recordedAt: '2026-10-14T00:00:00Z',
        previous: { relativePath: `${id}.jar`, originalFileName: `${id}.jar` },
        backupRelativePath: backup,
        backupAvailable: available,
    }) as ResourceHistoryEntry

beforeEach(() => vi.resetAllMocks())

it('closing without rollback settles the caller and reopening excludes consumed backups', async () => {
    vi.mocked(resourceListHistory).mockResolvedValue([
        { ...history('consumed', 'consumed.old', false), backupRetired: true },
        history('missing', 'missing.old', false),
        history('valid', 'valid.old'),
    ])
    const wrapper = mount(ResourceHistoryModal)
    const result = wrapper.vm.show('instance', 'entry')
    await flushPromises()
    expect(wrapper.text()).not.toContain('consumed.jar')
    expect(wrapper.text()).toContain('missing.jar')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Close')!
        .trigger('click')
    expect(await result).toBeNull()
    expect(resourceRollback).not.toHaveBeenCalled()
    const reopened = wrapper.vm.show('instance', 'entry')
    await flushPromises()
    expect(wrapper.findAll('button').every((button) => !button.element.disabled)).toBe(true)
    wrapper.unmount()
    expect(await reopened).toBeNull()
})

it('a failed rollback releases controls and a successful retry settles with the queued job', async () => {
    vi.mocked(resourceListHistory).mockResolvedValue([history('valid', 'valid.old')])
    const job = { job_id: 'rollback', status: 'queued' } as Awaited<
        ReturnType<typeof resourceRollback>
    >
    vi.mocked(resourceRollback).mockRejectedValueOnce(new Error('busy')).mockResolvedValueOnce(job)
    const wrapper = mount(ResourceHistoryModal)
    const result = wrapper.vm.show('instance', 'entry')
    await flushPromises()
    const restore = () =>
        wrapper.findAll('button').find((button) => button.text() === 'Restore this backup')!
    await restore().trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('busy')
    expect(restore().element.disabled).toBe(false)
    await restore().trigger('click')
    await flushPromises()
    expect(await result).toEqual(job)
    expect(wrapper.find('section').exists()).toBe(false)
    wrapper.unmount()
})
