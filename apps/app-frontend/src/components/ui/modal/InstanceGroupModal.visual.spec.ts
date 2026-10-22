import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'

import InstanceGroupModal from './InstanceGroupModal.vue'

const backend = vi.hoisted(() => ({
    list: vi.fn(),
    groups: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    rename: vi.fn(),
    memberships: new Map<string, string[]>(),
    names: new Map<string, string>(),
}))
vi.mock('@/helpers/instance', () => ({ list: backend.list }))
vi.mock('@/composables/useInstanceGroups', () => ({ FAVORITES_GROUP_ID: 'group:favorites' }))
vi.mock('@/helpers/instance-groups', () => ({
    create_group: backend.create,
    list_groups: backend.groups,
    update_group_memberships: backend.update,
    rename_group: backend.rename,
}))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        Admonition: control,
        TagItem: control,
        StyledInput: defineComponent({
            props: ['modelValue'],
            setup: (props, { emit, expose }) => {
                expose({ select: vi.fn() })
                return () =>
                    h('input', {
                        value: props.modelValue,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    })
            },
        }),
        NewModal: defineComponent({
            props: ['onHide', 'disableClose'],
            setup: (props, { slots, expose }) => {
                const visible = ref(false)
                expose({
                    show: () => (visible.value = true),
                    hide: () => {
                        if (props.disableClose) return
                        visible.value = false
                        props.onHide?.()
                    },
                })
                return () =>
                    visible.value
                        ? h('dialog', { open: true, 'data-close-disabled': props.disableClose }, [
                              slots.default?.(),
                              slots.actions?.(),
                          ])
                        : null
            },
        }),
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, string | number>,
            ) => new IntlMessageFormat(message.defaultMessage, 'en').format(values),
        }),
    }
})

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

async function modal(existing = false) {
    backend.memberships = new Map([
        ['alpha', ['other']],
        ['beta', ['target']],
    ])
    backend.names = new Map([['target', 'Target']])
    backend.list
        .mockReset()
        .mockImplementation(async () =>
            [...backend.memberships].map(([id, groups]) => ({ id, name: id, groups: [...groups] })),
        )
    backend.groups.mockReset().mockResolvedValue([{ id: 'target', name: 'Target' }])
    backend.create.mockReset().mockImplementation(async (name: string) => {
        const id = `group-${backend.names.size}`
        backend.names.set(id, name)
        return { id, name }
    })
    backend.rename.mockReset().mockImplementation(async (id: string, name: string) => {
        backend.names.set(id, name)
        return { id, name }
    })
    backend.update.mockReset().mockImplementation(
        async (
            updates: {
                instance_id: string
                add_group_ids: string[]
                remove_group_ids: string[]
            }[],
        ) => {
            for (const change of updates) {
                const groups = new Set(backend.memberships.get(change.instance_id))
                change.remove_group_ids.forEach((id) => groups.delete(id))
                change.add_group_ids.forEach((id) => groups.add(id))
                backend.memberships.set(change.instance_id, [...groups])
            }
        },
    )
    const wrapper = mount(InstanceGroupModal, {
        props: {
            instanceIds: [],
            ...(existing ? { existingGroupName: 'Target', existingGroupId: 'target' } : {}),
        },
    })
    mounted.push(wrapper)
    wrapper.vm.show()
    await flushPromises()
    return wrapper
}

function button(wrapper: ReturnType<typeof mount>, label: string) {
    return wrapper.findAll('button').find((item) => item.text() === label)!
}

it('retains the draft and selection after saving fails, then retries successfully', async () => {
    const wrapper = await modal(true)
    await wrapper.get('input').setValue('alpha')
    await button(wrapper, 'Add').trigger('click')
    backend.update.mockRejectedValueOnce(new Error('Storage unavailable'))
    await button(wrapper, 'Save').trigger('click')
    await flushPromises()
    expect(wrapper.find('dialog').exists()).toBe(true)
    expect(wrapper.emitted('applied')).toBeUndefined()
    expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('alpha')
    expect(button(wrapper, 'Added').exists()).toBe(true)
    expect(wrapper.get('[role="alert"]').text()).toContain('try again')
    await button(wrapper, 'Save').trigger('click')
    await flushPromises()
    expect(backend.memberships.get('alpha')).toEqual(['other', 'target'])
    expect(wrapper.find('dialog').exists()).toBe(false)
    expect(wrapper.emitted('applied')).toHaveLength(1)
})

it('creates an empty named group and reports a creation failure without closing', async () => {
    const wrapper = await modal()
    await wrapper.get('#new-group-name').setValue('Empty group')
    backend.create.mockRejectedValueOnce(new Error('Storage unavailable'))
    await button(wrapper, 'Create group').trigger('click')
    await flushPromises()
    expect(wrapper.find('dialog').exists()).toBe(true)
    expect(wrapper.get<HTMLInputElement>('#new-group-name').element.value).toBe('Empty group')
    expect(wrapper.find('[role="alert"]').exists()).toBe(true)
    await button(wrapper, 'Create group').trigger('click')
    await flushPromises()
    expect([...backend.names.values()]).toContain('Empty group')
    expect(backend.update).not.toHaveBeenCalled()
    expect(wrapper.emitted('applied')).toHaveLength(1)
})

it('reuses the created group after a membership failure, including an edited name', async () => {
    const wrapper = await modal()
    await button(wrapper, 'Add').trigger('click')
    backend.update.mockRejectedValueOnce(new Error('Storage unavailable'))
    await button(wrapper, 'Create group').trigger('click')
    await flushPromises()
    expect(wrapper.find('dialog').exists()).toBe(true)
    await wrapper.get('#new-group-name').setValue('Renamed draft')
    await button(wrapper, 'Create group').trigger('click')
    await flushPromises()
    expect(backend.create).toHaveBeenCalledTimes(1)
    expect(backend.names.get('group-1')).toBe('Renamed draft')
    expect(backend.memberships.get('alpha')).toEqual(['other', 'group-1'])
    expect(wrapper.emitted('applied')).toHaveLength(1)
})

it('blocks duplicate submissions and locks draft controls until the operation finishes', async () => {
    const wrapper = await modal(true)
    await button(wrapper, 'Add').trigger('click')
    let release!: () => void
    const save = backend.update.getMockImplementation()!
    backend.update.mockImplementationOnce(async (updates) => {
        await new Promise<void>((resolve) => (release = resolve))
        await save(updates)
    })
    const submit = button(wrapper, 'Save')
    submit.element.click()
    submit.element.click()
    wrapper.vm.show()
    await flushPromises()
    expect(backend.update).toHaveBeenCalledTimes(1)
    expect(wrapper.get('dialog').attributes('data-close-disabled')).toBe('true')
    expect(wrapper.get<HTMLInputElement>('input').element.disabled).toBe(true)
    expect(button(wrapper, 'Cancel').attributes('disabled')).toBeDefined()
    release()
    await flushPromises()
    expect(backend.memberships.get('alpha')).toEqual(['other', 'target'])
    expect(wrapper.emitted('applied')).toHaveLength(1)
})

it('prevents saving an unloaded collection and supports retrying the load', async () => {
    const wrapper = await modal()
    backend.list.mockRejectedValueOnce(new Error('Unable to list instances'))
    wrapper.vm.show()
    await flushPromises()
    expect(button(wrapper, 'Create group').attributes('disabled')).toBeDefined()
    expect(wrapper.text()).toContain('Could not load instances')
    await button(wrapper, 'Retry').trigger('click')
    await flushPromises()
    expect(button(wrapper, 'Create group').attributes('disabled')).toBeUndefined()
    expect(wrapper.text()).toContain('alpha')
})
