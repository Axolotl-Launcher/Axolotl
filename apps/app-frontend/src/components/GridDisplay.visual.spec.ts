import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'

import type { GameInstance } from '@/helpers/types'

import GridDisplay from './GridDisplay.vue'

const backend = vi.hoisted(() => ({
    remove: vi.fn(),
    instances: [] as string[],
    processes: vi.fn(async () => []),
    processListener: vi.fn(async () => () => {}),
}))
vi.mock('@/helpers/instance', () => ({
    remove: backend.remove,
    kill: vi.fn(),
    run: vi.fn(),
    set_pinned: vi.fn(),
}))
vi.mock('@/helpers/instance-backup', () => ({ getBackupDeleteSummary: async () => null }))
vi.mock('@/helpers/events', () => ({
    instance_groups_listener: async () => () => {},
    process_listener: backend.processListener,
}))
vi.mock('@/helpers/process', () => ({
    get_all: backend.processes,
    get_by_instance_id: async () => [],
}))
vi.mock('@/helpers/instance-groups', () => ({
    list_groups: async () => [{ id: 'a', name: 'Group A' }],
    create_group: vi.fn(),
    delete_group: vi.fn(),
    rename_group: vi.fn(),
    set_group_order: vi.fn(),
    update_group_memberships: vi.fn(),
    MAX_INSTANCE_GROUP_NAME_LENGTH: 100,
}))
vi.mock('@/helpers/install', () => ({ install_duplicate_instance: vi.fn() }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@/helpers/utils.js', () => ({ showInstanceInFolder: vi.fn() }))
vi.mock('@/components/ui/modal/InstanceGroupModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/ContextMenu.vue', () => ({ default: { render: () => null } }))
vi.mock('@dnd-kit/vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        DragDropProvider: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
    }
})
vi.mock('@/components/ui/library/InstanceGroup.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: [
                'instances',
                'sectionKey',
                'isCollapsed',
                'selectedInstanceIds',
                'runningInstanceIds',
            ],
            setup:
                (props, { emit }) =>
                () =>
                    h(
                        'section',
                        {
                            'data-running-supplied': String(
                                props.runningInstanceIds instanceof Set,
                            ),
                        },
                        [
                            h(
                                'button',
                                { onClick: () => emit('toggle-collapse', props.sectionKey) },
                                'Collapse',
                            ),
                            ...(props.isCollapsed
                                ? []
                                : props.instances.map(
                                      (instance: Pick<GameInstance, 'id' | 'name'>) =>
                                          h('input', {
                                              type: 'checkbox',
                                              'aria-label': instance.name,
                                              checked: props.selectedInstanceIds.has(instance.id),
                                              onClick: (event: MouseEvent) =>
                                                  emit('handle-checkbox-click', instance.id, event),
                                          }),
                                  )),
                        ],
                    ),
        }),
    }
})
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
        Combobox: control,
        PopoutMenu: control,
        Admonition: control,
        FloatingActionBar: defineComponent({
            props: ['shown'],
            setup:
                (props, { slots }) =>
                () =>
                    props.shown ? h('aside', slots.default?.()) : null,
        }),
        StyledInput: defineComponent({
            props: ['modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        ConfirmActionModal: defineComponent({
            props: ['confirmAction', 'confirmLabel'],
            setup: (props, { slots, expose }) => {
                const visible = ref(false)
                const busy = ref(false)
                const error = ref(false)
                expose({
                    show: () => {
                        visible.value = true
                        error.value = false
                    },
                })
                return () =>
                    visible.value
                        ? h('dialog', { open: true }, [
                              slots.default?.(),
                              error.value ? h('p', 'Deletion failed') : null,
                              h(
                                  'button',
                                  {
                                      disabled: busy.value,
                                      onClick: async () => {
                                          if (busy.value) return
                                          busy.value = true
                                          try {
                                              await props.confirmAction()
                                              visible.value = false
                                          } catch {
                                              error.value = true
                                          } finally {
                                              busy.value = false
                                          }
                                      },
                                  },
                                  props.confirmLabel,
                              ),
                          ])
                        : null
            },
        }),
        NewModal: defineComponent({
            setup: (_, { slots, expose }) => {
                const visible = ref(false)
                expose({ show: () => (visible.value = true), hide: () => (visible.value = false) })
                return () =>
                    visible.value
                        ? h('dialog', { open: true }, [slots.default?.(), slots.actions?.()])
                        : null
            },
        }),
        commonMessages: {
            clearButton: { defaultMessage: 'Clear' },
            deleteLabel: { defaultMessage: 'Delete' },
            cancelButton: { defaultMessage: 'Cancel' },
        },
        defineMessages: (messages: unknown) => messages,
        formatLoader: (_: unknown, loader: string) => loader,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useFormatBytes: () => String,
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, string | number>,
            ) => new IntlMessageFormat(message.defaultMessage, 'en').format(values),
        }),
    }
})
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    localStorage.clear()
    backend.processes.mockReset().mockResolvedValue([])
    backend.processListener.mockReset().mockResolvedValue(() => {})
})

it('does not register a process listener after unmount during the initial read', async () => {
    let finish!: (value: []) => void
    backend.processes.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const wrapper = mount(GridDisplay, { props: { instances: [] } })
    wrapper.unmount()
    finish([])
    await flushPromises()
    expect(backend.processListener).not.toHaveBeenCalled()
})

it('immediately releases a process listener that registers after unmount', async () => {
    let finish!: (value: () => void) => void
    backend.processListener.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const wrapper = mount(GridDisplay, { props: { instances: [] } })
    await flushPromises()
    wrapper.unmount()
    const dispose = vi.fn()
    finish(dispose)
    await flushPromises()
    expect(dispose).toHaveBeenCalledOnce()
})

async function library() {
    backend.instances = ['alpha', 'beta']
    backend.remove.mockReset().mockImplementation(async (id: string) => {
        backend.instances = backend.instances.filter((existing) => existing !== id)
    })
    const wrapper = mount(GridDisplay, {
        props: {
            label: 'selection-test',
            instances: [
                { id: 'alpha', name: 'Alpha', groups: ['a'] },
                { id: 'beta', name: 'Beta', groups: ['a'] },
            ] as GameInstance[],
        },
        global: { directives: { tooltip: {} } },
    })
    mounted.push(wrapper)
    await flushPromises()
    return wrapper
}

function button(wrapper: ReturnType<typeof mount>, label: string) {
    return wrapper.findAll('button').find((item) => item.text() === label)!
}

it('clears hidden selections and deletes only the named visible target', async () => {
    const wrapper = await library()
    await wrapper.get<HTMLInputElement>('input[aria-label="Alpha"]').trigger('click')
    await wrapper.get<HTMLInputElement>('input[aria-label="Beta"]').trigger('click')
    await wrapper.get('input[type="text"]').setValue('Alpha')
    expect(wrapper.get('aside').text()).toContain('1 selected')
    await button(wrapper, 'Delete').trigger('click')
    expect(wrapper.get('dialog').text()).toContain('Alpha')
    expect(wrapper.get('dialog').text()).not.toContain('Beta')
    await button(wrapper, 'Delete instance').trigger('click')
    await flushPromises()
    expect(backend.instances).toEqual(['beta'])
    await wrapper.get('input[type="text"]').setValue('')
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Beta"]').element.checked).toBe(false)
})

it('keeps the confirmed target fixed when the search and selection change', async () => {
    const wrapper = await library()
    await wrapper.get<HTMLInputElement>('input[aria-label="Alpha"]').trigger('click')
    await button(wrapper, 'Delete').trigger('click')
    await wrapper.get('input[type="text"]').setValue('Beta')
    await wrapper.get<HTMLInputElement>('input[aria-label="Beta"]').trigger('click')
    expect(wrapper.get('dialog').text()).toContain('Alpha')
    await button(wrapper, 'Delete instance').trigger('click')
    await flushPromises()
    expect(backend.instances).toEqual(['beta'])
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Beta"]').element.checked).toBe(true)
})

it('clears selections in collapsed sections and does not restore them on expansion', async () => {
    const wrapper = await library()
    await wrapper.get<HTMLInputElement>('input[aria-label="Alpha"]').trigger('click')
    await button(wrapper, 'Collapse').trigger('click')
    expect(wrapper.find('aside').exists()).toBe(false)
    await button(wrapper, 'Collapse').trigger('click')
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Alpha"]').element.checked).toBe(false)
})

it('lists every batch target and preserves a failed deletion for retry', async () => {
    const wrapper = await library()
    await wrapper.get<HTMLInputElement>('input[aria-label="Alpha"]').trigger('click')
    await wrapper.get<HTMLInputElement>('input[aria-label="Beta"]').trigger('click')
    backend.remove.mockImplementation(async (id: string) => {
        if (id === 'beta') throw new Error('Delete failed')
        backend.instances = backend.instances.filter((existing) => existing !== id)
    })
    await button(wrapper, 'Delete').trigger('click')
    expect(wrapper.get('dialog').text()).toContain('Alpha')
    expect(wrapper.get('dialog').text()).toContain('Beta')
    expect(wrapper.get('dialog').text()).toContain('2 instances')
    await button(wrapper, 'Delete 2 instances').trigger('click')
    await flushPromises()
    expect(backend.instances).toEqual(['beta'])
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Alpha"]').element.checked).toBe(false)
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Beta"]').element.checked).toBe(true)
    expect(wrapper.get('dialog').text()).toContain('Deletion failed')
    expect(wrapper.get('dialog').text()).not.toContain('Alpha')
    backend.remove.mockImplementation(async (id: string) => {
        backend.instances = backend.instances.filter((existing) => existing !== id)
    })
    await button(wrapper, 'Delete instance').trigger('click')
    await flushPromises()
    expect(backend.instances).toEqual([])
    expect(wrapper.find('dialog').exists()).toBe(false)
    expect(backend.remove.mock.calls.filter(([id]) => id === 'alpha')).toHaveLength(1)
})

it('supplies the shared running-process set to every instance group', async () => {
    const wrapper = await library()
    const groups = wrapper.findAll('[data-running-supplied]')
    expect(groups.length).toBeGreaterThan(0)
    for (const group of groups) expect(group.attributes('data-running-supplied')).toBe('true')
})
